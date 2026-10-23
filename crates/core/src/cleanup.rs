//! Reviewed cleanup of terminal run resources. The ledger and integration are retained.
use crate::{
    git::{self, Repository},
    model::Run,
    paths, process,
    state::{Lease, Store},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

/// Historical CLI entry: remove safe worktrees immediately, preserving every branch.
pub fn cleanup(root: &Path, id: &str) -> Result<Value> {
    execute(root, id, true, CleanupScope::default(), None, false)
}

/// Capability entry: preview first, then apply the exact observed plan.
pub fn cleanup_with_options(
    root: &Path,
    id: &str,
    apply: bool,
    delete_branches: bool,
    plan_hash: Option<&str>,
) -> Result<Value> {
    cleanup_with_scope(root, id, apply, delete_branches, false, plan_hash)
}
pub fn cleanup_with_scope(
    root: &Path,
    id: &str,
    apply: bool,
    delete_branches: bool,
    delete_integration: bool,
    plan_hash: Option<&str>,
) -> Result<Value> {
    cleanup_reviewed(
        root,
        id,
        apply,
        CleanupScope {
            delete_branches,
            delete_integration,
            active: false,
        },
        plan_hash,
    )
}

#[derive(Default, Clone, Copy)]
pub struct CleanupScope {
    pub delete_branches: bool,
    pub delete_integration: bool,
    pub active: bool,
}

pub fn cleanup_reviewed(
    root: &Path,
    id: &str,
    apply: bool,
    scope: CleanupScope,
    plan_hash: Option<&str>,
) -> Result<Value> {
    execute(root, id, apply, scope, plan_hash, true)
}

// Immutable packets can refer to prior worktrees. Unknown references fail closed.
fn active_references(repo: &Repository, run: &Run) -> Result<String> {
    // A historical cwd is provenance, not a live dependency. Only actual
    // evidence storage paths must survive removal of the old checkout.
    let mut references = String::new();
    for evidence in &run.evidence {
        references.push_str(&evidence.log);
        if let Some(details) = &evidence.details {
            for key in ["manifest", "stdout", "stderr", "output_directory"] {
                if let Some(path) = details[key].as_str() {
                    references.push_str(path);
                }
            }
        }
    }
    for a in run.attempts.iter().filter(|a| !terminal_attempt(&a.status)) {
        let base = format!("spec-autonomous/runs/{}/attempts/{}", run.id, a.id);
        references.push_str(&paths::read(
            &repo.common,
            &format!("{base}/input.json"),
            16 * 1024 * 1024,
        )?);
        let full = format!("{base}/context-full.json");
        if paths::inside(&repo.common, &full)?.exists() {
            references.push_str(&paths::read(&repo.common, &full, 16 * 1024 * 1024)?);
        }
    }
    Ok(references)
}

#[derive(Clone)]
struct Candidate {
    branch: String,
    reason: Option<&'static str>,
}

fn terminal_attempt(status: &str) -> bool {
    matches!(
        status,
        "accepted" | "integrated" | "failed" | "revoked" | "superseded" | "operator_resolved"
    )
}

fn owned(repo: &Repository, path: &str, branch: &str) -> bool {
    let Ok(relative) = paths::localize(&repo.common, Path::new(path)) else {
        return false;
    };
    let Some(name) = relative.strip_prefix("spec-autonomous/worktrees/") else {
        return false;
    };
    paths::valid_id(name).is_ok() && branch == format!("codex/sa/{name}")
}

fn branch_head(repo: &Repository, branch: &str) -> Option<String> {
    git::command(
        &repo.root,
        &[
            "show-ref",
            "--verify",
            "--hash",
            &format!("refs/heads/{branch}"),
        ],
        None,
    )
    .ok()
    .map(|v| v.trim().to_owned())
}

fn plan(repo: &Repository, run: &Run, runs: &[Run], delete_branches: bool) -> Result<Value> {
    let inventory = repo.worktrees()?;
    let references = if run.terminal() {
        Ok(String::new())
    } else {
        active_references(repo, run)
    };
    let mut candidates: BTreeMap<String, Candidate> = BTreeMap::new();
    for a in &run.attempts {
        let lease_active = run
            .host
            .as_ref()
            .and_then(|h| h.requests.get(&a.id))
            .is_some_and(|l| l.owner.is_some() && !l.revoked && l.receipt_hash.is_none());
        let process_active = a.pid.is_some_and(|pid| !process::identity(pid).is_empty());
        let reason = if lease_active || process_active {
            Some("owner_not_stopped")
        } else if !run.terminal() && a.status == "accepted" {
            Some("recovery_worktree_retained")
        } else if !run.terminal()
            && a.kind == "implement"
            && a.status == "failed"
            && !run
                .completed_tasks
                .contains(&Run::key(&a.phase_id, &a.task_id))
        {
            // Retry/no-progress logic still fingerprints this checkout's patch.
            Some("retry_evidence_retained")
        } else if !terminal_attempt(&a.status) {
            Some("attempt_not_terminal")
        } else {
            None
        };
        let candidate = Candidate {
            branch: a.branch.clone(),
            reason,
        };
        if let Some(old) = candidates.get_mut(&a.worktree) {
            if old.branch != candidate.branch {
                old.reason = Some("ownership_conflict");
            } else if candidate.reason.is_some() {
                old.reason = candidate.reason;
            }
        } else {
            candidates.insert(a.worktree.clone(), candidate);
        }
    }
    for intent in &run.intents {
        let candidate = Candidate {
            branch: format!("codex/sa/{}", intent.id),
            reason: if matches!(intent.state.as_str(), "accepted" | "abandoned") {
                None
            } else {
                Some("intent_not_terminal")
            },
        };
        candidates
            .entry(intent.worktree.clone())
            .or_insert(candidate);
    }
    let mut worktrees = vec![];
    let mut branches = vec![];
    let mut retained = vec![];
    for (path, c) in candidates {
        let wt = inventory.iter().find(|w| w.path == path);
        let shared = runs.iter().filter(|r| r.id != run.id).any(|r| {
            r.integration == path
                || r.integration_branch == c.branch
                || r.attempts
                    .iter()
                    .any(|a| a.worktree == path || a.branch == c.branch)
                || r.intents
                    .iter()
                    .any(|i| i.worktree == path || format!("codex/sa/{}", i.id) == c.branch)
        });
        let reason = if path == run.integration || c.branch == run.integration_branch {
            Some("integration_retained")
        } else if Path::new(&path) == repo.root {
            Some("current_worktree_retained")
        } else if !owned(repo, &path, &c.branch) {
            Some("ownership_mismatch")
        } else if shared {
            Some("shared_resource")
        } else if c.reason.is_some() {
            c.reason
        } else if wt.is_some_and(|w| w.branch != c.branch) {
            Some("branch_changed")
        } else if wt.is_some_and(|w| w.locked) {
            Some("locked")
        } else if wt.is_some_and(|w| w.prunable) {
            Some("prunable_registration")
        } else if wt.is_none() && Path::new(&path).exists() {
            Some("not_in_git_inventory")
        } else if wt.is_some() && !Path::new(&path).exists() {
            Some("missing_worktree")
        } else if !run.terminal() && references.is_err() {
            Some("active_references_unavailable")
        } else if !run.terminal()
            && references.as_ref().is_ok_and(|r| {
                r.contains(&path) || r.contains(&serde_json::to_string(&path).unwrap_or_default())
            })
        {
            Some("active_reference_retained")
        } else if !run.terminal()
            && wt.is_some()
            && !git::command(
                Path::new(&path),
                &["ls-files", "--others", "--ignored", "--exclude-standard"],
                None,
            )?
            .is_empty()
        {
            Some("ignored_artifacts_retained")
        } else if wt.is_some() && !git::clean(Path::new(&path))? {
            Some("dirty")
        } else {
            None
        };
        if let Some(reason) = reason {
            retained.push(json!({"path":path,"branch":c.branch,"reason":reason}));
            continue;
        }
        if let Some(wt) = wt {
            worktrees.push(json!({"path":path,"branch":c.branch,"head":wt.head}));
        }
        if delete_branches && let Some(head) = branch_head(repo, &c.branch) {
            let checked_out_elsewhere = inventory
                .iter()
                .any(|w| w.branch == c.branch && w.path != path);
            if checked_out_elsewhere || !git::ancestor(&repo.root, &head, &run.accepted_head) {
                retained.push(json!({"worktree":path,"branch":c.branch,"reason":if checked_out_elsewhere {"branch_checked_out"} else {"branch_unmerged"}}));
            } else {
                branches.push(json!({"branch":c.branch,"head":head}));
            }
        }
    }
    Ok(
        json!({"run_id":run.id,"updated_at":run.updated_at,"accepted_head":run.accepted_head,
        "delete_branches":delete_branches,"worktrees":worktrees,"branches":branches,
        "retained_details":retained,"integration_retained":run.integration,"evidence_retained":true}),
    )
}

fn execute(
    root: &Path,
    id: &str,
    apply: bool,
    scope: CleanupScope,
    expected: Option<&str>,
    reviewed: bool,
) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let _lease = if apply {
        Some(Lease::acquire(&repo)?)
    } else {
        None
    };
    let store = Store::open(&repo, false)?.context("run_not_found")?;
    let run = store.get(id)?;
    if !run.terminal() && !scope.active {
        bail!(
            "run_active: preview with active:true to clean closed work while preserving live work"
        );
    }
    if !run.terminal() && scope.delete_integration {
        bail!("run_active: active integration must be retained");
    }
    let delete_branches = scope.delete_branches;
    let delete_integration = scope.delete_integration;
    let mut result = plan(&repo, &run, &store.list()?, delete_branches)?;
    result["active"] = json!(scope.active);
    result["delete_integration"] = json!(delete_integration);
    if delete_integration {
        let inventory = repo.worktrees()?;
        let entry = inventory.iter().find(|w| w.path == run.integration);
        let shared = store.list()?.iter().filter(|r| r.id != run.id).any(|r| {
            r.integration == run.integration
                || r.attempts.iter().any(|a| a.worktree == run.integration)
                || r.intents.iter().any(|i| i.worktree == run.integration)
        });
        let safe = owned(&repo, &run.integration, &run.integration_branch)
            && Path::new(&run.integration) != repo.root
            && !shared
            && entry.is_some_and(|w| {
                w.branch == run.integration_branch
                    && w.head == run.accepted_head
                    && !w.locked
                    && !w.prunable
            })
            && git::clean(Path::new(&run.integration)).unwrap_or(false);
        result["integration_removal"] =
            json!({"path":run.integration,"head":entry.map(|w|&w.head),"eligible":safe});
    }
    let hash = paths::hash(serde_json::to_vec(&result)?);
    // Removing closed worktrees keeps every ref and all evidence, so it may apply without
    // a preview; deleting branches or the integration still needs the reviewed hash.
    let destructive = delete_branches || delete_integration;
    if apply && reviewed && (expected.is_some() || destructive) && expected != Some(hash.as_str()) {
        bail!(
            "source_drift: cleanup preview scope changed (delete_branches={delete_branches}, delete_integration={delete_integration}); re-run run.cleanup with the same scope and apply its new plan_hash"
        );
    }
    result["plan_hash"] = json!(hash);
    result["applied"] = json!(apply);
    let mut removed = vec![];
    let mut removed_branches = vec![];
    let mut retained = result["retained_details"].as_array().unwrap().clone();
    if apply {
        if delete_integration {
            let integration = Path::new(&run.integration);
            if result["integration_removal"]["eligible"] != true
                || integration == repo.root
                || !git::clean(integration)?
                || git::head(integration)? != run.accepted_head
            {
                retained.push(json!({"path":run.integration,"branch":run.integration_branch,"reason":"integration_not_safe_to_remove"}));
            } else if git::command(
                &repo.root,
                &["worktree", "remove", "--", &run.integration],
                None,
            )
            .is_ok()
            {
                removed.push(run.integration.clone());
            } else {
                retained.push(json!({"path":run.integration,"branch":run.integration_branch,"reason":"integration_remove_failed"}));
            }
        }
        for item in result["worktrees"].as_array().unwrap() {
            let path = item["path"].as_str().unwrap();
            // Git repeats dirtiness/lock checks at mutation time; never force removal.
            let unchanged = git::head(Path::new(path)).is_ok_and(|h| h == item["head"]);
            let removal = unchanged
                && git::command(&repo.root, &["worktree", "remove", "--", path], None).is_ok();
            if removal {
                removed.push(path.to_owned());
            } else {
                retained.push(json!({"path":path,"branch":item["branch"],"reason":"worktree_changed_or_remove_failed"}));
            }
        }
        for item in result["branches"].as_array().unwrap() {
            let branch = item["branch"].as_str().unwrap();
            let head = item["head"].as_str().unwrap();
            let inventory = repo.worktrees()?;
            let safe = !inventory.iter().any(|w| w.branch == branch)
                && git::ancestor(&repo.root, head, &run.accepted_head);
            // Compare-and-swap deletion cannot remove a ref moved since preview.
            if safe
                && git::command(
                    &repo.root,
                    &["update-ref", "-d", &format!("refs/heads/{branch}"), head],
                    None,
                )
                .is_ok()
            {
                removed_branches.push(branch.to_owned());
            } else {
                retained.push(json!({"branch":branch,"reason":"branch_changed_or_in_use"}));
            }
        }
    }
    result["removed"] = json!(removed);
    result["removed_branches"] = json!(removed_branches);
    result["branch_refs_retained"] = json!(removed_branches.is_empty());
    result["retained"] = json!(
        retained
            .iter()
            .filter_map(|r| r["path"].as_str())
            .collect::<Vec<_>>()
    );
    result["retained_details"] = json!(retained);
    Ok(result)
}
