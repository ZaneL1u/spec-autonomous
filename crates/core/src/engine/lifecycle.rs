//! Native planning and milestone/phase verification.
use super::*;

impl Coordinator<'_> {
    pub(super) fn drive(&mut self, create_roadmap: bool) -> Result<()> {
        self.run.status = "running".into();
        if create_roadmap {
            self.run.stage = "planning_roadmap".into();
            self.save("planning_roadmap")?;
            let instruction="Return a Milestone object for the user's bounded goal using ONLY their existing framework. Include schema_version=1, id, goal, framework, revision=1, phases with id/label/title/depends_on/source{kind,selector}/verification, and milestone verification. Split into useful phases with explicit dependencies. Do not edit files. Source kind is openspec-change or speckit-feature. Keep the supplied milestone ID and framework. Propose real verification argv, not echo/true placeholders.".to_string();
            let instruction = format!(
                "{}\n{}\nEnvironment: {}",
                instruction,
                crate::verification_preflight::planning_guidance(),
                crate::verification_preflight::environment_with_overrides(
                    &self.project(),
                    &self.run.config.runner.environment
                )
            );
            let (result, _worktree, index) =
                self.invoke("roadmap", "roadmap", "", instruction, None)?;
            if !self.attempt_changes(index)?.is_empty() {
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
            let readiness = crate::verification_preflight::inspect_milestone_with_environment(
                &self.project(),
                &m,
                &self.run.config.runner.environment,
            );
            if !readiness.ready {
                let message = format!(
                    "verification_preflight_failed: {}",
                    serde_json::to_string(&readiness)?
                );
                self.run.attempts[index].status = "failed".into();
                self.run.attempts[index].error = Some(message.clone());
                self.save("roadmap_verification_rejected")?;
                bail!("{message}");
            }
            self.run.milestone = m;
            self.run.attempts[index].status = "accepted".into();
            self.supersede_replaced_submissions()?;
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
        if let Some(repair) = self
            .run
            .host
            .as_ref()
            .and_then(|h| h.pending_repair.clone())
        {
            let phase = self
                .run
                .milestone
                .phases
                .iter()
                .find(|p| p.id == repair.phase_id)
                .cloned()
                .context("stale_repair: phase removed")?;
            let snapshot = provider::inspect(
                &self.project(),
                self.run.milestone.framework,
                &phase.source.selector,
                &self.run.config,
            )?;
            if snapshot.source_hash != repair.source_hash {
                bail!("source_drift: repair source changed");
            }
            self.converge(&phase, &snapshot, &repair.audit)?;
            self.run.host.as_mut().unwrap().pending_repair = None;
            self.reopen(&phase.id);
            self.save("repair_applied")?;
        }
        let mut phase_count = 0;
        loop {
            if self.run.host.as_ref().is_some_and(|h| h.reconcile_pending)
                && !self.run.has_outstanding_host_work()
            {
                self.reconcile_source()?;
            }
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
            let discussion =
                crate::discussion::next(&self.project(), &self.run.id, Some(&phase.id))?;
            let unresolved = discussion["cards"].as_array().is_some_and(|cards| {
                cards.iter().any(|card| {
                    card["requires_user"] == true
                        && !self
                            .run
                            .host
                            .as_ref()
                            .unwrap()
                            .decisions
                            .iter()
                            .any(|d| d["card_id"] == card["id"])
                })
            });
            if unresolved {
                self.run.status = "needs_input".into();
                self.run.blocker = Some(format!(
                    "discussion_required: phase {} has gray-area cards; use discussion.next then discussion.apply before continuing",
                    phase.id
                ));
                self.save("discussion_required")?;
                bail!(
                    "needs_input: discussion_required: phase {} has unresolved gray areas",
                    phase.id
                );
            }
            self.native_planning(&phase)?;
            if self.run.milestone.framework == Framework::Speckit {
                provider::check_hooks(&self.project(), &self.run.config)?;
            }
            if self.run.mode != "plan" {
                self.hooks(&phase, "before_implement")?;
            }
            loop {
                if let Err(error) = self.execute_phase(&phase) {
                    if format!("{error:#}").starts_with("source_reconciled:") {
                        return self.drive(false);
                    }
                    return Err(error);
                }
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
                if self.run.accepted_head == audit_head {
                    // No hook moved the verified revision: the phase wave already covers it.
                    break;
                }
                if let Err(error) =
                    self.verify(&self.project(), &self.checks(&phase, &[]), "post-hook")
                {
                    self.repair_failure(&phase, error)?;
                }
                continue;
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
            self.run.stale_phases.retain(|id| id != &phase.id);
            if let Some(host) = self.run.host.as_mut() {
                let prefix = format!("{}:", phase.id);
                host.repair_signatures
                    .retain(|k, _| !k.starts_with(&prefix));
            }
            self.run
                .phase_hashes
                .insert(phase.id.clone(), snapshot.source_hash);
            self.run.stage = "advancing".into();
            self.save("phase_completed")?;
        }
        self.verify_stale_phases()?;
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
        let tree = git::tree(&self.project())?;
        for (batch, refs) in required.chunks(32).enumerate() {
            // An audit of the same acceptance ids against the same integration tree has
            // the same inputs; a fully passed one is reused instead of asked again.
            let ids: Vec<&str> = refs.iter().filter_map(|v| v["id"].as_str()).collect();
            let key = paths::hash(serde_json::to_string(&serde_json::json!({
                "phase": phase.id,
                "scope": snapshot.metadata["scope"],
                "tree": tree,
                "ids": ids,
            }))?);
            if let Some(items) = self
                .run
                .host
                .as_ref()
                .and_then(|h| h.audit_cache.get(&key))
                .cloned()
            {
                all.extend(items);
                self.save("audit_reused")?;
                continue;
            }
            let mut partial = snapshot.clone();
            partial.metadata["acceptance"] = serde_json::json!(refs);
            let audit_task = if snapshot.metadata["scope"] == "milestone" {
                format!("audit-milestone-{}-{}", phase.id, batch + 1)
            } else {
                format!("audit-{}-{}", phase.id, batch + 1)
            };
            let (result, _worktree, index) = self.invoke(
                "audit",
                &audit_task,
                &phase.id,
                "Read every listed native acceptance reference and actual implementation. For claims about native workflow execution, fresh workers, or test provenance, also inspect snapshot.metadata.execution_evidence (immutable JSON and hashes linking actual attempts, native sessions and verification logs). Return audit items {requirement,evidence,passed}; requirement MUST be an exact snapshot.metadata.acceptance[].id. Cover every listed ID exactly once, with concise evidence paths/commands. Do not change files. Be explicit about missing implementation; task checkboxes are not proof.".into(),
                Some(partial.clone()),
            )?;
            let verified = (|| -> Result<()> {
                if !self.attempt_changes(index)?.is_empty() {
                    bail!("scope_violation: verifier changed files");
                }
                provider::check_audit(&partial, &result.audit)?;
                Ok(())
            })();
            if let Err(error) = verified {
                self.run.attempts[index].status = "failed".into();
                self.run.attempts[index].error = Some(format!("{error:#}"));
                self.save("audit_rejected")?;
                return Err(error);
            }
            if result.audit.iter().all(|a| a.passed)
                && let Some(host) = self.run.host.as_mut()
            {
                host.audit_cache.insert(key, result.audit.clone());
            }
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
        // Wave paths differ every time, so a host-verification failure is identified by
        // the failing command and exit code; audit failures by their requirement ids.
        let failing = self
            .run
            .evidence
            .iter()
            .rev()
            .find(|e| e.exit_code != 0 || !e.tree_unchanged)
            .map(|e| serde_json::json!({"argv":e.argv,"cwd":e.cwd,"exit_code":e.exit_code}));
        let failed: Vec<serde_json::Value> = audit
            .iter()
            .filter(|a| !a.passed)
            .map(|a| {
                if a.requirement == "host-verification" {
                    serde_json::json!({"requirement":a.requirement,"check":failing})
                } else {
                    serde_json::json!({"requirement":a.requirement})
                }
            })
            .collect();
        let signature = format!(
            "{}:{}",
            phase.id,
            paths::hash(serde_json::to_string(&failed)?)
        );
        let requirements: Vec<&str> = audit
            .iter()
            .filter(|a| !a.passed)
            .map(|a| a.requirement.as_str())
            .collect();
        let seen = self
            .run
            .host
            .as_mut()
            .context("legacy_run_read_only")?
            .repair_signatures
            .entry(signature)
            .or_insert(0);
        *seen += 1;
        if *seen > 1 {
            self.save("repair_repeated")?;
            bail!(
                "needs_input: repair_repeated: phase {} failed the same way after a repair round ({}); inspect the evidence, then use run.revise for a wrong check or fix the native source before continuing",
                phase.id,
                requirements.join(", ")
            );
        }
        self.run.repair_rounds += 1;
        self.run
            .host
            .as_mut()
            .context("legacy_run_read_only")?
            .pending_repair = Some(PendingRepair {
            phase_id: phase.id.clone(),
            source_hash: snapshot.source_hash.clone(),
            audit: audit.to_vec(),
        });
        self.save("repair_scheduled")?;
        self.converge(phase, snapshot, audit)?;
        self.run.host.as_mut().unwrap().pending_repair = None;
        Ok(())
    }
    /// Reopen one phase for planning and execution. Phases depending on it keep their
    /// accepted work and plans; they only owe a fresh command gate before delivery.
    pub(super) fn reopen(&mut self, phase_id: &str) {
        self.run.completed_phases.retain(|id| id != phase_id);
        self.run.stale_phases.retain(|id| id != phase_id);
        self.run.phase_hashes.remove(phase_id);
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
        for id in affected.into_iter().skip(1) {
            self.mark_stale(&id);
        }
    }
    pub(super) fn mark_stale(&mut self, phase_id: &str) {
        if self.run.completed_phases.iter().any(|id| id == phase_id)
            && !self.run.stale_phases.iter().any(|id| id == phase_id)
        {
            self.run.stale_phases.push(phase_id.to_string());
        }
    }
    /// Re-run only the command gates of completed phases whose tree changed underneath
    /// them. Accepted work, plans and audits stay; a failure enters the normal repair path.
    fn verify_stale_phases(&mut self) -> Result<()> {
        self.run
            .stale_phases
            .retain(|id| self.run.completed_phases.contains(id));
        while let Some(id) = self.run.stale_phases.first().cloned() {
            let Some(phase) = self
                .run
                .milestone
                .phases
                .iter()
                .find(|p| p.id == id)
                .cloned()
            else {
                self.run.stale_phases.retain(|p| p != &id);
                continue;
            };
            self.run.stage = "verifying_stale_phase".into();
            self.save("stale_phase_verification")?;
            if let Err(error) = self.verify(&self.project(), &self.checks(&phase, &[]), "phase") {
                self.repair_failure(&phase, error)?;
                self.reopen(&phase.id);
                return self.drive(false);
            }
            self.run.stale_phases.retain(|p| p != &id);
            self.save("stale_phase_verified")?;
        }
        Ok(())
    }
    pub(super) fn refresh_roadmap(&mut self) -> Result<()> {
        let path = provider::milestone_path(&self.project(), &self.run.milestone.id)?;
        if !path.exists() {
            return Ok(());
        }
        let mut m = provider::load_milestone(&self.project(), &self.run.milestone.id)?;
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
            crate::run_revision::reconcile_milestone(
                &mut m,
                &mut self.run.host.as_mut().unwrap().verification_revisions,
            )?;
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
        x.extend(self.run.config.phase_verification.clone());
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
        x.extend(self.run.config.task_verification.clone());
        x.extend(task.verification.clone());
        x
    }
    pub(super) fn checks_milestone(&self) -> Vec<Check> {
        let mut x = self.run.config.verification.clone();
        x.extend(self.run.config.milestone_verification.clone());
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
        if !git::clean(project)? {
            bail!(
                "verification_uncommitted_input: code must be committed before host verification"
            );
        }
        let directory = self
            .store
            .root
            .join("runs")
            .join(&self.run.id)
            .join(paths::id("verify"));
        let wave = crate::verification::Wave::new(directory, project, label, &self.run.config)?;
        let wave_id = wave
            .directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let tree = git::tree(project)?;
        // Long-lived integration checkouts carry build caches that fresh worktrees do not;
        // evidence never crosses that boundary.
        let class = if project.starts_with(&self.run.integration) {
            "integration"
        } else {
            "worktree"
        };
        let reuse = self.run.config.execution.verification_reuse == "same-tree";
        let preparation = self.run.config.hooks.get("verification_prepare").cloned();
        if let Some(hook) = &preparation {
            if !hook.idempotent {
                bail!(
                    "verification_prepare_requires_idempotent: preparation must be safe to retry for its wave identity"
                );
            }
        }
        let mut prepared = preparation.is_none();
        for check in unique_checks(checks.to_vec()) {
            self.check()?;
            let key = paths::hash(serde_json::to_string(&serde_json::json!({
                "tree": tree,
                "class": class,
                "argv": check.argv,
                "cwd": check.cwd,
                "environment": self.run.config.runner.environment,
                "prepare": preparation.as_ref().map(|h| &h.argv),
                "attempt_timeout_seconds": self.run.config.execution.attempt_timeout_seconds,
                "max_log_bytes": self.run.config.execution.max_log_bytes,
            }))?);
            let hit = if reuse {
                self.run
                    .host
                    .as_ref()
                    .and_then(|h| h.verification_cache.get(&key))
                    .cloned()
            } else {
                None
            };
            if let Some(pass) = hit {
                let cwd = paths::inside(project, &check.cwd)?;
                let details = serde_json::json!({"schema_version":1,"wave_id":wave_id,"stage":label,"step":"check","duration_ms":0,"failure_kind":null,"exit_code":0,"argv":check.argv,"cwd":cwd,"revision":wave.revision,"tree_unchanged":true,"stdout":pass.log,"stdout_sha256":pass.log_hash,"manifest":pass.manifest,"output_directory":wave.directory,"reused_from":{"stage":pass.stage,"wave_id":pass.wave_id,"revision":pass.revision,"manifest":pass.manifest,"tree":pass.tree,"finished_at":pass.finished_at},"cache_key":key});
                self.run.evidence.push(Evidence {
                    argv: check.argv.clone(),
                    cwd: cwd.to_string_lossy().into_owned(),
                    revision: wave.revision.clone(),
                    exit_code: 0,
                    log: pass.log.clone(),
                    log_hash: pass.log_hash.clone(),
                    finished_at: paths::now(),
                    tree_unchanged: true,
                    details: Some(details),
                });
                self.save("verification_reused")?;
                continue;
            }
            if !prepared {
                let hook = preparation.as_ref().unwrap();
                let step = Check {
                    argv: hook.argv.clone(),
                    cwd: ".".into(),
                };
                self.record_check(&wave, project, &step, true)?;
                prepared = true;
            }
            let details = self.record_check(&wave, project, &check, false)?;
            if reuse && details["tree_unchanged"] == true {
                if let Some(host) = self.run.host.as_mut() {
                    host.verification_cache.insert(
                        key,
                        VerificationPass {
                            stage: label.into(),
                            wave_id: wave_id.clone(),
                            tree: tree.clone(),
                            revision: wave.revision.clone(),
                            manifest: details["manifest"].as_str().unwrap_or_default().into(),
                            log: details["stdout"].as_str().unwrap_or_default().into(),
                            log_hash: details["stdout_sha256"].as_str().unwrap_or_default().into(),
                            finished_at: paths::now(),
                        },
                    );
                }
            }
        }
        Ok(())
    }
    /// Execute one check or preparation step, persist its evidence and fail on any
    /// non-passing outcome. Returns the durable manifest details of a passing step.
    fn record_check(
        &mut self,
        wave: &crate::verification::Wave,
        project: &Path,
        check: &Check,
        preparation: bool,
    ) -> Result<serde_json::Value> {
        let label = wave.stage.clone();
        let result = wave.execute(check, preparation, &self.run.config, self.cancel.clone())?;
        let details = result.evidence.details.clone().unwrap();
        self.run.evidence.push(result.evidence);
        self.save("verification")?;
        if let Some(failure) = result.failure {
            if failure == "source_changed" {
                bail!(
                    "verification_mutated_tree: diagnostics {}",
                    details["manifest"]
                );
            }
            // Retain the existing actionable Node directory diagnosis without
            // exposing arbitrary child output in a public error.
            let stderr =
                fs::read_to_string(details["stderr"].as_str().unwrap()).unwrap_or_default();
            let readiness = crate::verification_preflight::inspect_checks_with_environment(
                project,
                std::slice::from_ref(check),
                &[],
                &self.run.config.runner.environment,
            );
            if stderr.contains("MODULE_NOT_FOUND")
                && readiness
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "node_test_directory_operand")
            {
                bail!(
                    "needs_input: verification_configuration: Node rejected a test directory operand; use run.revise; diagnostics {}",
                    details["manifest"]
                );
            }
            bail!(
                "verification_failed: {label} {} {failure}, exit {}; diagnostics {}; stdout {}; stderr {}",
                if preparation { "preparation" } else { "check" },
                details["exit_code"],
                details["manifest"],
                details["stdout"],
                details["stderr"]
            );
        }
        Ok(details)
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
