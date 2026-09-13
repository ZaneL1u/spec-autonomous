//! Reviewed, CAS-protected changes to pending verification. No Git mutations.
use crate::{
    git::{self, Repository},
    model::{Check, Run},
    paths, provider,
    state::{Lease, Store},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

/// Preserve overlays across unrelated native edits; explicit native check changes
/// supersede the old overlay and are recorded for the next semantic audit.
pub fn reconcile_milestone(
    milestone: &mut crate::model::Milestone,
    revisions: &mut [Value],
) -> Result<()> {
    let mut scopes = std::collections::BTreeMap::<String, Vec<Vec<Check>>>::new();
    let key = |diff: &Value| -> Option<String> {
        match diff["scope"].as_str() {
            Some("milestone") => Some("milestone".into()),
            Some("phase") => diff["phase_id"].as_str().map(|id| format!("phase/{id}")),
            _ => None,
        }
    };
    for record in revisions.iter() {
        for diff in record["diff"].as_array().into_iter().flatten() {
            if diff["superseded_by_native"] == true {
                continue;
            }
            let Some(scope) = key(diff) else {
                continue;
            };
            let values = scopes.entry(scope).or_default();
            values.push(serde_json::from_value(diff["before"].clone())?);
            values.push(serde_json::from_value(diff["after"].clone())?);
        }
    }
    let mut superseded = BTreeSet::new();
    for (scope, versions) in scopes {
        let target = if scope == "milestone" {
            Some(&mut milestone.verification)
        } else {
            milestone
                .phases
                .iter_mut()
                .find(|p| p.id == scope[6..])
                .map(|p| &mut p.verification)
        };
        if let Some(target) = target
            && versions.contains(target)
        {
            *target = versions.last().unwrap().clone();
        } else {
            superseded.insert(scope);
        }
    }
    for record in revisions.iter_mut() {
        for diff in record["diff"].as_array_mut().into_iter().flatten() {
            if key(diff).is_some_and(|scope| superseded.contains(&scope)) {
                diff["superseded_by_native"] = json!(true);
                diff["superseded_at"] = json!(paths::now());
            }
        }
    }
    Ok(())
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TaskChecks {
    phase_id: String,
    task_id: String,
    checks: Vec<Check>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PhaseChecks {
    phase_id: String,
    checks: Vec<Check>,
}
#[derive(Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct Changes {
    #[serde(default)]
    task_checks: Vec<TaskChecks>,
    #[serde(default)]
    phase_checks: Vec<PhaseChecks>,
    #[serde(default)]
    milestone_checks: Option<Vec<Check>>,
}

fn checks(values: &[Check]) -> Result<()> {
    if values.is_empty() || values.len() > 100 {
        bail!("invalid_revision: supply 1..100 checks; removing verification is not a repair");
    }
    for check in values {
        crate::config::validate_check(check)?;
    }
    Ok(())
}

pub fn revise(root: &Path, args: &Value) -> Result<Value> {
    let id = args["run_id"]
        .as_str()
        .context("invalid_arguments: run_id")?;
    let reason = args["reason"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 8192)
        .context("invalid_revision: explain the correction and its native requirement evidence")?;
    let changes: Changes = serde_json::from_value(json!({
        "task_checks":args.get("task_checks").cloned().unwrap_or(json!([])),
        "phase_checks":args.get("phase_checks").cloned().unwrap_or(json!([])),
        "milestone_checks":args.get("milestone_checks").cloned().unwrap_or(Value::Null),
    }))
    .context("invalid_revision: correction payload does not match the revision schema")?;
    if changes.task_checks.len() + changes.phase_checks.len() > 100 {
        bail!("invalid_revision: too many changes");
    }
    let apply = args["apply"] == true;
    let repo = Repository::discover(root)?;
    let _lease = if apply {
        Some(Lease::acquire(&repo)?)
    } else {
        None
    };
    let mut store = Store::open(&repo, false)?.context("run_not_found")?;
    let mut run = store.get(id)?;
    if store.requested(id)? == "cancel" {
        bail!("run_cancelled: process the queued cancellation before revision");
    }
    let host = run.host.as_ref().context("legacy_run_read_only")?;
    // An already accepted request is replayable even after the run has advanced.
    if apply
        && let Some(hash) = args["plan_hash"].as_str()
        && let Some(record) = host
            .verification_revisions
            .iter()
            .find(|r| r["plan_hash"] == hash)
    {
        if record["changes"] != serde_json::to_value(&changes)? || record["reason"] != reason {
            bail!("revision_conflict: hash was used for different changes");
        }
        return Ok(
            json!({"run_id":id,"applied":true,"replayed":true,"revision":record,"status":run.status}),
        );
    }
    if run.terminal() {
        bail!("run_terminal: preserve completed or cancelled runs; revision requires a live run");
    }
    let integration = Path::new(&run.integration);
    if git::head(integration)? != run.accepted_head || !git::clean(integration)? {
        bail!("recovery_conflict: reconcile the integration checkout before revising verification");
    }
    if run
        .intents
        .iter()
        .any(|i| !["accepted", "abandoned"].contains(&i.state.as_str()))
    {
        bail!("revision_busy: recover outstanding integration intents before revision");
    }
    let active: Vec<_> = run
        .attempts
        .iter()
        .filter(|a| ["issued", "claimed", "receiving", "submitted"].contains(&a.status.as_str()))
        .map(|a| a.id.clone())
        .collect();
    let mut modified = run.clone();
    let mut diff = vec![];
    let mut touched = BTreeSet::new();
    let mut task_keys = vec![];
    for update in &changes.task_checks {
        checks(&update.checks)?;
        let key = Run::key(&update.phase_id, &update.task_id);
        if !touched.insert(format!("task/{key}")) {
            bail!("invalid_revision: duplicate task update");
        }
        if !run.selected_phases.contains(&update.phase_id) || run.completed_tasks.contains(&key) {
            bail!(
                "revision_scope_violation: only pending tasks in the selected phase range can change"
            );
        }
        let task = modified
            .plans
            .get_mut(&update.phase_id)
            .and_then(|p| p.tasks.iter_mut().find(|t| t.id == update.task_id))
            .context("task_not_found")?;
        if task.verification == update.checks {
            bail!("invalid_revision: task checks did not change");
        }
        diff.push(json!({"scope":"task","phase_id":update.phase_id,"task_id":update.task_id,"before":task.verification,"after":update.checks}));
        task.verification = update.checks.clone();
        task_keys.push(key);
    }
    for update in &changes.phase_checks {
        checks(&update.checks)?;
        if !touched.insert(format!("phase/{}", update.phase_id)) {
            bail!("invalid_revision: duplicate phase update");
        }
        if !run.selected_phases.contains(&update.phase_id)
            || run.completed_phases.contains(&update.phase_id)
        {
            bail!("revision_scope_violation: only pending selected phases can change");
        }
        let phase = modified
            .milestone
            .phases
            .iter_mut()
            .find(|p| p.id == update.phase_id)
            .context("phase_not_found")?;
        if phase.verification == update.checks {
            bail!("invalid_revision: phase checks did not change");
        }
        diff.push(json!({"scope":"phase","phase_id":update.phase_id,"before":phase.verification,"after":update.checks}));
        phase.verification = update.checks.clone();
    }
    if let Some(values) = &changes.milestone_checks {
        checks(values)?;
        if run.range.bounded() {
            bail!(
                "revision_scope_violation: bounded runs cannot change milestone-wide verification"
            );
        }
        if modified.milestone.verification == *values {
            bail!("invalid_revision: milestone checks did not change");
        }
        diff.push(
            json!({"scope":"milestone","before":modified.milestone.verification,"after":values}),
        );
        modified.milestone.verification = values.clone();
    }
    if diff.is_empty() {
        bail!("invalid_revision: no verification changes supplied");
    }
    if let Some(repair) = &host.pending_repair {
        let superseded = repair
            .audit
            .iter()
            .all(|a| a.requirement == "host-verification")
            && (changes.milestone_checks.is_some()
                || changes
                    .phase_checks
                    .iter()
                    .any(|p| p.phase_id == repair.phase_id));
        if !superseded {
            bail!(
                "revision_busy: resolve the outstanding native/semantic repair before revising unrelated checks"
            );
        }
        modified.host.as_mut().unwrap().pending_repair = None;
    }
    let project = integration.join(&run.project_relative);
    let mut sources = serde_json::Map::new();
    for phase in &run.milestone.phases {
        if !run.selected_phases.contains(&phase.id) {
            continue;
        }
        let Some(expected) = host
            .source_revisions
            .get(&phase.id)
            .or_else(|| run.plans.get(&phase.id).map(|p| &p.source_hash))
        else {
            sources.insert(phase.id.clone(), json!("unopened"));
            continue;
        };
        let snapshot = provider::inspect(
            &project,
            run.milestone.framework,
            &phase.source.selector,
            &run.config,
        )?;
        if expected != &snapshot.source_hash {
            bail!("source_drift: reconcile native source before verification revision");
        }
        sources.insert(phase.id.clone(), json!(snapshot.source_hash));
    }
    let hash = paths::hash(serde_json::to_vec(
        &json!({"run":run,"changes":changes,"reason":reason,"sources":sources}),
    )?);
    let mut preview = json!({"run_id":id,"applied":false,"plan_hash":hash,"accepted_head":run.accepted_head,"reason":reason,"changes":changes,"diff":diff,"active_requests":active,"can_apply":active.is_empty(),"retained_verified_tasks":run.completed_tasks,"scope":"run-local verification overlay","next_action":"Review checks against native requirements, then apply the same changes with plan_hash. Use prepare with the same run_id afterwards."});
    if !apply {
        return Ok(preview);
    }
    if args["plan_hash"].as_str() != Some(&hash) {
        bail!("revision_conflict: preview is stale or plan_hash is missing");
    }
    if !active.is_empty() {
        bail!(
            "revision_busy: stop and revoke host work, or finish receipt processing before revising"
        );
    }
    let record = json!({"plan_hash":hash,"reason":reason,"changes":changes,"diff":diff,"at":paths::now(),"accepted_head":run.accepted_head,"preserved_verified_tasks":run.completed_tasks.len(),"superseded_verification_repair":host.pending_repair});
    let host = modified.host.as_mut().unwrap();
    host.verification_revisions.push(record.clone());
    for key in task_keys {
        host.task_retry_epochs.insert(key, run.attempts.len());
    }
    // No success, budget extension, checkbox change or Git advance is implied.
    modified.status = "paused".into();
    modified.blocker = None;
    modified.updated_at = paths::now();
    store = Store::open(&repo, true)?.unwrap();
    if store.requested(id)? == "cancel" {
        bail!("run_cancelled: cancellation arrived during revision");
    }
    store.save(&modified, "verification_revised")?;
    preview["applied"] = json!(true);
    preview["revision"] = record;
    preview["next_action"] = json!({"capability":"prepare","arguments":{"run_id":id}});
    run = modified;
    preview["status"] = json!(run.status);
    Ok(preview)
}
