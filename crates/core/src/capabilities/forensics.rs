//! Read-only anomaly report for a run: where time and attempts went, and what looks stuck.
use super::*;
use crate::model::{Attempt, Run};
use std::collections::BTreeMap;

pub fn run_forensics(root: &Path, id: &str) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let store = Store::open(&repo, false)?.context("run_not_found")?;
    let run = store.get(id)?;
    let events = store.events(id)?;
    let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let mut anomalies = vec![];
    // Same command failing more than once on this run.
    let mut failing: BTreeMap<(Vec<String>, String), Vec<usize>> = BTreeMap::new();
    for (index, e) in run.evidence.iter().enumerate() {
        if e.exit_code != 0 || !e.tree_unchanged {
            failing
                .entry((e.argv.clone(), e.cwd.clone()))
                .or_default()
                .push(index);
        }
    }
    for ((argv, cwd), indexes) in &failing {
        if indexes.len() >= 2 {
            anomalies.push(json!({"code":"repeated_failing_check","severity":"warning","argv":argv,"cwd":cwd,"count":indexes.len(),"evidence_indexes":indexes,"hint":"The same check failed repeatedly. Inspect its manifests; if the command is wrong use run.revise, otherwise the native source needs a real fix."}));
        }
    }
    // Tasks that consumed many attempts.
    let mut per_task: BTreeMap<String, Vec<&Attempt>> = BTreeMap::new();
    for a in run.attempts.iter().filter(|a| a.kind == "implement") {
        per_task
            .entry(Run::key(&a.phase_id, &a.task_id))
            .or_default()
            .push(a);
    }
    for (key, attempts) in &per_task {
        if attempts.len() >= 3 {
            let mut statuses = BTreeMap::<&str, usize>::new();
            for a in attempts {
                *statuses.entry(a.status.as_str()).or_default() += 1;
            }
            anomalies.push(json!({"code":"task_attempt_churn","severity":"warning","task":key,"attempts":attempts.len(),"statuses":statuses,"completed":run.completed_tasks.contains(key),"hint":"Many attempts on one task usually mean a wrong check, an unclear native task or in-flight work that kept being revoked."}));
        }
    }
    // Outstanding work nobody has touched for a long time.
    for a in &run.attempts {
        if matches!(a.status.as_str(), "issued" | "claimed" | "receiving")
            && let Some(lease) = run.host.as_ref().and_then(|h| h.requests.get(&a.id))
            && !lease.revoked
        {
            let age = now.saturating_sub(lease.heartbeat_at_ms);
            if age > 2 * 3600 * 1000 {
                anomalies.push(json!({"code":"idle_outstanding_request","severity":"warning","request_id":a.id,"kind":a.kind,"task_id":a.task_id,"heartbeat_age_ms":age,"owner":lease.owner,"hint":"Check the host session with request.get; if it is gone, work.revoke with host_stopped:true, otherwise let it finish."}));
            }
        }
    }
    let count = |kind: &str| events.iter().filter(|e| e["kind"] == kind).count();
    let phase_completions = count("phase_completed");
    if phase_completions > run.completed_phases.len() + run.selected_phases.len() {
        anomalies.push(json!({"code":"phases_completed_repeatedly","severity":"warning","phase_completed_events":phase_completions,"completed_phases":run.completed_phases.len(),"hint":"Phases were completed more times than they exist. Older versions reset progress on reconcile; keep the CLI current and avoid mode switches mid-run."}));
    }
    let reconciles = count("source_reconciled");
    if reconciles > 10 {
        anomalies.push(json!({"code":"frequent_source_reconciliation","severity":"info","count":reconciles,"hint":"The original checkout keeps changing during the run. Batch native edits or commit them on the integration branch."}));
    }
    let handoffs = count("handed_off");
    if handoffs > 3 {
        anomalies.push(json!({"code":"repeated_handoff","severity":"info","count":handoffs,"hint":"Many native handoffs in one run; the host may be switching between native and autonomous mode repeatedly."}));
    }
    if let Some(host) = &run.host {
        for (signature, hits) in &host.repair_signatures {
            if *hits >= 1 {
                anomalies.push(json!({"code":"repair_round_recorded","severity":"info","signature":signature,"rounds":hits}));
            }
        }
        if host.reconcile_pending {
            anomalies.push(json!({"code":"source_reconcile_pending","severity":"info","hint":"The original checkout changed while work is in flight; it is merged once the receipts land."}));
        }
    }
    if !run.stale_phases.is_empty() {
        anomalies.push(json!({"code":"stale_phases_pending","severity":"info","phases":run.stale_phases,"hint":"These completed phases owe a command gate before delivery because the tree changed underneath them."}));
    }
    let worktrees_present = run
        .attempts
        .iter()
        .filter(|a| Path::new(&a.worktree).exists() && a.worktree != run.integration)
        .count();
    let reclaimed = run
        .attempts
        .iter()
        .filter(|a| a.worktree_state.as_deref() == Some("reclaimed"))
        .count();
    if worktrees_present > 25 {
        anomalies.push(json!({"code":"worktree_backlog","severity":"warning","present":worktrees_present,"hint":"Many attempt worktrees still exist. Set execution.reclaim_worktrees = \"terminal\" (default) or run run.cleanup with active:true."}));
    }
    let reused = run
        .evidence
        .iter()
        .filter(|e| {
            e.details
                .as_ref()
                .is_some_and(|d| d.get("reused_from").is_some())
        })
        .count();
    let elapsed_ms = events.first().zip(events.last()).and_then(|(first, last)| {
        let a = chrono::DateTime::parse_from_rfc3339(first["time"].as_str()?).ok()?;
        let b = chrono::DateTime::parse_from_rfc3339(last["time"].as_str()?).ok()?;
        Some((b.timestamp_millis() - a.timestamp_millis()).max(0) as u64)
    });
    Ok(json!({
        "run_id": id,
        "status": run.status,
        "stage": run.stage,
        "anomalies": anomalies,
        "metrics": {
            "attempts": run.attempts.len(),
            "attempts_by_kind_status": run.attempts.iter().fold(BTreeMap::<String, usize>::new(), |mut m, a| { *m.entry(format!("{}:{}", a.kind, a.status)).or_default() += 1; m }),
            "verification_checks": run.evidence.len(),
            "reused_checks": reused,
            "executed_checks": run.evidence.len() - reused,
            "failed_checks": run.evidence.iter().filter(|e| e.exit_code != 0).count(),
            "phase_completed_events": phase_completions,
            "source_reconciliations": reconciles,
            "handoffs": handoffs,
            "events": events.len(),
            "ledger_elapsed_ms": elapsed_ms,
            "worktrees_present": worktrees_present,
            "worktrees_reclaimed": reclaimed,
            "repair_rounds": run.repair_rounds,
        },
        "read_only": true,
    }))
}
