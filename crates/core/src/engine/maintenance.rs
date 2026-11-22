//! Reuse only closed, exclusively owned read-only checkouts; contexts stay fresh.
use super::*;

impl Coordinator<'_> {
    pub(super) fn review_checkout(&self) -> Result<Option<(PathBuf, String)>> {
        let inventory = self.repo.worktrees()?;
        let runs = self.store.list()?;
        for a in self
            .run
            .attempts
            .iter()
            .rev()
            .filter(|a| matches!(a.kind.as_str(), "plan-tasks" | "audit"))
        {
            let path = Path::new(&a.worktree);
            let Some(wt) = inventory.iter().find(|w| w.path == a.worktree) else {
                continue;
            };
            if wt.locked
                || wt.prunable
                || wt.branch != a.branch
                || wt.head != a.base_commit
                || path == self.repo.root
                || a.worktree == self.run.integration
                || path.canonicalize().ok().as_deref() != Some(path)
                || !self
                    .repo
                    .in_worktree_roots(&self.run.config.execution.worktree_root, path)
                || path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_none_or(|name| a.branch != format!("codex/sa/{name}"))
            {
                continue;
            }
            let users: Vec<_> = self
                .run
                .attempts
                .iter()
                .filter(|old| old.worktree == a.worktree)
                .collect();
            let stopped = users.iter().all(|old| {
                matches!(old.kind.as_str(), "plan-tasks" | "audit")
                    && matches!(
                        old.status.as_str(),
                        "accepted" | "failed" | "revoked" | "superseded"
                    )
                    && !old
                        .pid
                        .is_some_and(|pid| !process::identity(pid).is_empty())
                    && !self
                        .run
                        .host
                        .as_ref()
                        .and_then(|h| h.requests.get(&old.id))
                        .is_some_and(|l| {
                            l.owner.is_some() && !l.revoked && l.receipt_hash.is_none()
                        })
            });
            let shared = runs.iter().filter(|r| r.id != self.run.id).any(|r| {
                r.integration == a.worktree
                    || r.attempts.iter().any(|old| old.worktree == a.worktree)
                    || r.intents.iter().any(|old| old.worktree == a.worktree)
            });
            // Ignored files block reuse unless `worker_prepare` left exactly those entries
            // (a synced .venv); the idempotent hook runs again when the checkout is reused.
            let prepared: Vec<String> = self
                .store
                .attempt_dir(&self.run.id, &a.id)
                .ok()
                .and_then(|dir| fs::read(dir.join(super::PREPARED_IGNORED)).ok())
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
            if !stopped
                || shared
                || !git::clean(path)?
                || git::ignored_entries(path)?
                    .iter()
                    .any(|entry| !prepared.contains(entry))
            {
                continue;
            }
            // checkout without --force refuses untracked collisions. Old receipts
            // record their base revision; checkout contents are never historical proof.
            git::command(path, &["checkout", "--quiet", &a.branch], None)?;
            git::command(path, &["reset", "--keep", &self.run.accepted_head], None)?;
            return Ok(Some((path.into(), a.branch.clone())));
        }
        Ok(None)
    }
}
