//! Concurrent tasks, candidate integration and native lifecycle hooks.
use super::*;
use crate::markdown;

impl Coordinator<'_> {
    pub(super) fn execute_phase(&mut self, phase: &Phase) -> Result<()> {
        let mut snapshot = provider::inspect(
            &self.project(),
            self.run.milestone.framework,
            &phase.source.selector,
            &self.run.config,
        )?;
        if snapshot.tasks.is_empty() {
            bail!("no_executable_tasks");
        }
        if snapshot.tasks.iter().all(|t| t.done) {
            return Ok(());
        }
        let existing = self
            .run
            .plans
            .get(&phase.id)
            .filter(|p| {
                p.source_hash == snapshot.source_hash
                    || self
                        .run
                        .host
                        .as_ref()
                        .and_then(|h| h.source_revisions.get(&phase.id))
                        == Some(&snapshot.source_hash)
            })
            .cloned();
        let plan = if let Some(p) = existing {
            p
        } else {
            if self.run.attempts.iter().any(|a| {
                a.phase_id == phase.id
                    && a.kind == "implement"
                    && matches!(a.status.as_str(), "issued" | "claimed" | "submitted")
            }) {
                bail!("source_drift: stop outstanding host work before replanning");
            }
            self.run.stage = "planning_tasks".into();
            let p = self.build_plan(phase, &snapshot)?;
            self.run
                .completed_tasks
                .retain(|k| !p.tasks.iter().any(|t| *k == Run::key(&phase.id, &t.id)));
            self.run.plans.insert(phase.id.clone(), p.clone());
            let path = paths::inside(
                &self.project(),
                &format!(
                    ".spec-autonomous/plans/{}-{}.toml",
                    self.run.milestone.id, phase.id
                ),
            )?;
            paths::atomic_write(&path, toml::to_string_pretty(&p)?)?;
            self.run.accepted_head = git::commit(
                Path::new(&self.run.integration),
                "sa: record execution plan",
            )?;
            self.run
                .host
                .as_mut()
                .unwrap()
                .source_revisions
                .insert(phase.id.clone(), snapshot.source_hash.clone());
            self.save("plan_accepted")?;
            p
        };
        if plan.phase_id != phase.id {
            bail!("invalid_plan: supplied plan has wrong phase binding");
        }
        let done_ids: Vec<_> = plan
            .tasks
            .iter()
            .filter(|t| {
                self.run
                    .completed_tasks
                    .contains(&Run::key(&phase.id, &t.id))
            })
            .map(|t| t.id.clone())
            .collect();
        let mut remaining = plan.clone();
        remaining.source_hash = snapshot.source_hash.clone();
        remaining.tasks.retain(|t| !done_ids.contains(&t.id));
        for task in &mut remaining.tasks {
            task.depends_on.retain(|d| !done_ids.contains(d));
        }
        if !remaining.tasks.is_empty() {
            plan::validate_plan(&remaining, &snapshot)?;
        }
        self.run.stage = "executing_phase".into();
        if self.run.mode == "plan" {
            return Ok(());
        }
        let submitted: Vec<_> = self
            .run
            .attempts
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                a.phase_id == phase.id && a.kind == "implement" && a.status == "submitted"
            })
            .map(|(i, _)| i)
            .collect();
        for index in submitted {
            self.check()?;
            let task = plan
                .tasks
                .iter()
                .find(|t| t.id == self.run.attempts[index].task_id)
                .context("stale_request: task no longer in plan")?;
            let dir = self
                .store
                .attempt_dir(&self.run.id, &self.run.attempts[index].id)?;
            let result = work_packet::read_result(&dir);
            self.end_attempt(index, &result)?;
            let accepted =
                result.and_then(|_| self.integrate_task(phase, &plan, &snapshot, task, index));
            if let Err(error) = accepted {
                let message = format!("{error:#}");
                self.run.attempts[index].status = "failed".into();
                self.run.attempts[index].error = Some(message.clone());
                self.save("task_retry")?;
                if message.starts_with("needs_input:") {
                    return Err(error);
                }
                if task.writes.is_empty()
                    && message.starts_with("verification_failed:")
                    && git::patch(
                        Path::new(&self.run.attempts[index].worktree),
                        &self.run.attempts[index].base_commit,
                    )?
                    .is_empty()
                {
                    bail!(
                        "needs_input: verification_plan_requires_revision: task {} has no declared repair scope; inspect the failed check and use run.revise before retrying",
                        task.id
                    );
                }
            } else {
                snapshot = provider::inspect(
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
                    .insert(phase.id.clone(), snapshot.source_hash.clone());
                self.save("source_progress_recorded")?;
            }
        }
        let done: Vec<_> = plan
            .tasks
            .iter()
            .filter(|t| {
                self.run
                    .completed_tasks
                    .contains(&Run::key(&phase.id, &t.id))
            })
            .map(|t| t.id.clone())
            .collect();
        if done.len() == plan.tasks.len() {
            return Ok(());
        }
        let active: Vec<_> = plan
            .tasks
            .iter()
            .filter(|t| {
                self.run.attempts.iter().any(|a| {
                    a.phase_id == phase.id
                        && a.kind == "implement"
                        && a.task_id == t.id
                        && matches!(a.status.as_str(), "issued" | "claimed" | "submitted")
                })
            })
            .collect();
        let limit = self
            .run
            .config
            .execution
            .max_workers
            .min(self.run.config.host.max_concurrency);
        let ready: Vec<_> = plan::ready(&plan, &done, &active, limit)
            .into_iter()
            .cloned()
            .collect();
        let mut issued = 0;
        for task in ready {
            let epoch = self
                .run
                .host
                .as_ref()
                .and_then(|h| h.task_retry_epochs.get(&Run::key(&phase.id, &task.id)))
                .copied()
                .unwrap_or(0);
            let failures: Vec<_> = self
                .run
                .attempts
                .iter()
                .skip(epoch)
                .filter(|a| {
                    a.phase_id == phase.id
                        && a.kind == "implement"
                        && a.task_id == task.id
                        && a.status == "failed"
                })
                .collect();
            if failures.len() >= self.run.config.execution.max_attempts as usize {
                continue;
            }
            let fingerprints: Vec<_> = failures
                .iter()
                .map(|a| {
                    let code = a
                        .error
                        .as_deref()
                        .unwrap_or("")
                        .split(':')
                        .next()
                        .unwrap_or("");
                    let diff =
                        git::patch(Path::new(&a.worktree), &a.base_commit).unwrap_or_default();
                    paths::hash(format!("{code}\n{diff}"))
                })
                .collect();
            if repeated(
                Some(&fingerprints),
                self.run.config.execution.no_progress_limit,
            ) {
                continue;
            }
            let key = paths::hash(format!(
                "implement/{}/{}/{}",
                phase.id, task.id, self.run.accepted_head
            ));
            self.issue("implement",&task.id,&phase.id,"Implement only the assigned native task. Preserve shared native checkboxes and respect the declared write set. Submit a candidate result to the host; the CLI verifies and integrates it.".into(),Some(snapshot.clone()),Some(task.clone()),key)?;
            issued += 1;
        }
        if issued > 0 || !active.is_empty() {
            bail!("awaiting_host: prepared task work is outstanding");
        }
        let errors = self
            .run
            .attempts
            .iter()
            .filter(|a| a.phase_id == phase.id && a.status == "failed")
            .filter_map(|a| a.error.clone())
            .collect::<Vec<_>>()
            .join("; ");
        bail!("no_progress: no eligible task remains: {errors}")
    }
    pub(super) fn integrate_task(
        &mut self,
        phase: &Phase,
        plan: &Plan,
        snapshot: &Snapshot,
        task: &Task,
        index: usize,
    ) -> Result<WorkerResult> {
        self.check()?;
        let attempt = self.run.attempts[index].clone();
        let worker = PathBuf::from(&attempt.worktree);
        let changed = git::changed(&worker, &attempt.base_commit)?;
        let prefix = if self.run.project_relative == "." {
            String::new()
        } else {
            format!("{}/", self.run.project_relative)
        };
        for file in &changed {
            let local = file
                .strip_prefix(&prefix)
                .context("scope_violation: changed another project")?;
            if snapshot.context_files.iter().any(|c| c.path == local)
                || local.starts_with(".spec-autonomous/")
                || (!task.writes.is_empty() && !plan::allowed(local, &task.writes))
            {
                bail!("scope_violation: worker changed {file}");
            }
            paths::inside(&worker, file)?;
        }
        git::commit(&worker, &format!("sa: worker candidate {}", task.id))?;
        self.verify(
            &worker.join(&self.run.project_relative),
            &self.task_checks(task),
            "worker",
        )?;
        if !git::clean(&worker)? {
            bail!("verification_mutated_tree: worker tests changed tracked or unignored files");
        }
        let intent_id = paths::id("intent");
        let (candidate, _) = self
            .repo
            .add_worktree(&intent_id, &self.run.accepted_head)?;
        provider::copy_ignored_context(
            &self.project(),
            &candidate.join(&self.run.project_relative),
        )?;
        let key = Run::key(&phase.id, &task.id);
        self.run.intents.push(IntegrationIntent {
            id: intent_id.clone(),
            attempt_id: attempt.id.clone(),
            task_id: key.clone(),
            worktree: candidate.to_string_lossy().into(),
            expected_head: self.run.accepted_head.clone(),
            candidate_head: None,
            final_head: None,
            state: "prepared".into(),
        });
        let intent_index = self.run.intents.len() - 1;
        self.save("integration_intent")?;
        git::apply(&candidate, &git::patch(&worker, &attempt.base_commit)?)?;
        failpoint("after_candidate_patch");
        let head = git::commit(
            &candidate,
            &format!(
                "sa: candidate {}\n\nSpec-Autonomous-Intent: {intent_id}",
                task.id
            ),
        )?;
        self.run.intents[intent_index].candidate_head = Some(head);
        self.save("candidate_created")?;
        let mut combined = self.task_checks(task);
        for accepted in &plan.tasks {
            if self
                .run
                .completed_tasks
                .contains(&Run::key(&phase.id, &accepted.id))
            {
                combined.extend(accepted.verification.clone());
            }
        }
        self.verify(
            &candidate.join(&self.run.project_relative),
            &combined,
            "combined",
        )?;
        if !git::clean(&candidate)? {
            bail!("verification_mutated_tree: combined tests changed tracked or unignored files");
        }
        let satisfied: Vec<_> = task
            .source_ids
            .iter()
            .filter(|id| {
                plan.tasks
                    .iter()
                    .filter(|t| t.source_ids.contains(id))
                    .all(|t| {
                        t.id == task.id
                            || self
                                .run
                                .completed_tasks
                                .contains(&Run::key(&phase.id, &t.id))
                    })
            })
            .cloned()
            .collect();
        if !satisfied.is_empty() {
            let path = snapshot
                .tracking_file
                .as_ref()
                .context("tracking_unsupported")?;
            let text = paths::read(
                &candidate.join(&self.run.project_relative),
                path,
                2 * 1024 * 1024,
            )?;
            let parsed = markdown::parse(path, &text, self.run.milestone.framework)?;
            for id in &satisfied {
                if !parsed.tasks.iter().any(|t| {
                    &t.id == id
                        && snapshot
                            .tasks
                            .iter()
                            .any(|old| old.id == t.id && old.text_hash == t.text_hash)
                }) {
                    bail!("source_drift: source identity changed");
                }
            }
            markdown::complete(
                &candidate.join(&self.run.project_relative),
                path,
                self.run.milestone.framework,
                &paths::hash(&text),
                &satisfied,
            )?;
        }
        failpoint("after_source_writeback");
        let final_head = git::commit(
            &candidate,
            &format!(
                "sa: accept {}\n\nSpec-Autonomous-Intent: {intent_id}",
                task.id
            ),
        )?;
        failpoint("before_intent_finalize");
        self.run.intents[intent_index].final_head = Some(final_head.clone());
        self.run.intents[intent_index].state = "verified".into();
        self.save("integration_verified")?;
        failpoint("after_candidate_commit");
        git::advance(Path::new(&self.run.integration), &final_head)?;
        failpoint("after_git_advance");
        self.run.accepted_head = final_head;
        self.run.completed_tasks.push(key);
        self.run.intents[intent_index].state = "accepted".into();
        self.run.attempts[index].status = "integrated".into();
        self.save("task_integrated")?;
        let dir = self.store.attempt_dir(&self.run.id, &attempt.id)?;
        Ok(serde_json::from_slice(&fs::read(dir.join("result.json"))?)?)
    }
    pub(super) fn converge(
        &mut self,
        phase: &Phase,
        snap: &Snapshot,
        audit: &[AuditItem],
    ) -> Result<()> {
        self.hooks(phase, "before_converge")?;
        let instruction = format!(
            "Append only in-scope repair tasks to the native tracking file. Preserve every previous byte and task ID; do not change code or existing specifications. Gaps: {}",
            serde_json::to_string(audit)?
        );
        let (_, worker, index) = self.invoke(
            "converge",
            &format!("converge-{}", phase.id),
            &phase.id,
            instruction,
            Some(snap.clone()),
        )?;
        let tracking = snap
            .tracking_file
            .as_ref()
            .context("tracking_unsupported")?;
        let before = paths::read(&self.project(), tracking, 2 * 1024 * 1024)?;
        let after = paths::read(
            &worker.join(&self.run.project_relative),
            tracking,
            2 * 1024 * 1024,
        )?;
        if !after.starts_with(&before) || after.len() == before.len() {
            bail!("invalid_convergence: repairs must append source tasks");
        }
        let parsed = markdown::parse(tracking, &after, self.run.milestone.framework)?;
        if !parsed.diagnostics.is_empty() {
            bail!("invalid_convergence: appended tasks contain parser diagnostics");
        }
        let changed = git::changed(&worker, &self.run.accepted_head)?;
        let expected = if self.run.project_relative == "." {
            tracking.clone()
        } else {
            format!("{}/{tracking}", self.run.project_relative)
        };
        if changed != vec![expected] {
            bail!("scope_violation: convergence changed other files");
        }
        git::apply(
            Path::new(&self.run.integration),
            &git::patch(&worker, &self.run.accepted_head)?,
        )?;
        self.run.accepted_head = git::commit(
            Path::new(&self.run.integration),
            "sa: append in-scope convergence tasks",
        )?;
        self.run.attempts[index].status = "accepted".into();
        self.run.plans.remove(&phase.id);
        self.save("repair_plan")?;
        self.hooks(phase, "after_converge")
    }
    pub(super) fn hooks(&mut self, phase: &Phase, event: &str) -> Result<()> {
        if self.run.milestone.framework != Framework::Speckit {
            return Ok(());
        }
        let path = paths::inside(&self.project(), ".specify/extensions.yml")?;
        if !path.exists() {
            return Ok(());
        }
        let config: serde_json::Value = serde_yaml::from_str(&fs::read_to_string(path)?)?;
        let Some(hooks) = config
            .pointer(&format!("/hooks/{event}"))
            .and_then(serde_json::Value::as_array)
        else {
            return Ok(());
        };
        for hook in hooks {
            if hook["enabled"] == false {
                continue;
            }
            let command = hook["command"]
                .as_str()
                .context("hook_invalid: missing command")?;
            let Some(binding) = self.run.config.hooks.get(command).cloned() else {
                if hook["optional"] == true {
                    self.run.hook_results.insert(
                        format!("{}/{event}/{command}", phase.id),
                        "skipped_optional".into(),
                    );
                    self.save("optional_hook_skipped")?;
                    continue;
                }
                bail!("hook_unsupported: configure an argv bridge for mandatory {command}");
            };
            if hook["condition"].as_str().is_some_and(|s| !s.is_empty()) {
                bail!("hook_unsupported: conditional hook requires a native condition bridge");
            }
            let suffix = if event.contains("converge") {
                format!("/round-{}", self.run.repair_rounds)
            } else {
                String::new()
            };
            let key = format!("{}/{event}/{command}{suffix}", phase.id);
            match self.run.hook_results.get(&key).map(String::as_str) {
                Some("done") => continue,
                Some("intent") if !binding.idempotent => bail!("hook_outcome_unknown: {command}"),
                _ => (),
            }
            self.run.hook_results.insert(key.clone(), "intent".into());
            self.save("hook_intent")?;
            let hook_id = paths::id("hook");
            let dir = self.store.attempt_dir(&self.run.id, &hook_id)?;
            let index = self.run.attempts.len();
            self.run.attempts.push(Attempt {
                id: hook_id,
                task_id: key.clone(),
                phase_id: phase.id.clone(),
                kind: "hook".into(),
                worktree: self.run.integration.clone(),
                branch: self.run.integration_branch.clone(),
                base_commit: self.run.accepted_head.clone(),
                status: "running".into(),
                started_at: paths::now(),
                finished_at: None,
                summary: command.into(),
                error: None,
                pid: None,
                process_identity: None,
                evidence: vec![],
            });
            self.run.stage = format!("hook_{event}");
            self.save("hook_started")?;
            let mut env = self.run.config.environment.clone();
            env.insert(
                "SPEC_AUTONOMOUS_HOOK_KEY".into(),
                format!("{}:{key}", self.run.id),
            );
            let output = process::execute(process::Request {
                argv: &binding.argv,
                cwd: &self.project(),
                env: &env,
                stdin: None,
                directory: &dir,
                timeout: Duration::from_secs(self.run.config.execution.attempt_timeout_seconds),
                max_log_bytes: self.run.config.execution.max_log_bytes,
                cancel: self.cancel.clone(),
            })?;
            self.run.attempts[index].finished_at = Some(paths::now());
            self.run.attempts[index].pid = Some(output.identity.pid);
            self.run.attempts[index].process_identity = Some(output.identity.identity);
            if output.code != 0 {
                bail!("hook_outcome_unknown: {command} returned {}", output.code);
            }
            self.run.accepted_head = git::commit(
                Path::new(&self.run.integration),
                &format!("sa: native hook {command}"),
            )?;
            failpoint("after_hook");
            self.run.hook_results.insert(key, "done".into());
            self.run.attempts[index].status = "accepted".into();
            self.save("hook_done")?;
        }
        Ok(())
    }
}
