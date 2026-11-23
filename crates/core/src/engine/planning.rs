//! Bounded model calls. Large native task sets are planned in independent contexts
//! and joined with conservative batch barriers before global DAG validation.
use super::*;

/// Marks a planning candidate whose only preflight errors are project-level environment
/// gaps. Such a candidate stays `candidate` and is re-checked instead of re-planned.
pub(super) const ENVIRONMENT_UNAVAILABLE: &str = "verification_environment_unavailable";

/// True when every preflight error is a project-level (no task) missing executable or
/// cwd. A planner cannot fix those; the operator's environment or configuration can.
fn environment_only(report: &crate::verification_preflight::Report) -> bool {
    let mut errors = report
        .diagnostics
        .iter()
        .filter(|d| d.severity == "error")
        .peekable();
    errors.peek().is_some()
        && errors.all(|d| {
            d.task_id.is_none()
                && matches!(
                    d.code.as_str(),
                    "check_executable_missing" | "check_cwd_missing"
                )
        })
}

/// How declared scopes narrow verification; appended to the planning instruction.
pub(super) const PLAN_SCOPE_GUIDANCE: &str = "Declare reads for every task: the files its code and checks depend on beyond its writes (imported modules, fixtures, shared helpers). After a later change, a task's checks are re-run only when the changed files reach its writes or reads; a task with empty reads has its checks re-run after every change. A native task that only runs checks (for example \"run the following checks in order\") and should change no files is one Task with verify_only: true, writes: [] and those commands as its verification; the coordinator runs them itself and issues a repair packet only if they fail.";

impl Coordinator<'_> {
    /// The newest planning candidate for this batch that only the verification
    /// environment held back, when it still matches the current source and baseline.
    /// A stale one is superseded so a fresh planning packet is issued instead.
    fn kept_planning_candidate(
        &mut self,
        task_id: &str,
        phase_id: &str,
        partial: &Snapshot,
    ) -> Result<Option<(WorkerResult, PathBuf, usize)>> {
        let Some(index) = self.run.attempts.iter().rposition(|a| {
            a.kind == "plan-tasks"
                && a.task_id == task_id
                && a.phase_id == phase_id
                && a.status == "candidate"
                && a.error
                    .as_deref()
                    .is_some_and(|e| e.starts_with(ENVIRONMENT_UNAVAILABLE))
        }) else {
            return Ok(None);
        };
        let attempt = &self.run.attempts[index];
        let dir = self.store.attempt_dir(&self.run.id, &attempt.id)?;
        let previous = work_packet::read_input(&dir)
            .ok()
            .and_then(|input| input.snapshot);
        let ids = |s: &Snapshot| s.tasks.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
        let current = attempt.base_commit == self.run.accepted_head
            && previous
                .as_ref()
                .is_some_and(|p| p.source_hash == partial.source_hash && ids(p) == ids(partial));
        let result = if current {
            work_packet::read_result(&dir).ok()
        } else {
            None
        };
        let Some(result) = result else {
            let attempt = &mut self.run.attempts[index];
            attempt.status = "superseded".into();
            attempt.error = Some(
                "superseded: the planning candidate kept for an environment fix no longer matches the source or baseline"
                    .into(),
            );
            self.save("planning_candidate_superseded")?;
            return Ok(None);
        };
        let worktree = PathBuf::from(&attempt.worktree);
        Ok(Some((result, worktree, index)))
    }

    pub(super) fn build_plan(&mut self, phase: &Phase, snapshot: &Snapshot) -> Result<Plan> {
        let pending: Vec<_> = snapshot.tasks.iter().filter(|t| !t.done).cloned().collect();
        let limit = self.run.config.execution.max_planner_tasks;
        let batched = pending.len() > limit;
        let mut combined = Plan {
            schema_version: SCHEMA,
            phase_id: phase.id.clone(),
            source_hash: snapshot.source_hash.clone(),
            tasks: vec![],
            milestone: Some(self.run.milestone.clone()),
        };
        let mut previous_leaves: Vec<String> = vec![];
        for (batch, rows) in pending.chunks(limit).enumerate() {
            let mut partial = snapshot.clone();
            partial.metadata["native_counts"] = serde_json::json!({
                "total": snapshot.tasks.len(),
                "checked": snapshot.tasks.iter().filter(|t| t.done).count()
            });
            partial.tasks = rows.to_vec();
            let mut overrides = BTreeMap::new();
            if let Some(host) = &self.run.host {
                for record in &host.verification_revisions {
                    for diff in record["diff"].as_array().into_iter().flatten() {
                        if diff["scope"] == "task"
                            && diff["phase_id"] == phase.id
                            && diff["superseded_by_native"] != true
                            && let Some(id) = diff["task_id"].as_str()
                            && !self.run.completed_tasks.contains(&Run::key(&phase.id, id))
                        {
                            overrides.insert(id, diff["after"].clone());
                        }
                    }
                }
            }
            partial.metadata["reviewed_task_checks"] = serde_json::json!(overrides);
            partial.metadata["verification_environment"] =
                crate::verification_preflight::environment_for_config(
                    &self.project(),
                    &self.run.config,
                );
            let task_id = format!("plan-{}-{}", phase.id, batch + 1);
            let failures: Vec<&Attempt> = self
                .run
                .attempts
                .iter()
                .filter(|a| {
                    a.kind == "plan-tasks"
                        && a.task_id == task_id
                        && a.phase_id == phase.id
                        && a.status == "failed"
                })
                .collect();
            if let Some(failed) = failures.last() {
                partial.metadata["previous_planning_failure"] = serde_json::json!({
                    "request_id":failed.id,"error":failed.error,
                    "action":"Correct the rejected plan before returning a new candidate; preserve native task coverage."
                });
                // Every earlier rejection, summarised by diagnostic code, so a planner
                // does not cycle through the same mistakes one at a time.
                let history: Vec<serde_json::Value> = failures
                    .iter()
                    .rev()
                    .take(10)
                    .map(|a| {
                        let error = a.error.clone().unwrap_or_default();
                        let mut codes: Vec<String> = error
                            .match_indices("\"code\":\"")
                            .filter_map(|(start, needle)| {
                                let rest = &error[start + needle.len()..];
                                rest.split('"').next().map(str::to_owned)
                            })
                            .collect();
                        codes.sort();
                        codes.dedup();
                        let summary: String = error.chars().take(400).collect();
                        serde_json::json!({"request_id":a.id,"finished_at":a.finished_at,"codes":codes,"summary":summary})
                    })
                    .collect();
                partial.metadata["previous_planning_failures"] = serde_json::json!(history);
            }
            let instruction = format!(
                "Return an execution Plan for phase {}. Use schema_version=1 and exact snapshot.source_hash. Cover EVERY source task in snapshot.tasks using its exact ID; do not add other sources even when the native tracking file contains more rows. Tasks contain id,description,source_ids,depends_on,reads,writes,verification. Preserve snapshot.metadata.reviewed_task_checks for matching task IDs; these are reviewed corrections, not obsolete baseline checks. Preserve native order and phase barriers; only independent [P] siblings may parallelize in Spec Kit. Every task must have meaningful, individually runnable verification argv unless project-level checks are configured. snapshot.tasks[].details, when present, holds the indented sub-items written under that native checkbox (requirements, notes, acceptance commands, code blocks); carry them into the covering Task.description and turn runnable acceptance commands from them into that task's verification argv. Do not edit files. This is planning batch {}; dependency references must be internal to this batch; the host adds conservative cross-batch barriers. {}",
                phase.id,
                batch + 1,
                crate::verification_preflight::planning_guidance()
            );
            // A candidate held back only by the verification environment is checked
            // again as-is; the planner is not asked for the plan it already returned.
            let kept = self.kept_planning_candidate(&task_id, &phase.id, &partial)?;
            let (result, _worktree, index) = match kept {
                Some(kept) => kept,
                // Scope, environment and parallelism guidance are appended after the keyed
                // text, so planning packets issued before them still match their receipts.
                None => self.invoke_host_keyed(
                    "plan-tasks",
                    &task_id,
                    &phase.id,
                    format!(
                        "{instruction} {PLAN_SCOPE_GUIDANCE} {} {} {}",
                        crate::verification_preflight::planning_guidance_addendum(),
                        crate::verification_preflight::parallel_planning_guidance(),
                        crate::verification_preflight::task_sizing_guidance()
                    ),
                    &instruction,
                    Some(partial.clone()),
                )?,
            };
            let candidate = (|| -> Result<Plan> {
                if !self.attempt_changes(index)?.is_empty() {
                    bail!("scope_violation: task planner changed files");
                }
                let plan = result
                    .plan
                    .context("worker_protocol_error: missing execution plan")?;
                if plan.phase_id != phase.id {
                    bail!("invalid_plan: wrong phase identity");
                }
                let project_checks: Vec<Check> = self
                    .run
                    .config
                    .verification
                    .iter()
                    .chain(&self.run.config.task_verification)
                    .cloned()
                    .collect();
                for task in &plan.tasks {
                    if task.verification.is_empty() && project_checks.is_empty() {
                        bail!(
                            "verification_missing: each task needs checks or configured project checks"
                        );
                    }
                }
                plan::validate_plan(&plan, &partial)?;
                let readiness = crate::verification_preflight::inspect_plan_with_config(
                    &self.project(),
                    &plan,
                    &project_checks,
                    &self.run.config,
                );
                if !readiness.ready {
                    bail!(
                        "{}: {}",
                        if environment_only(&readiness) {
                            ENVIRONMENT_UNAVAILABLE
                        } else {
                            "verification_preflight_failed"
                        },
                        serde_json::to_string(&readiness)?
                    );
                }
                Ok(plan)
            })();
            let mut plan = match candidate {
                Ok(plan) => plan,
                Err(error) if format!("{error:#}").starts_with(ENVIRONMENT_UNAVAILABLE) => {
                    // The plan is fine; the project environment is not. Keep the
                    // candidate so the next prepare only re-runs the preflight.
                    let message = format!("{error:#}");
                    let attempt = &mut self.run.attempts[index];
                    attempt.status = "candidate".into();
                    attempt.error = Some(message.clone());
                    attempt.finished_at = Some(paths::now());
                    let attempt_id = attempt.id.clone();
                    self.save("planning_environment_blocked")?;
                    bail!(
                        "needs_input: {ENVIRONMENT_UNAVAILABLE}: project-level verification cannot run in this environment; the planning candidate {attempt_id} is kept. Install the missing executable or create the missing directory (or correct runner.environment / project verification and pass reload_config), then run `spec-autonomous prepare --run-id {}` without --change/--feature; only the verification preflight runs again: {}",
                        self.run.id,
                        message
                            .strip_prefix(&format!("{ENVIRONMENT_UNAVAILABLE}: "))
                            .unwrap_or(&message)
                    );
                }
                Err(error) => {
                    let attempt = &mut self.run.attempts[index];
                    attempt.status = "failed".into();
                    attempt.error = Some(format!("{error:#}"));
                    attempt.finished_at = Some(paths::now());
                    self.save("planning_batch_rejected")?;
                    bail!(
                        "needs_input: planning candidate rejected; run `spec-autonomous prepare --run-id {}` without --change/--feature to issue a corrected planning packet: {error:#}",
                        self.run.id
                    );
                }
            };
            if batched {
                let names: BTreeMap<_, _> = plan
                    .tasks
                    .iter()
                    .map(|t| {
                        (
                            t.id.clone(),
                            format!("batch{}-{}", batch + 1, &paths::hash(&t.id)[..16]),
                        )
                    })
                    .collect();
                for task in &mut plan.tasks {
                    task.id = names[&task.id].clone();
                    task.depends_on = task.depends_on.iter().map(|id| names[id].clone()).collect();
                    if task.depends_on.is_empty() {
                        task.depends_on.extend(previous_leaves.clone());
                    }
                }
            }
            if let Some(host) = &self.run.host
                && let Err(error) =
                    crate::run_revision::reconcile_plan(&mut plan, &host.verification_revisions)
            {
                self.run.attempts[index].status = "failed".into();
                self.run.attempts[index].error = Some(error.to_string());
                self.save("planning_revision_conflict")?;
                return Err(error);
            }
            previous_leaves = plan
                .tasks
                .iter()
                .filter(|t| {
                    !plan
                        .tasks
                        .iter()
                        .any(|other| other.depends_on.contains(&t.id))
                })
                .map(|t| t.id.clone())
                .collect();
            combined.tasks.extend(plan.tasks);
            self.run.attempts[index].status = "accepted".into();
            if self.run.attempts[index]
                .error
                .as_deref()
                .is_some_and(|e| e.starts_with(ENVIRONMENT_UNAVAILABLE))
            {
                self.run.attempts[index].error = None;
            }
            self.save("planning_batch_accepted")?;
        }
        plan::validate_plan(&combined, snapshot)?;
        Ok(combined)
    }
}
