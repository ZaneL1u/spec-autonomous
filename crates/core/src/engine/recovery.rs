//! Handoff, delivery and durable-intent reconciliation.
use super::*;

impl Coordinator<'_> {
    pub(super) fn handoff(&mut self) -> Result<()> {
        let next = self
            .run
            .milestone
            .phases
            .iter()
            .find(|p| {
                self.run.selected_phases.contains(&p.id)
                    && !self.run.completed_phases.contains(&p.id)
            })
            .cloned();
        self.run.stage = "handed_off".into();
        self.deliver(true)?;
        self.run.status = "handed_off".into();
        self.run.blocker = next.map(|p| {
            format!(
                "Continue native {:?} workflow for {} in {}",
                self.run.milestone.framework, p.source.selector, self.run.integration
            )
        });
        self.save("handed_off")
    }
    pub(super) fn deliver(&mut self, handoff: bool) -> Result<()> {
        self.check()?;
        self.run.stage = "delivering".into();
        self.save("delivery_intent")?;
        if self.run.config.execution.delivery == "ff-original" {
            let origin = Path::new(&self.run.origin);
            if git::head(origin)? != self.run.origin_head || !git::clean(origin)? {
                bail!("delivery_pending: original checkout changed; verified branch retained");
            }
            if Repository::discover(origin)?.branch()? != self.run.origin_branch {
                bail!("delivery_pending: origin branch changed");
            }
            git::advance(origin, &self.run.accepted_head)?;
            failpoint("after_origin_advance");
            if handoff {
                self.run.origin_head = self.run.accepted_head.clone();
                self.run.origin_sources.clear();
                self.capture_sources()?;
            }
        }
        Ok(())
    }
    pub(super) fn recover(&mut self) -> Result<()> {
        if self.run.stage == "delivering"
            && self.run.config.execution.delivery == "ff-original"
            && git::head(Path::new(&self.run.origin))? == self.run.accepted_head
            && git::clean(Path::new(&self.run.origin))?
        {
            self.run.origin_head = self.run.accepted_head.clone();
            self.run.origin_sources.clear();
            self.capture_sources()?;
            self.run.status = match self.run.mode.as_str() {
                "native" => "handed_off",
                "plan" => "plan_ready",
                _ => {
                    if self.run.range.bounded() {
                        "scope_completed"
                    } else {
                        "completed"
                    }
                }
            }
            .into();
            self.run.stage = "delivery_reconciled".into();
            self.save("delivery_reconciled")?;
            if self.run.terminal() {
                return Ok(());
            }
        }

        for index in 0..self.run.attempts.len() {
            if self.run.attempts[index].status == "receiving" {
                let id = self.run.attempts[index].id.clone();
                let meta = &self
                    .run
                    .host
                    .as_ref()
                    .context("legacy_run_read_only")?
                    .requests[&id];
                let file = paths::inside(
                    &self.store.root,
                    &format!("runs/{}/attempts/{id}/result.json", self.run.id),
                )?;
                if !file.exists() {
                    bail!("receipt_recovery_required: resubmit the same result for {id}");
                }
                if Some(paths::hash(fs::read(file)?)) != meta.receipt_hash {
                    bail!("receipt_corrupt");
                }
                self.run.attempts[index].status = "submitted".into();
                self.run.attempts[index].finished_at = Some(paths::now());
            }
        }
        for a in self
            .run
            .attempts
            .clone()
            .iter()
            .filter(|a| a.status == "running" && a.kind == "hook")
        {
            let dir = self.store.attempt_dir(&self.run.id, &a.id)?;
            process::reconcile_process(&dir.join("process.json"))?;
        }
        for a in &mut self.run.attempts {
            if a.status == "running" {
                a.status = "interrupted".into();
            }
        }
        let uncertain: Vec<_> = self
            .run
            .hook_results
            .iter()
            .filter(|(_, v)| v.as_str() == "intent")
            .map(|(k, _)| k.clone())
            .collect();
        for key in uncertain {
            let command = key.split('/').nth(2).unwrap_or("");
            if !self
                .run
                .config
                .hooks
                .get(command)
                .is_some_and(|h| h.idempotent)
            {
                bail!(
                    "hook_outcome_unknown: {key}; use resolve-hook with explicit outcome evidence"
                );
            }
            if !git::clean(Path::new(&self.run.integration))? {
                bail!(
                    "hook_outcome_unknown: inspect and commit the partial hook changes before retry"
                );
            }
            self.run.accepted_head = git::head(Path::new(&self.run.integration))?;
        }
        for index in 0..self.run.intents.len() {
            let intent = self.run.intents[index].clone();
            if intent.state == "accepted" || intent.state == "abandoned" {
                continue;
            }
            if intent.state == "verified" || intent.state == "baseline_ready" {
                let target = intent
                    .final_head
                    .as_ref()
                    .context("state_corrupt: verified intent missing head")?;
                let actual = git::head(Path::new(&self.run.integration))?;
                if actual == intent.expected_head {
                    git::advance(Path::new(&self.run.integration), target)?;
                } else if actual != *target {
                    bail!("recovery_conflict: unexpected integration HEAD");
                }
                self.run.accepted_head = target.clone();
                if intent.state == "verified" && !self.run.completed_tasks.contains(&intent.task_id)
                {
                    self.run.completed_tasks.push(intent.task_id.clone());
                }
                self.run.intents[index].state = "accepted".into();
                if let Some(a) = self
                    .run
                    .attempts
                    .iter_mut()
                    .find(|a| a.id == intent.attempt_id)
                {
                    a.status = "integrated".into();
                }
                if let Some((phase_id, _)) = intent.task_id.split_once('/') {
                    if let Some(phase) = self.run.milestone.phases.iter().find(|p| p.id == phase_id)
                    {
                        let snap = provider::inspect(
                            &self.project(),
                            self.run.milestone.framework,
                            &phase.source.selector,
                            &self.run.config,
                        )?;
                        self.run
                            .host
                            .as_mut()
                            .unwrap()
                            .source_revisions
                            .insert(phase_id.into(), snap.source_hash);
                    }
                }
            } else {
                self.run.intents[index].state = "abandoned".into();
            }
        }
        if self.run.status == "handed_off" || self.run.status == "plan_ready" {
            let integration = Path::new(&self.run.integration);
            if !git::clean(integration)? {
                bail!("dirty_checkout: commit native handoff changes before resume");
            }
            let actual = git::head(integration)?;
            if actual != self.run.accepted_head {
                self.run.accepted_head = actual;
                self.run.plans.clear();
                self.run.completed_phases.clear();
                self.run.phase_hashes.clear();
                if let Some(host) = self.run.host.as_mut() {
                    host.pending_repair = None;
                    host.source_revisions.clear();
                }
            }
        }
        if git::head(Path::new(&self.run.integration))? != self.run.accepted_head {
            bail!("recovery_conflict: unrecognized integration commit");
        }
        self.reconcile_source()?;
        self.save("reconciled")
    }
    pub(super) fn reconcile_source(&mut self) -> Result<()> {
        let origin = Path::new(&self.run.origin);
        let current = git::head(origin)?;
        if current == self.run.origin_head {
            return Ok(());
        }
        if !git::clean(origin)? {
            bail!("dirty_checkout: commit native changes before resuming");
        }
        if self
            .run
            .attempts
            .iter()
            .any(|a| matches!(a.status.as_str(), "issued" | "claimed" | "receiving"))
        {
            bail!(
                "source_drift: revoke outstanding host work after it stops before reconciling source changes"
            );
        }
        for attempt in &mut self.run.attempts {
            if attempt.status == "submitted" {
                attempt.status = "superseded".into();
                attempt.error = Some(
                    "source_drift: receipt retained but old planning context is no longer current"
                        .into(),
                );
                if let Some(lease) = self
                    .run
                    .host
                    .as_mut()
                    .and_then(|h| h.requests.get_mut(&attempt.id))
                {
                    lease.revoked = true;
                }
            }
        }
        if !git::ancestor(origin, &self.run.origin_head, &current) {
            bail!(
                "needs_input: original history was rewritten; preserve the integration branch and choose a new baseline"
            );
        }
        let changed = git::command(
            origin,
            &[
                "diff",
                "--name-only",
                "-z",
                &self.run.origin_head,
                &current,
                "--",
            ],
            None,
        )?;
        let id = paths::id("reconcile");
        let (candidate, _) = self.repo.add_worktree(&id, &self.run.accepted_head)?;
        if let Err(error) = git::command(
            &candidate,
            &["merge", "--no-ff", "--no-commit", &current],
            None,
        ) {
            bail!(
                "source_reconciliation_conflict: preserved {}: {error}",
                candidate.display()
            );
        }
        let target = git::commit(
            &candidate,
            &format!("sa: reconcile native changes\n\nSpec-Autonomous-Intent: {id}"),
        )?;
        let candidate_project = candidate.join(&self.run.project_relative);
        if provider::milestone_path(&candidate_project, &self.run.milestone.id)?.exists() {
            let mut m = provider::load_milestone(&candidate_project, &self.run.milestone.id)?;
            if m.goal != self.run.milestone.goal || m.framework != self.run.milestone.framework {
                bail!(
                    "needs_input: milestone goal or provider changed; start an explicitly selected new run"
                );
            }
            crate::run_revision::reconcile_milestone(
                &mut m,
                &mut self.run.host.as_mut().unwrap().verification_revisions,
            )?;
            self.run.milestone = m;
        }
        let files: Vec<_> = changed.split('\0').filter(|s| !s.is_empty()).collect();
        let source_prefix = |phase: &Phase| -> String {
            let dir = if self.run.milestone.framework == Framework::Openspec {
                format!("openspec/changes/{}", phase.source.selector)
            } else {
                phase.source.selector.clone()
            };
            if self.run.project_relative == "." {
                dir
            } else {
                format!("{}/{dir}", self.run.project_relative)
            }
        };
        let all_native = files.iter().all(|path| {
            self.run
                .milestone
                .phases
                .iter()
                .any(|p| path.starts_with(&format!("{}/", source_prefix(p))))
        });
        if all_native {
            let affected: Vec<_> = self
                .run
                .milestone
                .phases
                .iter()
                .filter(|p| {
                    files
                        .iter()
                        .any(|path| path.starts_with(&format!("{}/", source_prefix(p))))
                })
                .map(|p| p.id.clone())
                .collect();
            for phase in affected {
                self.reopen(&phase);
                self.run.plans.remove(&phase);
            }
        } else {
            self.run.completed_phases.clear();
            self.run.phase_hashes.clear();
            if let Some(host) = self.run.host.as_mut() {
                host.pending_repair = None;
                host.source_revisions.clear();
            }
            self.run.plans.clear();
        }
        let mut range = self.run.range.clone();
        if range.only.is_some() {
            range.only = self.run.selected_phases.first().cloned();
        } else if range.bounded() {
            range.from = self.run.selected_phases.first().cloned();
            range.to = self.run.selected_phases.last().cloned();
        }
        self.run.selected_phases =
            plan::select(&self.run.milestone, &range, &self.run.completed_phases)?;
        self.run.origin_head = current;
        self.run.origin_sources.clear();
        self.capture_sources()?;
        self.run.intents.push(IntegrationIntent {
            id: id.clone(),
            attempt_id: id.clone(),
            task_id: "native-baseline".into(),
            worktree: candidate.to_string_lossy().into(),
            expected_head: self.run.accepted_head.clone(),
            candidate_head: Some(target.clone()),
            final_head: Some(target.clone()),
            state: "baseline_ready".into(),
        });
        self.save("source_reconciliation_intent")?;
        git::advance(Path::new(&self.run.integration), &target)?;
        self.run.accepted_head = target;
        self.run.intents.last_mut().unwrap().state = "accepted".into();
        self.save("source_reconciled")
    }
}
