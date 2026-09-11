//! Native planning and milestone/phase verification.
use super::*;

impl Coordinator<'_> {
    pub(super) fn drive(&mut self, create_roadmap: bool) -> Result<()> {
        self.run.status = "running".into();
        if create_roadmap {
            self.run.stage = "planning_roadmap".into();
            self.save("planning_roadmap")?;
            let instruction="Return a Milestone object for the user's bounded goal using ONLY their existing framework. Include schema_version=1, id, goal, framework, revision=1, phases with id/label/title/depends_on/source{kind,selector}/verification, and milestone verification. Split into useful phases with explicit dependencies. Do not edit files. Source kind is openspec-change or speckit-feature. Keep the supplied milestone ID and framework. Propose real verification argv, not echo/true placeholders.".to_string();
            let (result, worktree, index) =
                self.invoke("roadmap", "roadmap", "", instruction, None)?;
            if !git::changed(&worktree, &self.run.accepted_head)?.is_empty() {
                bail!("scope_violation: roadmap planner changed files");
            }
            let m = result
                .milestone
                .context("worker_protocol_error: missing milestone")?;
            if m.id != self.run.milestone.id
                || m.framework != self.run.milestone.framework
                || m.goal != self.run.milestone.goal
            {
                bail!("scope_violation: roadmap changed milestone identity, goal or provider");
            }
            plan::validate_milestone(&m)?;
            self.run.milestone = m;
            self.run.attempts[index].status = "accepted".into();
            provider::save_milestone(&self.project(), &self.run.milestone)?;
            self.run.accepted_head = git::commit(
                Path::new(&self.run.integration),
                "sa: record milestone roadmap",
            )?;
            self.run.selected_phases = plan::select(
                &self.run.milestone,
                &self.run.range,
                &self.run.completed_phases,
            )?;
            self.capture_sources()?;
            self.save("roadmap_ready")?;
        }
        if self.run.mode == "native" {
            return self.handoff();
        }
        let mut phase_count = 0;
        loop {
            self.check()?;
            self.refresh_roadmap()?;
            let phase = self
                .run
                .milestone
                .phases
                .iter()
                .find(|p| {
                    self.run.selected_phases.contains(&p.id)
                        && !self.run.completed_phases.contains(&p.id)
                        && p.depends_on
                            .iter()
                            .all(|d| self.run.completed_phases.contains(d))
                })
                .cloned();
            let Some(phase) = phase else {
                if self
                    .run
                    .selected_phases
                    .iter()
                    .any(|p| !self.run.completed_phases.contains(p))
                {
                    bail!("dependency_blocked: no phase can advance");
                }
                break;
            };
            phase_count += 1;
            if phase_count > 100 {
                bail!("no_progress: phase revision limit");
            }
            self.run.current_phase = Some(phase.id.clone());
            self.native_planning(&phase)?;
            if self.run.milestone.framework == Framework::Speckit {
                provider::check_hooks(&self.project(), &self.run.config)?;
            }
            if self.run.mode != "plan" {
                self.hooks(&phase, "before_implement")?;
            }
            loop {
                self.execute_phase(&phase)?;
                if self.run.mode == "plan" {
                    self.deliver(true)?;
                    self.run.status = "plan_ready".into();
                    self.run.stage = "plan_ready".into();
                    return self.save("plan_ready");
                }
                self.run.stage = "verifying_phase".into();
                self.save("phase_verification")?;
                if let Err(error) = self.verify(&self.project(), &self.checks(&phase, &[]), "phase")
                {
                    self.repair_failure(&phase, error)?;
                    continue;
                }
                let snap = provider::inspect(
                    &self.project(),
                    self.run.milestone.framework,
                    &phase.source.selector,
                    &self.run.config,
                )?;
                let audit = self.audit_snapshot(&phase, &snap)?;
                if audit.iter().any(|a| !a.passed) {
                    self.repair_audit(&phase, &snap, &audit)?;
                    continue;
                }
                let audit_head = self.run.accepted_head.clone();
                self.hooks(&phase, "after_implement")?;
                if let Err(error) =
                    self.verify(&self.project(), &self.checks(&phase, &[]), "post-hook")
                {
                    self.repair_failure(&phase, error)?;
                    continue;
                }
                if self.run.accepted_head != audit_head {
                    continue;
                }
                break;
            }
            let snapshot = provider::inspect(
                &self.project(),
                self.run.milestone.framework,
                &phase.source.selector,
                &self.run.config,
            )?;
            if snapshot.tasks.iter().any(|t| !t.done) {
                bail!("phase_incomplete: remaining native tasks");
            }
            self.run.completed_phases.push(phase.id.clone());
            self.run
                .phase_hashes
                .insert(phase.id.clone(), snapshot.source_hash);
            self.run.stage = "advancing".into();
            self.save("phase_completed")?;
        }
        self.run.stage = if self.run.range.bounded() {
            "verifying_scope"
        } else {
            "verifying_milestone"
        }
        .into();
        if !self.run.range.bounded() {
            let phase = self
                .run
                .milestone
                .phases
                .last()
                .cloned()
                .context("empty_milestone")?;
            if let Err(error) = self.verify(&self.project(), &self.checks_milestone(), "milestone")
            {
                self.repair_failure(&phase, error)?;
                self.reopen(&phase.id);
                return self.drive(false);
            }
            let snap = self.milestone_snapshot()?;
            let audit = self.audit_snapshot(&phase, &snap)?;
            if audit.iter().any(|a| !a.passed) {
                let failed = audit.iter().find(|a| !a.passed).unwrap();
                let path = snap.metadata["acceptance"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|v| v["id"] == failed.requirement)
                    .and_then(|v| v["source_path"].as_str())
                    .unwrap_or("");
                let phase = self
                    .run
                    .milestone
                    .phases
                    .iter()
                    .find(|p| path.contains(&p.source.selector))
                    .cloned()
                    .unwrap_or(phase);
                let current = provider::inspect(
                    &self.project(),
                    self.run.milestone.framework,
                    &phase.source.selector,
                    &self.run.config,
                )?;
                self.repair_audit(&phase, &current, &audit)?;
                self.reopen(&phase.id);
                return self.drive(false);
            }
        }
        self.deliver(false)?;
        self.run.status = if self.run.range.bounded() {
            "scope_completed"
        } else {
            "completed"
        }
        .into();
        self.run.stage = "done".into();
        self.save("completed")
    }
    pub(super) fn audit_snapshot(
        &mut self,
        phase: &Phase,
        snapshot: &Snapshot,
    ) -> Result<Vec<AuditItem>> {
        let required = snapshot.metadata["acceptance"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if required.is_empty() {
            bail!("acceptance_missing: no native references");
        }
        let mut all = vec![];
        for (batch, refs) in required.chunks(32).enumerate() {
            let mut partial = snapshot.clone();
            partial.metadata["acceptance"] = serde_json::json!(refs);
            let (result,worktree,index)=self.invoke("audit",&format!("audit-{}-{}",phase.id,batch+1),&phase.id,"Read every listed native acceptance reference and actual implementation. For claims about native workflow execution, fresh workers, or test provenance, also inspect snapshot.metadata.execution_evidence (immutable JSON and hashes linking actual attempts, native sessions and verification logs). Return audit items {requirement,evidence,passed}; requirement MUST be an exact snapshot.metadata.acceptance[].id. Cover every listed ID exactly once, with concise evidence paths/commands. Do not change files. Be explicit about missing implementation; task checkboxes are not proof.".into(),Some(partial.clone()))?;
            if !git::changed(&worktree, &self.run.accepted_head)?.is_empty() {
                bail!("scope_violation: verifier changed files");
            }
            provider::check_audit(&partial, &result.audit)?;
            all.extend(result.audit);
            self.run.attempts[index].status = "accepted".into();
            self.save("audit_recorded")?;
        }
        provider::check_audit(snapshot, &all)?;
        Ok(all)
    }

    pub(super) fn milestone_snapshot(&self) -> Result<Snapshot> {
        let mut snapshots = vec![];
        for phase in &self.run.milestone.phases {
            snapshots.push(provider::inspect(
                &self.project(),
                self.run.milestone.framework,
                &phase.source.selector,
                &self.run.config,
            )?);
        }
        let mut combined = snapshots.first().cloned().context("empty_milestone")?;
        let mut refs = vec![];
        let mut context = BTreeMap::new();
        for snapshot in snapshots {
            refs.extend(
                snapshot.metadata["acceptance"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            );
            for file in snapshot.context_files {
                context.insert(file.path.clone(), file);
            }
        }
        refs.push(serde_json::json!({"id":format!("milestone/{}/goal",self.run.milestone.id),"description":self.run.milestone.goal,"source_path":"milestone.toml"}));
        refs.sort_by_key(|v| v["id"].as_str().unwrap_or("").to_string());
        refs.dedup_by(|a, b| a["id"] == b["id"]);
        combined.context_files = context.into_values().collect();
        combined.metadata["acceptance"] = serde_json::json!(refs);
        combined.metadata["scope"] = serde_json::json!("milestone");
        Ok(combined)
    }
    pub(super) fn repair_failure(&mut self, phase: &Phase, error: anyhow::Error) -> Result<()> {
        let text = format!("{error:#}");
        if !text.starts_with("verification_failed:") {
            return Err(error);
        }
        self.check()?;
        let snapshot = provider::inspect(
            &self.project(),
            self.run.milestone.framework,
            &phase.source.selector,
            &self.run.config,
        )?;
        self.repair_audit(
            phase,
            &snapshot,
            &[AuditItem {
                requirement: "host-verification".into(),
                evidence: text,
                passed: false,
            }],
        )
    }
    pub(super) fn repair_audit(
        &mut self,
        phase: &Phase,
        snapshot: &Snapshot,
        audit: &[AuditItem],
    ) -> Result<()> {
        if self.run.repair_rounds >= self.run.config.execution.max_repair_rounds {
            bail!("repair_budget_exhausted: selected scope still fails verification");
        }
        self.run.repair_rounds += 1;
        self.save("repair_scheduled")?;
        self.converge(phase, snapshot, audit)
    }
    pub(super) fn reopen(&mut self, phase_id: &str) {
        let mut affected = vec![phase_id.to_string()];
        loop {
            let before = affected.len();
            for p in &self.run.milestone.phases {
                if !affected.contains(&p.id) && p.depends_on.iter().any(|d| affected.contains(d)) {
                    affected.push(p.id.clone());
                }
            }
            if before == affected.len() {
                break;
            }
        }
        self.run
            .completed_phases
            .retain(|id| !affected.contains(id));
        for id in affected {
            self.run.phase_hashes.remove(&id);
        }
    }
    pub(super) fn refresh_roadmap(&mut self) -> Result<()> {
        let path = provider::milestone_path(&self.project(), &self.run.milestone.id)?;
        if !path.exists() {
            return Ok(());
        }
        let m = provider::load_milestone(&self.project(), &self.run.milestone.id)?;
        if m.revision != self.run.milestone.revision {
            if m.goal != self.run.milestone.goal || m.framework != self.run.milestone.framework {
                bail!("needs_input: roadmap scope changed");
            }
            let mut range = self.run.range.clone();
            if range.bounded() {
                range.only = None;
                range.from = self.run.selected_phases.first().cloned();
                range.to = self.run.selected_phases.last().cloned();
            }
            self.run.selected_phases = plan::select(&m, &range, &self.run.completed_phases)?;
            self.run.milestone = m;
            self.save("roadmap_reconciled")?;
        }
        Ok(())
    }
    pub(super) fn native_planning(&mut self, phase: &Phase) -> Result<()> {
        let mut last = String::new();
        let mut repeats = 0;
        for _ in 0..32 {
            self.check()?;
            self.run.stage = "planning_phase".into();
            let snap = provider::inspect(
                &self.project(),
                self.run.milestone.framework,
                &phase.source.selector,
                &self.run.config,
            )?;
            match snap.next_action.kind.as_str() {
                "implement" => return Ok(()),
                "blocked" => bail!("needs_input: {}", snap.next_action.instruction),
                "create" => {
                    self.save("source_create_intent")?;
                    provider::create_source(
                        &self.project(),
                        self.run.milestone.framework,
                        &phase.source.selector,
                        &self.run.config,
                    )?;
                    self.run.accepted_head = git::commit(
                        Path::new(&self.run.integration),
                        "sa: create native planning unit",
                    )?;
                    self.save("source_created")?;
                }
                "planning" => {
                    let before_hook = self.run.accepted_head.clone();
                    self.hooks(phase, &format!("before_{}", snap.next_action.artifact))?;
                    if self.run.accepted_head != before_hook {
                        continue;
                    }
                    if snap.source_hash == last {
                        repeats += 1;
                    } else {
                        repeats = 0;
                        last = snap.source_hash.clone();
                    }
                    if repeats >= self.run.config.execution.no_progress_limit {
                        bail!("no_progress: native artifact did not advance");
                    }
                    let (_, worktree, index) = self.invoke(
                        "native-planning",
                        &format!("{}-{}", phase.id, snap.next_action.artifact),
                        &phase.id,
                        snap.next_action.instruction.clone(),
                        Some(snap.clone()),
                    )?;
                    if self.run.milestone.framework == Framework::Speckit {
                        // Native specify persists a checkout-local selector. It must
                        // never replace the user's selector with a worker path.
                        provider::restore_feature_pointer(
                            &self.project(),
                            &worktree.join(&self.run.project_relative),
                        )?;
                    }
                    let changed = git::changed(&worktree, &self.run.accepted_head)?;
                    let prefix = if self.run.project_relative == "." {
                        String::new()
                    } else {
                        format!("{}/", self.run.project_relative)
                    };
                    let allowed: Vec<_> = snap
                        .next_action
                        .outputs
                        .iter()
                        .map(|p| format!("{prefix}{p}"))
                        .collect();
                    if changed.is_empty() || changed.iter().any(|p| !plan::allowed(p, &allowed)) {
                        bail!(
                            "scope_violation: native planner changed files outside its artifact contract"
                        );
                    }
                    let newer = provider::inspect(
                        &worktree.join(&self.run.project_relative),
                        self.run.milestone.framework,
                        &phase.source.selector,
                        &self.run.config,
                    )?;
                    if newer.source_hash == snap.source_hash {
                        bail!("no_progress: unchanged native artifact");
                    }
                    let patch = git::patch(&worktree, &self.run.accepted_head)?;
                    git::apply(Path::new(&self.run.integration), &patch)?;
                    self.run.accepted_head = git::commit(
                        Path::new(&self.run.integration),
                        &format!("sa: plan native artifact {}", snap.next_action.artifact),
                    )?;
                    self.run.attempts[index].status = "accepted".into();
                    self.save("artifact_accepted")?;
                    self.hooks(phase, &format!("after_{}", snap.next_action.artifact))?;
                }
                _ => bail!("native_action_unsupported"),
            }
        }
        bail!("planning_budget_exhausted")
    }
    pub(super) fn checks(&self, phase: &Phase, task: &[Check]) -> Vec<Check> {
        let mut x = self.run.config.verification.clone();
        x.extend(phase.verification.clone());
        x.extend_from_slice(task);
        if x.is_empty() {
            if let Some(plan) = self.run.plans.get(&phase.id) {
                for task in &plan.tasks {
                    x.extend(task.verification.clone());
                }
            }
        }
        unique_checks(x)
    }
    pub(super) fn task_checks(&self, task: &Task) -> Vec<Check> {
        let mut x = self.run.config.verification.clone();
        x.extend(task.verification.clone());
        x
    }
    pub(super) fn checks_milestone(&self) -> Vec<Check> {
        let mut x = self.run.config.verification.clone();
        x.extend(self.run.milestone.verification.clone());
        if x.is_empty() {
            for phase in &self.run.milestone.phases {
                x.extend(self.checks(phase, &[]));
            }
        }
        unique_checks(x)
    }
    pub(super) fn verify(&mut self, project: &Path, checks: &[Check], label: &str) -> Result<()> {
        if checks.is_empty() {
            bail!("verification_missing: configure real verification argv for {label}");
        }
        let revision = git::head(project)?;
        if !git::clean(project)? {
            bail!(
                "verification_uncommitted_input: code must be committed before host verification"
            );
        }
        for check in checks {
            self.check()?;
            crate::config::validate_check(check)?;
            let dir = self.store.attempt_dir(&self.run.id, &paths::id("check"))?;
            let output = process::execute(process::Request {
                argv: &check.argv,
                cwd: &paths::inside(project, &check.cwd)?,
                env: &self.run.config.runner.environment,
                stdin: None,
                directory: &dir,
                timeout: Duration::from_secs(self.run.config.execution.attempt_timeout_seconds),
                max_log_bytes: self.run.config.execution.max_log_bytes,
                cancel: self.cancel.clone(),
            })?;
            let log = dir.join("stdout.log");
            let stderr = fs::read_to_string(dir.join("stderr.log")).unwrap_or_default();
            let tree_unchanged = git::head(project)? == revision && git::clean(project)?;
            let evidence = Evidence {
                argv: check.argv.clone(),
                cwd: project.join(&check.cwd).to_string_lossy().into(),
                revision: revision.clone(),
                exit_code: output.code,
                log: log.to_string_lossy().into(),
                log_hash: paths::hash(fs::read(&log)?),
                finished_at: paths::now(),
                tree_unchanged,
            };
            self.run.evidence.push(evidence);
            self.save("verification")?;
            if !tree_unchanged {
                bail!("verification_mutated_tree: check changed the code or HEAD being verified");
            }
            if output.code != 0 {
                let stdout = fs::read_to_string(&log).unwrap_or_default();
                bail!(
                    "verification_failed: {label} {:?}, exit {}: {}",
                    check.argv,
                    output.code,
                    format!("{stderr}\n{stdout}")
                        .chars()
                        .take(4096)
                        .collect::<String>()
                );
            }
        }
        Ok(())
    }
}

fn unique_checks(checks: Vec<Check>) -> Vec<Check> {
    let mut unique = vec![];
    for check in checks {
        if !unique.contains(&check) {
            unique.push(check);
        }
    }
    unique
}
