//! Cleanup contracts use real Git registrations, refs, files and durable run state.
use serde_json::{Value, json};
use spec_autonomous_core::{
    cleanup,
    config::Config,
    git::{self, Repository},
    model::{Attempt, Run},
    state::Store,
};
use std::{fs, path::Path};

struct Fixture {
    _temp: tempfile::TempDir,
    repo: Repository,
    run: Run,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        git::command(temp.path(), &["init", "-q", "-b", "main"], None).unwrap();
        fs::write(temp.path().join("README.md"), "fixture\n").unwrap();
        let head = git::commit(temp.path(), "initial").unwrap();
        let repo = Repository::discover(temp.path()).unwrap();
        let (integration, branch) = repo.add_worktree("run-cleanup", &head).unwrap();
        let run = serde_json::from_value(json!({
            "schema_version":1,"id":"run-cleanup","milestone":{"schema_version":1,"id":"M1","goal":"Cleanup","framework":"openspec","revision":1,"phases":[],"verification":[]},
            "mode":"autonomous","range":{},"selected_phases":[],"origin":repo.root,"origin_head":head,"origin_branch":"main","project_relative":".",
            "integration":integration,"integration_branch":branch,"accepted_head":head,"status":"cancelled","stage":"executing_phase","current_phase":null,
            "started_at":"2026-09-13T00:00:00Z","updated_at":"2026-09-13T00:00:00Z","elapsed_ms":0,"blocker":null,"config":Config::default()
        })).unwrap();
        Self {
            _temp: temp,
            repo,
            run,
        }
    }
    fn attempt(&mut self, name: &str, status: &str) -> Attempt {
        let (worktree, branch) = self
            .repo
            .add_worktree(name, &self.run.accepted_head)
            .unwrap();
        let a: Attempt = serde_json::from_value(json!({"id":name,"task_id":name,"phase_id":"P1","kind":"implement","worktree":worktree,"branch":branch,"base_commit":self.run.accepted_head,"status":status,"started_at":"2026-09-13T00:00:00Z","finished_at":"2026-09-13T00:01:00Z","summary":"test","error":null,"pid":null,"process_identity":null})).unwrap();
        self.run.attempts.push(a.clone());
        a
    }
    fn save(&self) {
        Store::open(&self.repo, true)
            .unwrap()
            .unwrap()
            .save(&self.run, "fixture")
            .unwrap();
    }
    fn preview(&self, branches: bool) -> Value {
        cleanup::cleanup_with_options(&self.repo.root, &self.run.id, false, branches, None).unwrap()
    }
    fn apply(&self, preview: &Value, branches: bool) -> Value {
        cleanup::cleanup_with_options(
            &self.repo.root,
            &self.run.id,
            true,
            branches,
            preview["plan_hash"].as_str(),
        )
        .unwrap()
    }
}

#[test]
fn failed_revoked_and_accepted_worktrees_are_reviewed_without_losing_evidence() {
    let mut f = Fixture::new();
    for (name, status) in [
        ("failed", "failed"),
        ("revoked", "revoked"),
        ("accepted", "accepted"),
        ("superseded", "superseded"),
    ] {
        f.attempt(name, status);
    }
    f.save();
    let evidence = f
        .repo
        .runtime()
        .unwrap()
        .join("runs/run-cleanup/attempts/failed/evidence.json");
    fs::create_dir_all(evidence.parent().unwrap()).unwrap();
    fs::write(&evidence, "durable evidence").unwrap();
    let before = git::command(&f.repo.root, &["show-ref"], None).unwrap();
    let preview = f.preview(false);
    assert_eq!(preview["worktrees"].as_array().unwrap().len(), 4);
    assert_eq!(preview["removed"], json!([]));
    assert!(
        f.run
            .attempts
            .iter()
            .all(|a| Path::new(&a.worktree).exists())
    );
    let result = f.apply(&preview, false);
    assert_eq!(result["removed"].as_array().unwrap().len(), 4);
    assert_eq!(
        git::command(&f.repo.root, &["show-ref"], None).unwrap(),
        before
    );
    assert_eq!(fs::read_to_string(evidence).unwrap(), "durable evidence");
    assert!(Path::new(&f.run.integration).exists());
    assert_eq!(
        Store::open(&f.repo, false)
            .unwrap()
            .unwrap()
            .get(&f.run.id)
            .unwrap()
            .attempts
            .len(),
        4
    );
}

#[test]
fn terminal_run_still_protects_live_owners_dirty_locked_unknown_and_shared_work() {
    let mut f = Fixture::new();
    let dirty = f.attempt("dirty", "failed");
    fs::write(Path::new(&dirty.worktree).join("user.txt"), "keep").unwrap();
    let locked = f.attempt("locked", "revoked");
    git::command(
        &f.repo.root,
        &["worktree", "lock", "--", &locked.worktree],
        None,
    )
    .unwrap();
    let claimed = f.attempt("claimed", "claimed");
    let owner = f.attempt("owner", "failed");
    f.run.host = Some(serde_json::from_value(json!({"protocol_version":1,"requests":{"owner":{"token":"token","input_hash":"hash","request_key":"key","issued_at_ms":0,"heartbeat_at_ms":0,"owner":{"host_id":"host","session_id":"session","fresh_context":true},"receipt_hash":null,"revoked":false}}})).unwrap());
    let unknown = f.attempt("unknown", "mystery");
    let foreign = f.attempt("foreign", "failed");
    f.run.attempts.last_mut().unwrap().branch = "codex/sa/different-owner".into();
    let shared = f.attempt("shared", "accepted");
    f.save();
    let mut other = f.run.clone();
    other.id = "run-other".into();
    other.attempts = vec![shared.clone()];
    Store::open(&f.repo, true)
        .unwrap()
        .unwrap()
        .save(&other, "fixture")
        .unwrap();
    let result = f.apply(&f.preview(true), true);
    assert_eq!(result["removed"], json!([]));
    for a in [dirty, locked, claimed, owner, unknown, foreign, shared] {
        assert!(Path::new(&a.worktree).exists());
    }
    let reasons: Vec<_> = result["retained_details"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["reason"].as_str().unwrap())
        .collect();
    for reason in [
        "dirty",
        "locked",
        "attempt_not_terminal",
        "owner_not_stopped",
        "ownership_mismatch",
        "shared_resource",
    ] {
        assert!(reasons.contains(&reason), "{reasons:?}");
    }
}

#[test]
fn cleanup_rejects_stale_or_missing_hash_before_any_removal() {
    let mut f = Fixture::new();
    let a = f.attempt("worker", "failed");
    f.save();
    let preview = f.preview(true);
    assert!(
        cleanup::cleanup_with_options(&f.repo.root, &f.run.id, true, true, None)
            .unwrap_err()
            .to_string()
            .starts_with("source_drift:")
    );
    fs::write(Path::new(&a.worktree).join("new.txt"), "new evidence").unwrap();
    let err = cleanup::cleanup_with_options(
        &f.repo.root,
        &f.run.id,
        true,
        true,
        preview["plan_hash"].as_str(),
    )
    .unwrap_err();
    assert!(err.to_string().starts_with("source_drift:"));
    assert!(Path::new(&a.worktree).exists());
    fs::remove_file(Path::new(&a.worktree).join("new.txt")).unwrap();
    fs::write(Path::new(&a.worktree).join("commit.txt"), "new commit").unwrap();
    git::commit(Path::new(&a.worktree), "user commit").unwrap();
    assert!(
        cleanup::cleanup_with_options(
            &f.repo.root,
            &f.run.id,
            true,
            true,
            preview["plan_hash"].as_str()
        )
        .is_err()
    );
    assert!(Path::new(&a.worktree).exists());
}

#[test]
fn optional_branch_cleanup_deletes_only_owned_merged_unchecked_refs() {
    let mut f = Fixture::new();
    let merged = f.attempt("merged", "accepted");
    let unmerged = f.attempt("unmerged", "failed");
    fs::write(
        Path::new(&unmerged.worktree).join("candidate.txt"),
        "unique candidate evidence",
    )
    .unwrap();
    let unique = git::commit(Path::new(&unmerged.worktree), "candidate").unwrap();
    let checked = f.attempt("checked-elsewhere", "accepted");
    git::command(
        &f.repo.root,
        &["worktree", "remove", "--", &checked.worktree],
        None,
    )
    .unwrap();
    let outside_temp = tempfile::tempdir().unwrap();
    let outside = outside_temp.path().join("external");
    git::command(
        &f.repo.root,
        &[
            "worktree",
            "add",
            "--",
            outside.to_str().unwrap(),
            &checked.branch,
        ],
        None,
    )
    .unwrap();
    f.save();
    let result = f.apply(&f.preview(true), true);
    assert_eq!(result["removed_branches"], json!([merged.branch]));
    assert!(!Path::new(&merged.worktree).exists());
    assert!(!Path::new(&unmerged.worktree).exists());
    assert_eq!(
        git::command(&f.repo.root, &["rev-parse", &unmerged.branch], None)
            .unwrap()
            .trim(),
        unique
    );
    assert!(outside.exists());
    assert!(Path::new(&f.run.integration).exists());
    git::command(
        &f.repo.root,
        &["worktree", "remove", "--", outside.to_str().unwrap()],
        None,
    )
    .unwrap();
    // A previously removed worktree does not prevent a later safe branch cleanup.
    let next = f.apply(&f.preview(true), true);
    assert_eq!(next["removed_branches"], json!([checked.branch]));
}

#[test]
fn nonterminal_runs_refuse_cleanup_and_legacy_cleanup_preserves_refs() {
    let mut f = Fixture::new();
    let a = f.attempt("worker", "failed");
    f.run.status = "running".into();
    f.save();
    assert!(
        cleanup::cleanup(&f.repo.root, &f.run.id)
            .unwrap_err()
            .to_string()
            .starts_with("run_active:")
    );
    f.run.status = "cancelled".into();
    f.save();
    let result = cleanup::cleanup(&f.repo.root, &f.run.id).unwrap();
    assert_eq!(result["removed"], json!([a.worktree]));
    assert_eq!(result["branch_refs_retained"], true);
    assert!(git::command(&f.repo.root, &["rev-parse", &a.branch], None).is_ok());
}

#[test]
fn cleanup_invoked_inside_a_worker_retains_its_current_directory() {
    let mut f = Fixture::new();
    let a = f.attempt("current-worker", "accepted");
    f.save();
    let result = cleanup::cleanup(Path::new(&a.worktree), &f.run.id).unwrap();
    assert_eq!(result["removed"], json!([]));
    assert!(
        result["retained_details"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["reason"] == "current_worktree_retained")
    );
    assert!(Path::new(&a.worktree).exists());
}
