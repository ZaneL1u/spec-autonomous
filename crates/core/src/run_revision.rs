//! Reviewed, CAS-protected changes to pending verification and task write sets.
//! No Git mutations.
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
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TaskWrites {
    phase_id: String,
    task_id: String,
    add: Vec<String>,
}
#[derive(Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct Changes {
    #[serde(default)]
    task_checks: Vec<TaskChecks>,
    /// Omitted when empty so earlier revision records and their hashes stay identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    task_writes: Vec<TaskWrites>,
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

/// Paths a blocked worker may have added to its task's write set without review.
pub const MAX_REQUESTED_WRITES: usize = 10;

/// Native context a worker must never edit for one phase: its context files and the
/// tracking file the coordinator alone checks off.
pub fn context_paths(snapshot: &crate::model::Snapshot) -> Vec<String> {
    snapshot
        .context_files
        .iter()
        .map(|f| f.path.clone())
        .chain(snapshot.tracking_file.clone())
        .collect()
}

fn normalize_write(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || Path::new(trimmed).is_absolute() || trimmed.starts_with('/') {
        bail!("write_scope_invalid: {raw:?} is not a project-relative path");
    }
    paths::relative(trimmed).map_err(|_| {
        anyhow::anyhow!("write_scope_invalid: {raw:?} is not a safe project-relative path")
    })?;
    let normal = trimmed
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/");
    if normal.is_empty() {
        bail!("write_scope_invalid: {raw:?} names the project root");
    }
    if normal == ".git"
        || normal.starts_with(".git/")
        || normal == ".spec-autonomous"
        || normal.starts_with(".spec-autonomous/")
    {
        bail!("write_scope_forbidden: {normal} is reserved for Git and the coordinator");
    }
    globset::Glob::new(&normal)
        .map_err(|_| anyhow::anyhow!("write_scope_invalid: {normal:?} is not a valid pattern"))?;
    Ok(normal)
}

/// Check paths to add to a pending task's write set and return the normalized ones its
/// writes do not already allow. Rejects reserved or native context paths, an unrestricted
/// task and overlap with another task's outstanding implement work (`exclude` skips the
/// attempt asking for them).
pub fn admissible_writes(
    run: &Run,
    phase_id: &str,
    task_id: &str,
    add: &[String],
    context: &[String],
    exclude: Option<&str>,
) -> Result<Vec<String>> {
    if add.is_empty() || add.len() > 100 {
        bail!("invalid_revision: supply 1..100 paths to add");
    }
    let task = run
        .plans
        .get(phase_id)
        .and_then(|p| p.tasks.iter().find(|t| t.id == task_id))
        .context("task_not_found")?;
    if task.writes.is_empty() {
        bail!(
            "write_scope_unrestricted: task {task_id} has no declared write set and may already change any file except native context"
        );
    }
    let mut added: Vec<String> = vec![];
    for raw in add {
        let path = normalize_write(raw)?;
        let matcher = globset::Glob::new(&path)?.compile_matcher();
        if let Some(hit) = context.iter().find(|c| **c == path || matcher.is_match(c)) {
            bail!(
                "write_scope_forbidden: {path} covers native context {hit}; change the spec through the native workflow instead"
            );
        }
        if crate::plan::allowed(&path, &task.writes) || added.contains(&path) {
            continue;
        }
        for attempt in run.attempts.iter().filter(|a| {
            a.kind == "implement"
                && ["issued", "claimed", "receiving", "submitted"].contains(&a.status.as_str())
                && Some(a.id.as_str()) != exclude
                && !(a.phase_id == phase_id && a.task_id == task_id)
        }) {
            let Some(other) = run
                .plans
                .get(&attempt.phase_id)
                .and_then(|p| p.tasks.iter().find(|t| t.id == attempt.task_id))
            else {
                continue;
            };
            if other
                .writes
                .iter()
                .chain(&other.reads)
                .any(|y| crate::plan::overlaps(&path, y))
            {
                bail!(
                    "write_scope_conflict: {path} overlaps task {}/{} whose request {} is outstanding; retry once it is received or revoked",
                    attempt.phase_id,
                    attempt.task_id,
                    attempt.id
                );
            }
        }
        added.push(path);
    }
    Ok(added)
}

/// Add `added` to the planned task's writes and return the ledger diff entry.
pub fn extend_task_writes(
    run: &mut Run,
    phase_id: &str,
    task_id: &str,
    added: &[String],
) -> Result<Value> {
    let task = run
        .plans
        .get_mut(phase_id)
        .and_then(|p| p.tasks.iter_mut().find(|t| t.id == task_id))
        .context("task_not_found")?;
    let before = task.writes.clone();
    task.writes.extend(added.iter().cloned());
    Ok(
        json!({"scope":"task_writes","phase_id":phase_id,"task_id":task_id,"before":before,"after":task.writes,"added":added}),
    )
}

/// Ready-to-run `run.revise` arguments granting `paths` to a task, for blockers.
pub fn grant_example(run_id: &str, phase_id: &str, task_id: &str, paths: &[String]) -> String {
    json!({"run_id":run_id,"reason":format!("Task {task_id} needs these files to finish its native requirement"),"task_writes":[{"phase_id":phase_id,"task_id":task_id,"add":paths}],"apply":true}).to_string()
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
        "task_writes":args.get("task_writes").cloned().unwrap_or(json!([])),
        "phase_checks":args.get("phase_checks").cloned().unwrap_or(json!([])),
        "milestone_checks":args.get("milestone_checks").cloned().unwrap_or(Value::Null),
    }))
    .context("invalid_revision: correction payload does not match the revision schema")?;
    if changes.task_checks.len() + changes.phase_checks.len() + changes.task_writes.len() > 100 {
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
    // Only work whose verification the revision changes blocks it: implement attempts of
    // revised tasks, and repair/audit attempts of revised phases. Other outstanding work
    // keeps running and is integrated against the revised checks.
    let revised_tasks: BTreeSet<String> = changes
        .task_checks
        .iter()
        .map(|u| Run::key(&u.phase_id, &u.task_id))
        .chain(
            changes
                .task_writes
                .iter()
                .map(|u| Run::key(&u.phase_id, &u.task_id)),
        )
        .collect();
    let revised_phases: BTreeSet<&str> = changes
        .task_checks
        .iter()
        .map(|u| u.phase_id.as_str())
        .chain(changes.phase_checks.iter().map(|u| u.phase_id.as_str()))
        .collect();
    let blocking: Vec<_> = run
        .attempts
        .iter()
        .filter(|a| ["issued", "claimed", "receiving", "submitted"].contains(&a.status.as_str()))
        .filter(|a| match a.kind.as_str() {
            "implement" => revised_tasks.contains(&Run::key(&a.phase_id, &a.task_id)),
            "converge" | "audit" => {
                changes.milestone_checks.is_some() || revised_phases.contains(a.phase_id.as_str())
            }
            _ => false,
        })
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
    let project = integration.join(&run.project_relative);
    for update in &changes.task_writes {
        let key = Run::key(&update.phase_id, &update.task_id);
        if !touched.insert(format!("writes/{key}")) {
            bail!("invalid_revision: duplicate task write update");
        }
        if !run.selected_phases.contains(&update.phase_id) || run.completed_tasks.contains(&key) {
            bail!(
                "revision_scope_violation: only pending tasks in the selected phase range can change"
            );
        }
        let phase = run
            .milestone
            .phases
            .iter()
            .find(|p| p.id == update.phase_id)
            .context("phase_not_found")?;
        let snapshot = provider::inspect(
            &project,
            run.milestone.framework,
            &phase.source.selector,
            &run.config,
        )?;
        let added = admissible_writes(
            &modified,
            &update.phase_id,
            &update.task_id,
            &update.add,
            &context_paths(&snapshot),
            None,
        )?;
        if added.is_empty() {
            bail!("invalid_revision: task writes already allow every listed path");
        }
        diff.push(extend_task_writes(
            &mut modified,
            &update.phase_id,
            &update.task_id,
            &added,
        )?);
        if !task_keys.contains(&key) {
            task_keys.push(key);
        }
    }
    if diff.is_empty() {
        bail!("invalid_revision: no verification or write-set changes supplied");
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
    let mut preview = json!({"run_id":id,"applied":false,"plan_hash":hash,"accepted_head":run.accepted_head,"reason":reason,"changes":changes,"diff":diff,"active_requests":active,"blocking_requests":blocking,"can_apply":blocking.is_empty(),"retained_verified_tasks":run.completed_tasks,"scope":"run-local verification overlay","next_action":format!("Review checks against native requirements, then apply the same changes with plan_hash. Afterwards run `spec-autonomous prepare --run-id {id}` without --change/--feature.")});
    if !apply {
        return Ok(preview);
    }
    // A supplied plan_hash must match. Without one, the revision applies directly when
    // nothing in its scope is outstanding; otherwise the preview/hash round trip stays.
    match args["plan_hash"].as_str() {
        Some(supplied) if supplied != hash => {
            bail!("revision_conflict: preview is stale or plan_hash is missing");
        }
        None if !blocking.is_empty() => {
            bail!("revision_conflict: preview is stale or plan_hash is missing");
        }
        _ => {}
    }
    if !blocking.is_empty() {
        bail!(
            "revision_busy: outstanding work on the revised scope; finish its receipt or revoke it before revising"
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

/// A new planner must not silently restore a known obsolete check list, nor drop files
/// a revision added to a task's write set (an unrestricted task keeps its open scope).
pub fn reconcile_plan(plan: &mut crate::model::Plan, revisions: &[Value]) -> Result<()> {
    for task in &mut plan.tasks {
        let mut versions = vec![];
        for record in revisions {
            for diff in record["diff"].as_array().into_iter().flatten() {
                if diff["scope"] == "task"
                    && diff["phase_id"] == plan.phase_id
                    && diff["task_id"] == task.id
                    && diff["superseded_by_native"] != true
                {
                    versions.push(serde_json::from_value::<Vec<Check>>(
                        diff["before"].clone(),
                    )?);
                    versions.push(serde_json::from_value::<Vec<Check>>(diff["after"].clone())?);
                }
            }
        }
        if !task.writes.is_empty() {
            for record in revisions {
                for diff in record["diff"].as_array().into_iter().flatten() {
                    if diff["scope"] == "task_writes"
                        && diff["phase_id"] == plan.phase_id
                        && diff["task_id"] == task.id
                        && diff["superseded_by_native"] != true
                    {
                        for path in diff["added"].as_array().into_iter().flatten() {
                            if let Some(path) = path.as_str()
                                && !crate::plan::allowed(path, &task.writes)
                            {
                                task.writes.push(path.to_owned());
                            }
                        }
                    }
                }
            }
        }
        if let Some(latest) = versions.last() {
            if versions.contains(&task.verification) {
                task.verification = latest.clone();
            } else {
                bail!(
                    "verification_revision_conflict: task {} has reviewed run-local checks; preserve their latest values or explicitly reconcile the new check contract",
                    task.id
                );
            }
        }
    }
    Ok(())
}
