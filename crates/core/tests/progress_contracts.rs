//! Read-only, repository-wide progress against real temporary Git worktrees.
use serde_json::{Value, json};
use spec_autonomous_core::{
    config::Config,
    git::{self, Repository},
    model::{Attempt, Run},
    process, progress,
    state::Store,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    base: PathBuf,
    repo: Repository,
    initial: String,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let origin = base.join("origin with spaces");
        fs::create_dir(&origin).unwrap();
        git::command(&origin, &["init", "--quiet", "--initial-branch=main"], None).unwrap();
        fs::write(origin.join("README.md"), "# Progress contract fixture\n").unwrap();
        let initial = git::commit(&origin, "Create progress fixture").unwrap();
        let repo = Repository::discover(&origin).unwrap();
        Self {
            _temp: temp,
            base,
            repo,
            initial,
        }
    }

    fn external(&self, name: &str) -> PathBuf {
        let path = self.base.join(name);
        git::command(
            &self.repo.root,
            &[
                "worktree",
                "add",
                "--quiet",
                "--detach",
                path.to_str().unwrap(),
                "HEAD",
            ],
            None,
        )
        .unwrap();
        path.canonicalize().unwrap()
    }

    fn managed(&self, name: &str) -> (PathBuf, String) {
        self.repo.add_worktree(name, &self.initial).unwrap()
    }

    fn run(&self, id: &str, integration: &Path, branch: &str) -> Run {
        serde_json::from_value(json!({
            "schema_version": 1, "id": id,
            "milestone": {
                "schema_version": 1, "id": "greeting", "goal": "Deliver a verified greeting command",
                "framework": "openspec", "revision": 1,
                "phases": [{"id": "phase-a", "label": "1", "title": "Greeting", "depends_on": [],
                    "source": {"kind": "openspec-change", "selector": "greeting"}, "verification": []}],
                "verification": []
            },
            "mode": "autonomous", "range": {}, "selected_phases": ["phase-a"],
            "origin": self.repo.root, "origin_head": self.initial, "origin_branch": "main",
            "project_relative": ".", "integration": integration, "integration_branch": branch,
            "accepted_head": self.initial, "status": "running", "stage": "implementation", "current_phase": "phase-a",
            "started_at": "2026-09-11T12:00:00Z", "updated_at": "2026-09-11T12:00:00Z", "elapsed_ms": 0,
            "blocker": null, "config": Config::default()
        })).unwrap()
    }

    fn attempt(
        &self,
        worktree: &Path,
        branch: &str,
        id: &str,
        task: &str,
        status: &str,
    ) -> Attempt {
        Attempt {
            id: id.into(),
            task_id: task.into(),
            phase_id: "phase-a".into(),
            kind: "implement".into(),
            worktree: worktree.to_string_lossy().into_owned(),
            branch: branch.into(),
            base_commit: self.initial.clone(),
            status: status.into(),
            started_at: "2026-09-11T12:00:00Z".into(),
            finished_at: None,
            summary: String::new(),
            error: None,
            pid: None,
            process_identity: None,
            evidence: vec![],
        }
    }

    fn seed(&self, runs: &[Run]) {
        let mut db = Store::open(&self.repo, true).unwrap().unwrap();
        for run in runs {
            db.save(run, "contract-seed").unwrap();
        }
    }
}

fn row<'a>(snapshot: &'a Value, path: &Path) -> &'a Value {
    snapshot["worktrees"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"].as_str() == path.to_str())
        .unwrap_or_else(|| panic!("worktree {} absent from {snapshot}", path.display()))
}

fn inventory(snapshot: &Value) -> BTreeSet<String> {
    snapshot["worktrees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap().to_string())
        .collect()
}

fn file_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                visit(root, &entry.path(), output);
            } else if entry.file_type().unwrap().is_file() {
                output.insert(
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

#[test]
fn any_worktree_sees_the_same_complete_repository_inventory_without_a_database() {
    let fixture = Fixture::new();
    let first = fixture.external("external worker one");
    let second = fixture.external("external worker two");
    let expected = BTreeSet::from([
        fixture.repo.root.to_string_lossy().into_owned(),
        first.to_string_lossy().into_owned(),
        second.to_string_lossy().into_owned(),
    ]);
    let before = file_bytes(&fixture.base);
    let mut repository_id = None;
    for location in [&fixture.repo.root, &first, &second] {
        let snapshot = progress::snapshot(location).unwrap();
        assert_eq!(inventory(&snapshot), expected);
        assert_eq!(snapshot["consistency"], "consistent");
        assert_eq!(snapshot["active_workers"], 0);
        assert_eq!(snapshot["verified_tasks"], 0);
        assert_eq!(snapshot["usage"], "unavailable");
        assert!(snapshot["runs"].as_array().unwrap().is_empty());
        for worktree in snapshot["worktrees"].as_array().unwrap() {
            assert_eq!(worktree["kind"], "external");
            assert_eq!(worktree["status"], "unknown");
        }
        if let Some(id) = &repository_id {
            assert_eq!(&snapshot["repository_id"], id);
        }
        repository_id = Some(snapshot["repository_id"].clone());
    }
    assert_eq!(
        file_bytes(&fixture.base),
        before,
        "read-only inventory must not create state, registry, or alter Git files"
    );
    assert!(!fixture.repo.runtime().unwrap().exists());
}

#[test]
fn managed_metadata_joins_git_inventory_and_retries_do_not_inflate_task_totals() {
    let fixture = Fixture::new();
    let (integration, integration_branch) = fixture.managed("integration");
    let (worker, worker_branch) = fixture.managed("attempt-one");
    let external = fixture.external("unmanaged worktree");
    let mut run = fixture.run("run-one", &integration, &integration_branch);
    run.completed_tasks = vec!["phase-a/task-one".into()];
    run.attempts = vec![fixture.attempt(
        &worker,
        &worker_branch,
        "attempt-one",
        "task-one",
        "integrated",
    )];
    let mut resumed = run.clone();
    resumed.id = "run-two".into();
    resumed.integration = fixture.repo.root.to_string_lossy().into_owned();
    resumed.integration_branch = "main".into();
    resumed.attempts.clear();
    resumed.completed_tasks.push("phase-a/task-two".into());
    fixture.seed(&[run, resumed]);
    let before = fs::read(fixture.repo.runtime().unwrap().join("state.db")).unwrap();
    for from in [&fixture.repo.root, &integration, &worker, &external] {
        let snapshot = progress::snapshot(from).unwrap();
        assert_eq!(
            snapshot["verified_tasks"], 2,
            "same logical milestone task must not be counted once per run"
        );
        assert_eq!(snapshot["runs"].as_array().unwrap().len(), 2);
        assert_eq!(row(&snapshot, &integration)["kind"], "managed-integration");
        assert_eq!(row(&snapshot, &worker)["kind"], "managed-worker");
        assert_eq!(row(&snapshot, &worker)["task_id"], "task-one");
        assert_eq!(row(&snapshot, &worker)["attempt_id"], "attempt-one");
        assert_eq!(row(&snapshot, &external)["status"], "unknown");
    }
    assert_eq!(
        fs::read(fixture.repo.runtime().unwrap().join("state.db")).unwrap(),
        before
    );
}

#[test]
fn candidate_head_and_unverified_attempts_never_become_accepted_progress() {
    let fixture = Fixture::new();
    let (integration, integration_branch) = fixture.managed("integration");
    let (worker, worker_branch) = fixture.managed("candidate");
    fs::write(
        integration.join("candidate.rs"),
        "unverified candidate implementation\n",
    )
    .unwrap();
    let candidate_head =
        git::commit(&integration, "Candidate awaiting combined verification").unwrap();
    assert_ne!(candidate_head, fixture.initial);
    let mut run = fixture.run("run-one", &integration, &integration_branch);
    run.attempts.push(fixture.attempt(
        &worker,
        &worker_branch,
        "candidate-attempt",
        "task-one",
        "candidate",
    ));
    run.intents.push(serde_json::from_value(json!({
        "id": "intent-one", "attempt_id": "candidate-attempt", "task_id": "task-one", "worktree": worker,
        "expected_head": fixture.initial, "candidate_head": candidate_head, "final_head": null, "state": "candidate"
    })).unwrap());
    fixture.seed(&[run]);
    let snapshot = progress::snapshot(&worker).unwrap();
    assert_eq!(row(&snapshot, &integration)["head"], candidate_head);
    assert_eq!(
        row(&snapshot, &integration)["accepted_head"],
        fixture.initial
    );
    assert_eq!(row(&snapshot, &worker)["accepted_head"], fixture.initial);
    assert_eq!(row(&snapshot, &worker)["status"], "candidate");
    assert_eq!(snapshot["verified_tasks"], 0);
    assert_eq!(snapshot["runs"][0]["verified_tasks"], 0);
    assert_eq!(snapshot["runs"][0]["accepted_head"], fixture.initial);
}

#[test]
fn stale_attempt_without_process_identity_is_not_counted_as_active() {
    let fixture = Fixture::new();
    let (integration, integration_branch) = fixture.managed("integration");
    let (worker, worker_branch) = fixture.managed("worker");
    let mut run = fixture.run("run-one", &integration, &integration_branch);
    run.attempts.push(fixture.attempt(
        &worker,
        &worker_branch,
        "attempt-one",
        "task-one",
        "running",
    ));
    fixture.seed(&[run]);
    let runtime = fixture.repo.runtime().unwrap();
    let snapshot = progress::snapshot(&worker).unwrap();
    assert_eq!(snapshot["active_workers"], 0);
    assert_eq!(row(&snapshot, &worker)["status"], "stale");
    assert_eq!(row(&snapshot, &worker)["lease_state"], "stale");
    assert!(
        !runtime.join("runs/run-one/attempts/attempt-one").exists(),
        "progress must not create a missing attempt directory"
    );
}

#[test]
fn verified_process_identity_is_required_before_reporting_a_live_worker() {
    let fixture = Fixture::new();
    let (integration, integration_branch) = fixture.managed("integration");
    let (worker, worker_branch) = fixture.managed("worker");
    let mut run = fixture.run("run-one", &integration, &integration_branch);
    run.attempts.push(fixture.attempt(
        &worker,
        &worker_branch,
        "attempt-one",
        "task-one",
        "running",
    ));
    fixture.seed(&[run]);
    let identity_file = fixture
        .repo
        .runtime()
        .unwrap()
        .join("runs/run-one/attempts/attempt-one/process.json");
    fs::create_dir_all(identity_file.parent().unwrap()).unwrap();
    let pid = std::process::id();
    let identity = process::identity(pid);
    assert!(
        !identity.is_empty(),
        "current platform must support runner process identity"
    );
    fs::write(
        &identity_file,
        json!({"pid":pid,"identity":identity}).to_string(),
    )
    .unwrap();
    let live = progress::snapshot(&worker).unwrap();
    assert_eq!(live["active_workers"], 1);
    assert_eq!(row(&live, &worker)["lease_state"], "live");
    fs::write(
        &identity_file,
        json!({"pid":pid,"identity":"old-process-with-reused-pid"}).to_string(),
    )
    .unwrap();
    let stale = progress::snapshot(&worker).unwrap();
    assert_eq!(stale["active_workers"], 0);
    assert_eq!(row(&stale, &worker)["lease_state"], "stale");
}

#[test]
fn invalid_registry_cannot_redirect_progress_to_another_repositories_database() {
    let fixture = Fixture::new();
    let foreign = Fixture::new();
    let (foreign_integration, branch) = foreign.managed("foreign-integration");
    let mut secret_run = foreign.run("foreign-run", &foreign_integration, &branch);
    secret_run.milestone.id = "foreign-private-milestone".into();
    secret_run.completed_tasks = vec!["phase-a/foreign-task".into()];
    foreign.seed(&[secret_run]);
    let foreign_db = foreign.repo.runtime().unwrap().join("state.db");
    let foreign_before = fs::read(&foreign_db).unwrap();
    let runtime = fixture.repo.runtime().unwrap();
    fs::create_dir_all(&runtime).unwrap();
    let registry = format!(
        "schema_version = 1\nledger = {}\n",
        serde_json::to_string(foreign_db.to_str().unwrap()).unwrap()
    );
    fs::write(runtime.join("registry.toml"), &registry).unwrap();
    let snapshot = progress::snapshot(&fixture.repo.root).unwrap();
    assert_eq!(snapshot["consistency"], "partial");
    assert_eq!(snapshot["verified_tasks"], 0);
    assert!(snapshot["runs"].as_array().unwrap().is_empty());
    assert!(
        snapshot["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d.as_str().unwrap().contains("registry_invalid"))
    );
    assert!(!snapshot.to_string().contains("foreign-private-milestone"));
    assert!(!runtime.join("state.db").exists());
    assert_eq!(
        fs::read_to_string(runtime.join("registry.toml")).unwrap(),
        registry
    );
    assert_eq!(fs::read(foreign_db).unwrap(), foreign_before);
}

#[cfg(unix)]
#[test]
fn symlinked_ledger_is_not_followed_and_inventory_remains_available() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let external = fixture.external("external");
    let outside = fixture.base.join("outside.db");
    fs::write(&outside, "external ledger must not be read or changed").unwrap();
    let runtime = fixture.repo.runtime().unwrap();
    fs::create_dir_all(&runtime).unwrap();
    symlink(&outside, runtime.join("state.db")).unwrap();
    let snapshot = progress::snapshot(&external).unwrap();
    assert_eq!(snapshot["consistency"], "partial");
    assert_eq!(inventory(&snapshot).len(), 2);
    assert!(
        snapshot["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d.as_str().unwrap().contains("path_outside_scope"))
    );
    assert_eq!(
        fs::read_to_string(outside).unwrap(),
        "external ledger must not be read or changed"
    );
}

#[test]
fn corrupted_database_reports_partial_instead_of_hiding_git_worktrees() {
    let fixture = Fixture::new();
    let external = fixture.external("external");
    let runtime = fixture.repo.runtime().unwrap();
    fs::create_dir_all(&runtime).unwrap();
    let bytes = b"this is not a SQLite database";
    fs::write(runtime.join("state.db"), bytes).unwrap();
    let snapshot = progress::snapshot(&external).unwrap();
    assert_eq!(snapshot["consistency"], "partial");
    assert!(!snapshot["diagnostics"].as_array().unwrap().is_empty());
    assert_eq!(inventory(&snapshot).len(), 2);
    assert_eq!(fs::read(runtime.join("state.db")).unwrap(), bytes);
}

#[test]
fn corrupt_attempt_identity_cannot_read_an_external_process_file() {
    let fixture = Fixture::new();
    let (integration, integration_branch) = fixture.managed("integration");
    let (worker, worker_branch) = fixture.managed("worker");
    let outside = fixture.base.join("private process data");
    fs::create_dir_all(&outside).unwrap();
    let pid = std::process::id();
    let sentinel = json!({"pid":pid,"identity":process::identity(pid)}).to_string();
    fs::write(outside.join("process.json"), &sentinel).unwrap();
    let mut run = fixture.run("run-one", &integration, &integration_branch);
    run.attempts.push(fixture.attempt(
        &worker,
        &worker_branch,
        outside.to_str().unwrap(),
        "task-one",
        "running",
    ));
    fixture.seed(&[run]);
    let snapshot = progress::snapshot(&worker).unwrap();
    assert_eq!(
        snapshot["active_workers"], 0,
        "an invalid attempt ID must not redirect process-identity reads"
    );
    assert_eq!(snapshot["consistency"], "partial");
    assert!(!snapshot["diagnostics"].as_array().unwrap().is_empty());
    assert_eq!(
        fs::read_to_string(outside.join("process.json")).unwrap(),
        sentinel
    );
}

#[test]
fn missing_and_locked_external_worktrees_retain_git_inventory_status() {
    let fixture = Fixture::new();
    let locked = fixture.external("locked external");
    let removed = fixture.external("removed external");
    git::command(
        &fixture.repo.root,
        &[
            "worktree",
            "lock",
            "--reason",
            "owned by another workflow",
            locked.to_str().unwrap(),
        ],
        None,
    )
    .unwrap();
    fs::remove_dir_all(&removed).unwrap();
    let snapshot = progress::snapshot(&fixture.repo.root).unwrap();
    assert_eq!(inventory(&snapshot).len(), 3);
    assert_eq!(row(&snapshot, &locked)["locked"], true);
    assert_eq!(row(&snapshot, &locked)["kind"], "external");
    assert_eq!(row(&snapshot, &locked)["status"], "unknown");
    assert_eq!(row(&snapshot, &removed)["prunable"], true);
    assert_eq!(row(&snapshot, &removed)["status"], "prunable");
}

#[test]
fn json_and_toml_progress_views_have_identical_portable_semantics() {
    let fixture = Fixture::new();
    let (integration, branch) = fixture.managed("integration");
    let mut run = fixture.run("run-one", &integration, &branch);
    run.completed_tasks = vec!["phase-a/task-one".into()];
    fixture.seed(&[run]);
    let mut snapshot = progress::snapshot(&integration).unwrap();
    progress::portable(&mut snapshot);
    assert_eq!(snapshot["schema_version"], 1);
    assert!(
        snapshot["snapshot_id"]
            .as_str()
            .unwrap()
            .starts_with("snapshot-")
    );
    assert!(snapshot["generated_at"].as_str().unwrap().ends_with('Z'));
    let restored: Value = toml::from_str(&toml::to_string_pretty(&snapshot).unwrap()).unwrap();
    assert_eq!(restored, snapshot);
    assert_eq!(restored["verified_tasks"], 1);
    assert!(
        !restored["runs"][0]
            .as_object()
            .unwrap()
            .contains_key("blocker")
    );
}

#[test]
fn public_run_redacts_credentials_and_still_roundtrips_through_toml() {
    let fixture = Fixture::new();
    let (integration, branch) = fixture.managed("integration");
    let mut run = fixture.run("run-one", &integration, &branch);
    run.config
        .runner
        .environment
        .insert("TOKEN".into(), "sentinel-private-token".into());
    run.config.runner.command = vec!["agent".into(), "--token=sentinel-private-token".into()];
    let mut public = progress::public_run(&run);
    assert!(!public.to_string().contains("sentinel-private-token"));
    assert_eq!(public["config"]["runner"]["environment"], "redacted");
    assert_eq!(public["accepted_head"], fixture.initial);
    progress::portable(&mut public);
    let restored: Value = toml::from_str(&toml::to_string_pretty(&public).unwrap()).unwrap();
    assert_eq!(public, restored);
}

#[test]
fn human_progress_reports_partial_observations_with_their_diagnostics() {
    let rendered = progress::human(&json!({
        "schema_version": 1, "consistency": "partial", "active_workers": 0, "verified_tasks": 0,
        "worktrees": [{"path":"/repo","kind":"external","status":"unknown"}],
        "diagnostics": ["state_unavailable: ledger is temporarily unreadable"]
    }));
    assert!(
        rendered.contains("partial"),
        "human view must not present an incomplete observation as authoritative zero progress: {rendered}"
    );
    assert!(rendered.contains("state_unavailable"));
}

#[test]
fn terminal_scope_completion_has_no_implicit_resume_action() {
    let fixture = Fixture::new();
    let (integration, branch) = fixture.managed("integration");
    let mut run = fixture.run("run-one", &integration, &branch);
    run.status = "scope_completed".into();
    run.range.only = Some("phase-a".into());
    assert_eq!(progress::summary(&run)["next_action"], "");
    assert_eq!(progress::summary(&run)["status"], "scope_completed");
    run.status = "delivery_pending".into();
    assert_eq!(
        progress::summary(&run)["next_action"],
        "spec-autonomous resume run-one"
    );
}

#[cfg(unix)]
#[test]
fn inventory_changes_between_reads_are_reported_partial() {
    // Execute the child branch in its own test process so the temporary Git
    // wrapper cannot affect other concurrently running tests or the parent PATH.
    if let Ok(root) = std::env::var("SA_PROGRESS_CONTRACT_CHILD_ROOT") {
        let snapshot = progress::snapshot(Path::new(&root)).unwrap();
        assert_eq!(snapshot["consistency"], "partial");
        assert!(
            snapshot["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d == "inventory_changed_during_snapshot")
        );
        return;
    }
    use std::{os::unix::fs::PermissionsExt, process::Command};
    let fixture = Fixture::new();
    let real_git_output = Command::new("/bin/sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    assert!(real_git_output.status.success());
    let real_git = String::from_utf8(real_git_output.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert!(Path::new(&real_git).is_absolute());
    let wrapper_dir = fixture.base.join("git wrapper");
    fs::create_dir(&wrapper_dir).unwrap();
    let wrapper = wrapper_dir.join("git");
    fs::write(&wrapper, r#"#!/bin/sh
case " $* " in
  *" worktree list "*)
    if [ ! -f "$SA_PROGRESS_CONTRACT_MARKER" ]; then
      "$SA_PROGRESS_CONTRACT_REAL_GIT" "$@" || exit $?
      : > "$SA_PROGRESS_CONTRACT_MARKER"
      "$SA_PROGRESS_CONTRACT_REAL_GIT" -C "$SA_PROGRESS_CONTRACT_CHILD_ROOT" worktree add --quiet --detach "$SA_PROGRESS_CONTRACT_NEW_WORKTREE" HEAD >&2 || exit $?
      exit 0
    fi
    ;;
esac
exec "$SA_PROGRESS_CONTRACT_REAL_GIT" "$@"
"#).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let mut path = vec![wrapper_dir];
    path.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "inventory_changes_between_reads_are_reported_partial",
            "--nocapture",
        ])
        .env("PATH", std::env::join_paths(path).unwrap())
        .env("SA_PROGRESS_CONTRACT_CHILD_ROOT", &fixture.repo.root)
        .env("SA_PROGRESS_CONTRACT_REAL_GIT", real_git)
        .env(
            "SA_PROGRESS_CONTRACT_MARKER",
            fixture.base.join("inventory-read-marker"),
        )
        .env(
            "SA_PROGRESS_CONTRACT_NEW_WORKTREE",
            fixture.base.join("created during snapshot"),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child stdout:\n{}\nchild stderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fixture.repo.worktrees().unwrap().len(),
        2,
        "the interleaving must actually create a Git worktree"
    );
}
