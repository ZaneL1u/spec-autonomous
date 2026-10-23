//! Automatic reclamation of closed attempt and candidate worktrees. Branch refs, the
//! attempt directory (input, result, patch) and all evidence stay; only the checkout goes.
use crate::{
    git::{self, Repository},
    model::Run,
    paths,
    state::Store,
};
use anyhow::Result;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

fn closed(status: &str) -> bool {
    matches!(
        status,
        "integrated" | "superseded" | "revoked" | "operator_resolved"
    )
}

/// Remove the worktrees of closed attempts and accepted intents. Returns what was
/// removed and why anything eligible was kept. Never touches the integration checkout,
/// the original checkout, live or submitted work, or a worktree outside the runtime.
pub fn reclaim(repo: &Repository, store: &Store, run: &mut Run) -> Result<Value> {
    let mut removed = vec![];
    let mut retained = vec![];
    if run.config.execution.reclaim_worktrees == "never" || !run.config.execution.automatic_cleanup
    {
        return Ok(json!({"removed":removed,"retained":retained,"policy":"never"}));
    }
    let runtime = repo.runtime()?;
    let worktrees = runtime.join("worktrees");
    let keep_failed = run.config.execution.retain_failed_worktrees as usize;
    // Newest failed attempts per open task stay around for inspection.
    let mut failed_rank: BTreeMap<String, usize> = BTreeMap::new();
    let mut keep_failed_ids = std::collections::BTreeSet::new();
    for attempt in run.attempts.iter().rev() {
        if attempt.kind == "implement" && attempt.status == "failed" {
            let key = Run::key(&attempt.phase_id, &attempt.task_id);
            let seen = failed_rank.entry(key.clone()).or_insert(0);
            if *seen < keep_failed && !run.completed_tasks.contains(&key) {
                keep_failed_ids.insert(attempt.id.clone());
            }
            *seen += 1;
        }
    }
    let live_lease = |run: &Run, id: &str| {
        run.host
            .as_ref()
            .and_then(|h| h.requests.get(id))
            .is_some_and(|l| l.owner.is_some() && !l.revoked && l.receipt_hash.is_none())
    };
    for index in 0..run.attempts.len() {
        let attempt = run.attempts[index].clone();
        if attempt.worktree_state.as_deref() == Some("reclaimed")
            || attempt.worktree == run.integration
            || attempt.worktree == run.origin
            || !Path::new(&attempt.worktree).starts_with(&worktrees)
        {
            continue;
        }
        // Accepted read-only checkouts (planning, audit, native planning) stay for
        // review reuse and handoff maintenance; this pass owns writing work.
        let eligible = closed(&attempt.status)
            || (attempt.status == "failed" && !keep_failed_ids.contains(&attempt.id));
        if !eligible || live_lease(run, &attempt.id) {
            continue;
        }
        if !Path::new(&attempt.worktree).exists() {
            // Someone else removed the checkout; the stored patch still fixes the fingerprint.
            if attempt.patch_hash.is_none()
                && let Ok(dir) = store.attempt_dir(&run.id, &attempt.id)
                && let Ok(patch) = fs::read(dir.join("patch.diff"))
            {
                run.attempts[index].patch_hash = Some(paths::hash(patch));
            }
            run.attempts[index].worktree_state = Some("reclaimed".into());
            continue;
        }
        // Persist the patch first: a failed or revoked worker's uncommitted edits exist
        // nowhere else, and retry fingerprints read the hash instead of the checkout.
        let dir = store.attempt_dir(&run.id, &attempt.id)?;
        let patch = match git::patch(Path::new(&attempt.worktree), &attempt.base_commit) {
            Ok(patch) => patch,
            Err(error) => {
                retained.push(json!({"path":attempt.worktree,"reason":"patch_unavailable","error":error.to_string()}));
                continue;
            }
        };
        let hash = paths::hash(&patch);
        if !dir.join("patch.diff").exists() {
            paths::atomic_write(&dir.join("patch.diff"), patch.as_bytes())?;
        }
        run.attempts[index].patch_hash = Some(hash);
        match git::command(
            &repo.root,
            &["worktree", "remove", "--force", &attempt.worktree],
            None,
        ) {
            Ok(_) => {
                run.attempts[index].worktree_state = Some("reclaimed".into());
                removed.push(json!({"path":attempt.worktree,"attempt_id":attempt.id,"status":attempt.status,"branch":attempt.branch}));
            }
            Err(error) => {
                retained.push(json!({"path":attempt.worktree,"reason":"remove_failed","error":error.to_string()}));
            }
        }
    }
    for index in 0..run.intents.len() {
        let intent = run.intents[index].clone();
        // A direct candidate shares the worker checkout; the attempt pass owns that one
        // so its patch is stored before anything is removed.
        if !matches!(intent.state.as_str(), "accepted" | "abandoned")
            || intent.worktree == run.integration
            || !Path::new(&intent.worktree).starts_with(&worktrees)
            || !Path::new(&intent.worktree).exists()
            || run.attempts.iter().any(|a| a.worktree == intent.worktree)
        {
            continue;
        }
        if intent.state == "abandoned" && !git::clean(Path::new(&intent.worktree)).unwrap_or(false)
        {
            retained.push(json!({"path":intent.worktree,"reason":"abandoned_candidate_dirty"}));
            continue;
        }
        match git::command(
            &repo.root,
            &["worktree", "remove", "--force", &intent.worktree],
            None,
        ) {
            Ok(_) => removed
                .push(json!({"path":intent.worktree,"intent_id":intent.id,"state":intent.state})),
            Err(error) => retained.push(
                json!({"path":intent.worktree,"reason":"remove_failed","error":error.to_string()}),
            ),
        }
    }
    if !removed.is_empty() {
        let _ = git::command(&repo.root, &["worktree", "prune"], None);
    }
    Ok(json!({"removed":removed,"retained":retained,"policy":"terminal"}))
}

/// Attempt directories of reclaimed worktrees keep the patch; expose whether it exists.
pub fn patch_recorded(repo: &Repository, run: &str, attempt: &str) -> bool {
    repo.runtime()
        .map(|runtime| {
            fs::metadata(runtime.join(format!("runs/{run}/attempts/{attempt}/patch.diff"))).is_ok()
        })
        .unwrap_or(false)
}
