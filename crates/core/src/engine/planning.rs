//! Bounded model calls. Large native task sets are planned in independent contexts
//! and joined with conservative batch barriers before global DAG validation.
use super::*;

impl Coordinator<'_> {
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
                crate::verification_preflight::environment_with_overrides(
                    &self.project(),
                    &self.run.config.runner.environment,
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
                "Return an execution Plan for phase {}. Use schema_version=1 and exact snapshot.source_hash. Cover EVERY source task in snapshot.tasks using its exact ID; do not add other sources even when the native tracking file contains more rows. Tasks contain id,description,source_ids,depends_on,reads,writes,verification. Preserve snapshot.metadata.reviewed_task_checks for matching task IDs; these are reviewed corrections, not obsolete baseline checks. Preserve native order and phase barriers; only independent [P] siblings may parallelize in Spec Kit. Every task must have meaningful, individually runnable verification argv unless project-level checks are configured. Do not edit files. This is planning batch {}; dependency references must be internal to this batch; the host adds conservative cross-batch barriers. {}",
                phase.id,
                batch + 1,
                crate::verification_preflight::planning_guidance()
            );
            let (result, worktree, index) = self.invoke(
                "plan-tasks",
                &task_id,
                &phase.id,
                instruction,
                Some(partial.clone()),
            )?;
            let candidate = (|| -> Result<Plan> {
                if !git::changed(&worktree, &self.run.accepted_head)?.is_empty() {
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
                let readiness = crate::verification_preflight::inspect_plan_with_environment(
                    &self.project(),
                    &plan,
                    &project_checks,
                    &self.run.config.runner.environment,
                );
                if !readiness.ready {
                    bail!(
                        "verification_preflight_failed: {}",
                        serde_json::to_string(&readiness)?
                    );
                }
                Ok(plan)
            })();
            let mut plan = match candidate {
                Ok(plan) => plan,
                Err(error) => {
                    let attempt = &mut self.run.attempts[index];
                    attempt.status = "failed".into();
                    attempt.error = Some(format!("{error:#}"));
                    attempt.finished_at = Some(paths::now());
                    self.save("planning_batch_rejected")?;
                    bail!(
                        "needs_input: planning candidate rejected; prepare the run again for a corrected plan: {error:#}"
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
            self.save("planning_batch_accepted")?;
        }
        plan::validate_plan(&combined, snapshot)?;
        Ok(combined)
    }
}
