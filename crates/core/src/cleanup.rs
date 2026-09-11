//! Explicit cleanup preserves branch refs and all dirty/locked/active worktrees.
use crate::{
    git::{self, Repository},
    paths,
    state::{Lease, Store},
};
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::{collections::BTreeSet, path::Path};

pub fn cleanup(root: &Path, id: &str) -> Result<serde_json::Value> {
    let repo = Repository::discover(root)?;
    let _lease = Lease::acquire(&repo)?;
    let store = Store::open(&repo, false)?.context("run_not_found")?;
    let run = store.get(id)?;
    if !run.terminal() {
        bail!("run_active: only terminal runs can be cleaned");
    }
    let inventory = repo.worktrees()?;
    let mut candidates: BTreeSet<String> = run
        .attempts
        .iter()
        .filter(|a| a.status == "integrated" || a.status == "accepted")
        .map(|a| a.worktree.clone())
        .collect();
    candidates.extend(
        run.intents
            .iter()
            .filter(|i| i.state == "accepted")
            .map(|i| i.worktree.clone()),
    );
    candidates.remove(&run.integration);
    let mut removable = vec![];
    let mut retained = vec![];
    for candidate in candidates {
        let path = Path::new(&candidate);
        if !path.exists() {
            continue;
        }
        let relative = paths::localize(&repo.common, path)?;
        if !relative.starts_with("spec-autonomous/worktrees/") {
            bail!("cleanup_scope_violation: not a managed worktree");
        }
        let wt = inventory
            .iter()
            .find(|w| w.path == candidate)
            .context("cleanup_scope_violation: worktree is not in Git inventory")?;
        if !wt.branch.starts_with("codex/sa/") {
            bail!("cleanup_scope_violation: branch ownership mismatch");
        }
        if wt.locked || !git::clean(path)? {
            retained.push(candidate);
        } else {
            removable.push(candidate);
        }
    }
    for path in &removable {
        git::command(&repo.root, &["worktree", "remove", "--", path], None)?;
    }
    Ok(
        json!({"removed":removable,"retained":retained,"integration_retained":run.integration,"branch_refs_retained":true,"evidence_retained":true}),
    )
}
