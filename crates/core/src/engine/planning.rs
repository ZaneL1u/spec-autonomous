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
            let instruction = format!(
                "Return an execution Plan for phase {}. Use schema_version=1 and exact snapshot.source_hash. Cover EVERY source task in snapshot.tasks using its exact ID; do not add other sources even when the native tracking file contains more rows. Tasks contain id,description,source_ids,depends_on,reads,writes,verification. Preserve native order and phase barriers; only independent [P] siblings may parallelize in Spec Kit. Split tasks when useful. Every task must have meaningful, individually runnable verification argv unless project-level checks are configured. Do not edit files. This is planning batch {}; dependency references must be internal to this batch; the host adds conservative cross-batch barriers.",
                phase.id,
                batch + 1
            );
            let (result, worktree, index) = self.invoke(
                "plan-tasks",
                &format!("plan-{}-{}", phase.id, batch + 1),
                &phase.id,
                instruction,
                Some(partial.clone()),
            )?;
            if !git::changed(&worktree, &self.run.accepted_head)?.is_empty() {
                bail!("scope_violation: task planner changed files");
            }
            let mut plan = result
                .plan
                .context("worker_protocol_error: missing execution plan")?;
            if plan.phase_id != phase.id {
                bail!("invalid_plan: wrong phase identity");
            }
            for task in &plan.tasks {
                if task.verification.is_empty() && self.run.config.verification.is_empty() {
                    bail!(
                        "verification_missing: each task needs checks or configured project checks"
                    );
                }
            }
            plan::validate_plan(&plan, &partial)?;
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
