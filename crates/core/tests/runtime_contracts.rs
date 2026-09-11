//! Real local boundary checks. The test executable doubles as a deterministic
//! worker, so ordinary subprocess coverage does not require an external runtime.
//! One additional Node regression runs when the local Node executable is present.
use rusqlite::Connection;
use serde_json::{Value, json};
use spec_autonomous_core::{
    Framework,
    config::Config,
    git::{self, Repository},
    model::{Run, WorkerInput, WorkerResult},
    process, runner,
    state::{Lease, Store},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tempfile::{TempDir, tempdir};

const FIXTURE: &str = "SA_RUNTIME_FIXTURE";
const FIXTURE_DIR: &str = "SA_RUNTIME_FIXTURE_DIR";

fn test_argv() -> Vec<String> {
    vec![
        std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        "--exact".into(),
        "subprocess_fixture".into(),
        "--ignored".into(),
        "--nocapture".into(),
        "--test-threads=1".into(),
    ]
}

fn fixture_env(root: &Path, mode: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        (FIXTURE.into(), mode.into()),
        (FIXTURE_DIR.into(), root.to_string_lossy().into_owned()),
    ])
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "fixture did not create {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn fixture_spawn(root: &Path, mode: &str) -> Child {
    let argv = test_argv();
    Command::new(&argv[0])
        .args(&argv[1..])
        .envs(fixture_env(root, mode))
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// Launched explicitly in a fresh process by the tests below. Ignored by the
/// ordinary suite; arguments and environment are private to each child.
#[test]
#[ignore = "subprocess entry point"]
// Descendants intentionally outlive their immediate parent: the supervisor's
// process-group/Job Object cleanup, rather than this fixture, is under test.
#[allow(clippy::zombie_processes)]
fn subprocess_fixture() {
    let mode = std::env::var(FIXTURE).expect("fixture mode");
    let root = PathBuf::from(std::env::var_os(FIXTURE_DIR).expect("fixture directory"));
    match mode.as_str() {
        "inspect" => {
            let mut input = vec![];
            std::io::stdin().read_to_end(&mut input).unwrap();
            let observed = json!({
                "argv": std::env::args().skip(1).collect::<Vec<_>>(),
                "cwd": std::env::current_dir().unwrap(),
                "environment": std::env::var("SA_FIXTURE_LITERAL").unwrap(),
                "stdin": input,
            });
            fs::write(
                root.join("observed.json"),
                serde_json::to_vec(&observed).unwrap(),
            )
            .unwrap();
            println!("fixture stdout");
            eprintln!("fixture stderr");
            std::process::exit(7);
        }
        "sleep" => {
            fs::write(root.join("ready"), "ready").unwrap();
            thread::sleep(Duration::from_secs(30));
        }
        "log-stdout" | "log-stderr" => {
            let bytes = vec![b'x'; 256 * 1024];
            if mode == "log-stdout" {
                std::io::stdout().write_all(&bytes).unwrap();
            } else {
                std::io::stderr().write_all(&bytes).unwrap();
            }
            thread::sleep(Duration::from_secs(30));
        }
        "leaf" | "stubborn-leaf" => {
            #[cfg(unix)]
            if mode == "stubborn-leaf" {
                unsafe {
                    libc::signal(libc::SIGTERM, libc::SIG_IGN);
                }
            }
            fs::write(root.join("leaf.pid"), std::process::id().to_string()).unwrap();
            thread::sleep(Duration::from_secs(30));
        }
        "middle" | "middle-stubborn" => {
            let _leaf = fixture_spawn(
                &root,
                if mode == "middle-stubborn" {
                    "stubborn-leaf"
                } else {
                    "leaf"
                },
            );
            fs::write(root.join("middle.pid"), std::process::id().to_string()).unwrap();
            thread::sleep(Duration::from_secs(30));
        }
        "tree-exit" | "tree-wait" | "tree-stubborn" => {
            let _middle = fixture_spawn(
                &root,
                if mode == "tree-stubborn" {
                    "middle-stubborn"
                } else {
                    "middle"
                },
            );
            wait_for_file(&root.join("leaf.pid"));
            fs::write(root.join("ready"), "ready").unwrap();
            if mode != "tree-exit" {
                thread::sleep(Duration::from_secs(30));
            }
        }
        "stubborn" => {
            #[cfg(unix)]
            unsafe {
                libc::signal(libc::SIGTERM, libc::SIG_IGN);
            }
            fs::write(root.join("ready"), "ready").unwrap();
            thread::sleep(Duration::from_secs(30));
        }
        "runner" => {
            let input_path = std::env::var_os("SPEC_AUTONOMOUS_INPUT").unwrap();
            let output_path = std::env::var_os("SPEC_AUTONOMOUS_RESULT").unwrap();
            let input: WorkerInput =
                serde_json::from_slice(&fs::read(input_path).unwrap()).unwrap();
            let behavior = std::env::var("SA_RUNNER_BEHAVIOR").unwrap_or_default();
            if behavior == "missing" {
                return;
            }
            if behavior == "invalid-json" {
                fs::write(output_path, "not JSON").unwrap();
                return;
            }
            if behavior == "oversized" {
                fs::write(output_path, vec![b' '; 65 * 1024]).unwrap();
                return;
            }
            let mut result = valid_result(&input);
            if behavior == "wrong-attempt" {
                result.attempt_id = "some-other-attempt".into();
            }
            if behavior == "blocked" {
                result.status = "blocked".into();
                result.blockers.push("fixture needs decision".into());
            }
            fs::write(output_path, serde_json::to_vec(&result).unwrap()).unwrap();
            if behavior == "nonzero" {
                eprintln!("fixture rejected work");
                std::process::exit(17);
            }
        }
        "lock" => {
            let repo = Repository::discover(&root).unwrap();
            match Lease::acquire(&repo) {
                Ok(_lease) => std::process::exit(0),
                Err(error) => {
                    eprintln!("{error:#}");
                    std::process::exit(23);
                }
            }
        }
        "reconcile-record" => match process::reconcile_process(&root.join("process.json")) {
            Ok(()) => std::process::exit(0),
            Err(error) => {
                eprintln!("{error:#}");
                std::process::exit(24);
            }
        },
        other => panic!("unknown fixture mode {other}"),
    }
}

fn repository() -> (TempDir, Repository) {
    let dir = tempfile::Builder::new()
        .prefix("sa real git 空格 ")
        .tempdir()
        .unwrap();
    git::command(
        dir.path(),
        &["init", "--quiet", "--initial-branch=main"],
        None,
    )
    .unwrap();
    // Keep host Git settings from adding hooks, signatures, or CRLF conversions.
    git::command(
        dir.path(),
        &["config", "core.hooksPath", ".git/absent-hooks"],
        None,
    )
    .unwrap();
    git::command(dir.path(), &["config", "core.autocrlf", "false"], None).unwrap();
    fs::write(dir.path().join("seed.txt"), "base\n").unwrap();
    git::commit(dir.path(), "test baseline").unwrap();
    let repo = Repository::discover(dir.path()).unwrap();
    (dir, repo)
}

fn sample_run(repo: &Repository) -> Run {
    let head = repo.head().unwrap();
    serde_json::from_value(json!({
        "schema_version": 1, "id": "run-contract", "milestone": {
            "schema_version": 1, "id": "M001", "goal": "runtime contract", "framework": "openspec", "revision": 1,
            "phases": [{"id":"P001", "label":"1", "title":"runtime", "source":{"kind":"openspec-change","selector":"runtime"}}]
        },
        "mode": "autonomous", "range": {"from":"P001", "to":"P001", "only":null},
        "selected_phases": ["P001"], "origin": repo.root, "origin_head": head,
        "origin_branch": "main", "project_relative": ".", "integration": repo.root,
        "integration_branch": "main", "accepted_head": head, "status": "running", "stage":"executing_phase",
        "current_phase":"P001", "started_at":"2026-09-11T00:00:00Z", "updated_at":"2026-09-11T00:00:00Z",
        "elapsed_ms": 111, "blocker":null, "config":Config::default()
    })).unwrap()
}

fn worker_input(attempt: &str) -> WorkerInput {
    WorkerInput {
        schema_version: 1,
        run_id: "run-contract".into(),
        task_id: "task-contract".into(),
        attempt_id: attempt.into(),
        kind: "implement".into(),
        goal: "bounded mock task".into(),
        framework: Framework::Openspec,
        base_commit: "1234567".into(),
        instruction: "Perform only the fixture task".into(),
        task: None,
        snapshot: None,
        milestone: None,
        failure: None,
    }
}

fn valid_result(input: &WorkerInput) -> WorkerResult {
    WorkerResult {
        schema_version: 1,
        run_id: input.run_id.clone(),
        task_id: input.task_id.clone(),
        attempt_id: input.attempt_id.clone(),
        status: "candidate".into(),
        summary: "bounded deterministic result".into(),
        blockers: vec![],
        milestone: None,
        plan: None,
        audit: vec![],
    }
}

fn runner_config(root: &Path, behavior: &str) -> Config {
    let mut config = Config::default();
    config.runner.command = test_argv();
    config.runner.fresh_session = true;
    config.runner.environment = fixture_env(root, "runner");
    config
        .runner
        .environment
        .insert("SA_RUNNER_BEHAVIOR".into(), behavior.into());
    config.execution.attempt_timeout_seconds = 5;
    config
}

fn execute_fixture(
    root: &Path,
    mode: &str,
    timeout: Duration,
    quota: u64,
    cancel: Arc<AtomicBool>,
) -> anyhow::Result<process::Output> {
    process::execute(process::Request {
        argv: &test_argv(),
        cwd: root,
        env: &fixture_env(root, mode),
        stdin: None,
        directory: &root.join("logs"),
        timeout,
        max_log_bytes: quota,
        cancel,
    })
}

#[cfg(unix)]
fn is_running(pid: u32) -> bool {
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "stat="])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .is_some_and(|out| {
            let state = String::from_utf8_lossy(&out.stdout);
            let state = state.trim();
            !state.is_empty() && !state.starts_with('Z')
        })
}

#[cfg(windows)]
fn is_running(pid: u32) -> bool {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0;
        let queried = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        queried != 0 && code == 259
    }
}

fn assert_stopped(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while is_running(pid) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!is_running(pid), "worker descendant {pid} survived cleanup");
}

fn assert_descendants_stopped(root: &Path) {
    for name in ["middle.pid", "leaf.pid"] {
        let pid = fs::read_to_string(root.join(name))
            .unwrap()
            .parse()
            .unwrap();
        assert_stopped(pid);
    }
}

#[test]
fn managed_worktrees_isolate_writes_and_share_repository_identity() {
    let (_dir, repo) = repository();
    repo.preflight().unwrap();
    let base = repo.head().unwrap();
    let (left, left_branch) = repo.add_worktree("left", &base).unwrap();
    let (right, right_branch) = repo.add_worktree("right", &base).unwrap();
    assert_ne!(left, right);
    assert_ne!(left_branch, right_branch);
    fs::write(left.join("seed.txt"), "left\n").unwrap();
    fs::write(right.join("seed.txt"), "right\n").unwrap();
    assert_eq!(
        fs::read_to_string(repo.root.join("seed.txt")).unwrap(),
        "base\n"
    );
    assert!(git::clean(&repo.root).unwrap());
    assert_eq!(Repository::discover(&left).unwrap().common, repo.common);
    assert_eq!(
        Repository::discover(&right).unwrap().runtime().unwrap(),
        repo.runtime().unwrap()
    );
    let rows = Repository::discover(&left).unwrap().worktrees().unwrap();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().any(|row| {
        Path::new(&row.path)
            .canonicalize()
            .is_ok_and(|path| path == right.canonicalize().unwrap())
    }));
}

#[test]
fn git_patch_preserves_binary_deletions_and_spaced_paths_before_delivery() {
    let (_dir, repo) = repository();
    let base = repo.head().unwrap();
    let (worker, _) = repo.add_worktree("writer", &base).unwrap();
    let (integration, _) = repo.add_worktree("integration", &base).unwrap();
    let bytes = [0, 255, 13, 10, 0, 42];
    fs::write(worker.join("space 名字.bin"), bytes).unwrap();
    fs::remove_file(worker.join("seed.txt")).unwrap();
    let changed = git::changed(&worker, &base).unwrap();
    assert_eq!(
        changed.into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from(["seed.txt".into(), "space 名字.bin".into()])
    );
    let patch = git::patch(&worker, &base).unwrap();
    git::apply(&integration, &patch).unwrap();
    assert_eq!(fs::read(integration.join("space 名字.bin")).unwrap(), bytes);
    assert!(!integration.join("seed.txt").exists());
    assert!(repo.root.join("seed.txt").exists());
    let accepted = git::commit(&integration, "validated fixture patch").unwrap();
    assert!(git::ancestor(&repo.root, &base, &accepted));
    git::advance(&repo.root, &accepted).unwrap();
    assert_eq!(repo.head().unwrap(), accepted);
    assert!(git::clean(&repo.root).unwrap());
}

#[test]
fn git_preflight_and_delivery_preserve_dirty_or_unborn_checkout() {
    let unborn = tempdir().unwrap();
    git::command(
        unborn.path(),
        &["init", "--quiet", "--initial-branch=main"],
        None,
    )
    .unwrap();
    let unready = Repository::discover(unborn.path()).unwrap();
    assert!(format!("{:#}", unready.preflight().unwrap_err()).contains("initial commit"));
    let (_dir, repo) = repository();
    let base = repo.head().unwrap();
    fs::write(repo.root.join("seed.txt"), "user-owned edit\n").unwrap();
    assert!(
        repo.preflight()
            .unwrap_err()
            .to_string()
            .contains("dirty_checkout")
    );
    assert!(
        git::advance(&repo.root, &base)
            .unwrap_err()
            .to_string()
            .contains("dirty_checkout")
    );
    assert_eq!(
        fs::read_to_string(repo.root.join("seed.txt")).unwrap(),
        "user-owned edit\n"
    );
    assert_eq!(repo.head().unwrap(), base);
    assert!(
        git::command(&repo.root, &["stash", "list"], None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn invalid_managed_worktree_identifiers_cannot_escape_the_runtime_root() {
    let (_dir, repo) = repository();
    let base = repo.head().unwrap();
    for name in ["../escape", "-b evil", "nested/name", "", "a\\b"] {
        assert!(
            repo.add_worktree(name, &base)
                .unwrap_err()
                .to_string()
                .contains("invalid_id")
        );
    }
    assert_eq!(repo.worktrees().unwrap().len(), 1);
    assert!(!repo.runtime().unwrap().exists());
}

#[cfg(unix)]
#[test]
fn managed_worktree_directory_symlink_cannot_redirect_git_outside_runtime() {
    use std::os::unix::fs::symlink;
    let (_dir, repo) = repository();
    let external = tempdir().unwrap();
    let runtime = repo.runtime().unwrap();
    fs::create_dir_all(&runtime).unwrap();
    symlink(external.path(), runtime.join("worktrees")).unwrap();
    let error = repo
        .add_worktree("redirected", &repo.head().unwrap())
        .unwrap_err();
    assert!(error.to_string().contains("path_outside_scope"));
    assert_eq!(fs::read_dir(external.path()).unwrap().count(), 0);
    assert_eq!(repo.worktrees().unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn store_rejects_symlinked_database_and_attempt_ancestors() {
    use std::os::unix::fs::symlink;
    let (_dir, repo) = repository();
    let external = tempdir().unwrap();
    let store = Store::open(&repo, true).unwrap().unwrap();
    symlink(external.path(), store.root.join("runs")).unwrap();
    let error = store
        .attempt_dir("run-contract", "attempt-contract")
        .unwrap_err();
    assert!(error.to_string().contains("path_outside_scope"));
    assert_eq!(fs::read_dir(external.path()).unwrap().count(), 0);
    drop(store);
    let db_path = repo.runtime().unwrap().join("state.db");
    fs::remove_file(&db_path).unwrap();
    fs::write(external.path().join("private"), "untouched").unwrap();
    symlink(external.path().join("private"), &db_path).unwrap();
    for writable in [false, true] {
        let error = Store::open(&repo, writable)
            .err()
            .expect("database symlink must be rejected");
        assert!(error.to_string().contains("path_outside_scope"));
    }
    assert_eq!(
        fs::read_to_string(external.path().join("private")).unwrap(),
        "untouched"
    );
}

#[test]
fn read_only_store_on_new_repository_does_not_initialize_state() {
    let (_dir, repo) = repository();
    let runtime = repo.runtime().unwrap();
    assert!(Store::open(&repo, false).unwrap().is_none());
    assert!(!runtime.exists());
    assert!(git::clean(&repo.root).unwrap());
}

#[test]
fn store_roundtrip_preserves_policy_and_controls_across_reopen() {
    let (_dir, repo) = repository();
    let mut run = sample_run(&repo);
    run.config.execution.max_workers = 7;
    run.config.execution.max_attempts = 9;
    run.config.execution.delivery = "branch".into();
    {
        let mut store = Store::open(&repo, true).unwrap().unwrap();
        store.save(&run, "started").unwrap();
        store.control(&run.id, "pause").unwrap();
        assert!(store.control(&run.id, "unknown").is_err());
    }
    let store = Store::open(&repo, false).unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(store.get(&run.id).unwrap()).unwrap(),
        serde_json::to_value(&run).unwrap()
    );
    assert_eq!(store.requested(&run.id).unwrap(), "pause");
    assert_eq!(store.list().unwrap().len(), 1);
    assert_eq!(store.events(&run.id).unwrap()[0]["kind"], "started");
    assert!(store.get("../escape").is_err());
}

#[test]
fn event_insert_failure_rolls_back_the_run_update_in_same_transaction() {
    let (_dir, repo) = repository();
    let mut store = Store::open(&repo, true).unwrap().unwrap();
    let mut run = sample_run(&repo);
    store.save(&run, "before").unwrap();
    let before = serde_json::to_value(store.get(&run.id).unwrap()).unwrap();
    let db = Connection::open(store.root.join("state.db")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_event BEFORE INSERT ON events BEGIN SELECT RAISE(FAIL, 'fixture event failure'); END;").unwrap();
    run.status = "completed".into();
    run.updated_at = "2026-09-11T01:00:00Z".into();
    assert!(store.save(&run, "must-rollback").is_err());
    assert_eq!(
        serde_json::to_value(store.get(&run.id).unwrap()).unwrap(),
        before
    );
    assert_eq!(store.events(&run.id).unwrap().len(), 1);
}

#[test]
fn read_only_store_rejects_database_and_artifact_writes() {
    let (_dir, repo) = repository();
    let run = sample_run(&repo);
    {
        Store::open(&repo, true)
            .unwrap()
            .unwrap()
            .save(&run, "seed")
            .unwrap();
    }
    let mut store = Store::open(&repo, false).unwrap().unwrap();
    assert!(store.save(&run, "forbidden").is_err());
    assert!(store.control(&run.id, "cancel").is_err());
    assert!(
        store.attempt_dir(&run.id, "forbidden").is_err(),
        "read-only access must not create an attempt directory"
    );
    assert!(
        !store
            .root
            .join("runs/run-contract/attempts/forbidden")
            .exists()
    );
    assert_eq!(store.events(&run.id).unwrap().len(), 1);
}

#[test]
fn newer_database_schema_is_rejected_without_downgrade_or_write() {
    let (_dir, repo) = repository();
    drop(Store::open(&repo, true).unwrap());
    let path = repo.runtime().unwrap().join("state.db");
    let db = Connection::open(&path).unwrap();
    db.pragma_update(None, "user_version", 99).unwrap();
    for writable in [false, true] {
        let error = Store::open(&repo, writable)
            .err()
            .expect("newer schema must fail");
        assert!(error.to_string().contains("schema_unsupported"));
    }
    let version: u32 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 99);
}

#[test]
fn repository_lock_excludes_a_second_process_from_a_linked_worktree() {
    let (_dir, repo) = repository();
    let (linked, _) = repo
        .add_worktree("lock-peer", &repo.head().unwrap())
        .unwrap();
    let lease = Lease::acquire(&repo).unwrap();
    let argv = test_argv();
    let blocked = Command::new(&argv[0])
        .args(&argv[1..])
        .envs(fixture_env(&linked, "lock"))
        .current_dir(&linked)
        .output()
        .unwrap();
    assert_eq!(blocked.status.code(), Some(23));
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("run_already_active"));
    drop(lease);
    let available = Command::new(&argv[0])
        .args(&argv[1..])
        .envs(fixture_env(&linked, "lock"))
        .current_dir(&linked)
        .output()
        .unwrap();
    assert!(available.status.success());
}

#[test]
fn process_preserves_argv_stdin_cwd_environment_and_nonzero_status() {
    let root = tempfile::Builder::new()
        .prefix("sa process 空格 ")
        .tempdir()
        .unwrap();
    let mut argv = test_argv();
    argv.extend(
        [
            "a b",
            "$(not-a-command)",
            "`literal-command`",
            "quotes\"stay",
        ]
        .map(str::to_string),
    );
    let mut env = fixture_env(root.path(), "inspect");
    env.insert("SA_FIXTURE_LITERAL".into(), "space $() & literal".into());
    let input = b"a\0binary\r\nstdin";
    let output = process::execute(process::Request {
        argv: &argv,
        cwd: root.path(),
        env: &env,
        stdin: Some(input),
        directory: &root.path().join("logs"),
        timeout: Duration::from_secs(5),
        max_log_bytes: 4096,
        cancel: Arc::new(AtomicBool::new(false)),
    })
    .unwrap();
    assert_eq!(output.code, 7);
    assert!(!output.cancelled && !output.timed_out);
    let observed: Value =
        serde_json::from_slice(&fs::read(root.path().join("observed.json")).unwrap()).unwrap();
    assert_eq!(observed["argv"], json!(&argv[1..]));
    assert_eq!(
        Path::new(observed["cwd"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        root.path().canonicalize().unwrap()
    );
    assert_eq!(observed["environment"], "space $() & literal");
    assert_eq!(observed["stdin"], json!(input.as_slice()));
    assert!(
        fs::read_to_string(root.path().join("logs/stdout.log"))
            .unwrap()
            .contains("fixture stdout")
    );
    assert!(
        fs::read_to_string(root.path().join("logs/stderr.log"))
            .unwrap()
            .contains("fixture stderr")
    );
    let durable: process::Identity =
        serde_json::from_slice(&fs::read(root.path().join("logs/process.json")).unwrap()).unwrap();
    assert_eq!(durable.pid, output.identity.pid);
}

#[test]
fn missing_process_executable_is_not_reported_as_a_successful_attempt() {
    let root = tempdir().unwrap();
    let argv = vec![
        root.path()
            .join("absent executable")
            .to_string_lossy()
            .into_owned(),
    ];
    let error = process::execute(process::Request {
        argv: &argv,
        cwd: root.path(),
        env: &BTreeMap::new(),
        stdin: None,
        directory: &root.path().join("logs"),
        timeout: Duration::from_secs(1),
        max_log_bytes: 4096,
        cancel: Arc::new(AtomicBool::new(false)),
    })
    .unwrap_err();
    assert!(error.to_string().contains("runner_unavailable"));
    assert!(!root.path().join("logs/process.json").exists());
}

#[test]
fn nested_node_test_protocol_environment_cannot_turn_failed_verification_green() {
    let node = match Command::new("node").arg("--version").output() {
        Ok(output) if output.status.success() => "node".to_string(),
        Ok(output) => panic!(
            "installed Node is unusable: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("Node regression not run: node is unavailable on this machine");
            return;
        }
        Err(error) => panic!("cannot probe local Node: {error}"),
    };
    let root = tempfile::Builder::new()
        .prefix("sa nested node test ")
        .tempdir()
        .unwrap();
    let script = root.path().join("verification must fail.test.mjs");
    fs::write(&script, "throw new Error('VERIFICATION_MUST_FAIL');\n").unwrap();
    let argv = vec![
        node,
        "--test".into(),
        "--test-reporter=tap".into(),
        script.to_string_lossy().into_owned(),
    ];
    // These describe the *outer* Node harness or IPC channel, not the requested
    // test process. Leakage can skip the actual test or emit binary IPC frames.
    let env = BTreeMap::from([
        ("NODE_TEST_CONTEXT".into(), "child-v8".into()),
        ("NODE_TEST_WORKER_ID".into(), "99".into()),
        ("NODE_CHANNEL_FD".into(), "101".into()),
        ("NODE_CHANNEL_SERIALIZATION_MODE".into(), "advanced".into()),
        ("NODE_UNIQUE_ID".into(), "42".into()),
    ]);
    let logs = root.path().join("logs");
    let output = process::execute(process::Request {
        argv: &argv,
        cwd: root.path(),
        env: &env,
        stdin: None,
        directory: &logs,
        timeout: Duration::from_secs(10),
        max_log_bytes: 64 * 1024,
        cancel: Arc::new(AtomicBool::new(false)),
    })
    .unwrap();
    assert!(!output.cancelled && !output.timed_out);
    assert_ne!(
        output.code, 0,
        "a thrown verification error must not become a passing result"
    );
    let stdout = fs::read_to_string(logs.join("stdout.log")).unwrap();
    let stderr = fs::read_to_string(logs.join("stderr.log")).unwrap();
    let observed = format!("{stdout}\n{stderr}");
    assert!(
        stdout.contains("TAP version 13"),
        "expected ordinary test output, got: {observed}"
    );
    assert!(
        stdout.contains("not ok"),
        "the test runner must report the failed test: {observed}"
    );
    assert!(
        observed.contains("VERIFICATION_MUST_FAIL"),
        "the fixture must really execute, not merely fail IPC initialization: {observed}"
    );
    assert_stopped(output.identity.pid);
}

#[test]
fn process_timeout_and_cancel_have_distinct_outcomes_and_reap_the_worker() {
    for cancelled in [false, true] {
        let root = tempdir().unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let trigger = if cancelled {
            let trigger = flag.clone();
            let ready = root.path().join("ready");
            Some(thread::spawn(move || {
                wait_for_file(&ready);
                trigger.store(true, Ordering::SeqCst);
            }))
        } else {
            None
        };
        let output = execute_fixture(
            root.path(),
            "sleep",
            if cancelled {
                Duration::from_secs(5)
            } else {
                Duration::from_millis(250)
            },
            4096,
            flag,
        )
        .unwrap();
        if let Some(trigger) = trigger {
            trigger.join().unwrap();
        }
        assert_eq!(output.cancelled, cancelled);
        assert_eq!(output.timed_out, !cancelled);
        assert_eq!(output.code, if cancelled { 130 } else { 124 });
        assert_stopped(output.identity.pid);
    }
}

#[test]
fn process_log_quota_stops_writers_on_either_output_stream() {
    for mode in ["log-stdout", "log-stderr"] {
        let root = tempdir().unwrap();
        let error = execute_fixture(
            root.path(),
            mode,
            Duration::from_secs(5),
            1024,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
        assert!(error.to_string().contains("log_limit_exceeded"));
        let durable: process::Identity =
            serde_json::from_slice(&fs::read(root.path().join("logs/process.json")).unwrap())
                .unwrap();
        assert_stopped(durable.pid);
    }
}

#[test]
fn process_cleans_grandchildren_after_success_timeout_and_cancel() {
    for mode in ["success", "timeout", "cancel"] {
        let root = tempdir().unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let trigger = if mode == "cancel" {
            let flag = flag.clone();
            let ready = root.path().join("ready");
            Some(thread::spawn(move || {
                wait_for_file(&ready);
                flag.store(true, Ordering::SeqCst);
            }))
        } else {
            None
        };
        let output = execute_fixture(
            root.path(),
            if mode == "success" {
                "tree-exit"
            } else {
                "tree-wait"
            },
            if mode == "timeout" {
                Duration::from_secs(1)
            } else {
                Duration::from_secs(5)
            },
            4096,
            flag,
        )
        .unwrap();
        if let Some(trigger) = trigger {
            trigger.join().unwrap();
        }
        assert_eq!(
            output.code,
            match mode {
                "timeout" => 124,
                "cancel" => 130,
                _ => 0,
            }
        );
        assert_stopped(output.identity.pid);
        assert_descendants_stopped(root.path());
    }
}

#[cfg(unix)]
struct ProcessGuard(Child);
#[cfg(unix)]
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGKILL);
        }
        let _ = self.0.wait();
    }
}

#[cfg(unix)]
fn grouped_worker(root: &Path, mode: &str) -> ProcessGuard {
    use std::os::unix::process::CommandExt;
    let argv = test_argv();
    let child = Command::new(&argv[0])
        .args(&argv[1..])
        .envs(fixture_env(root, mode))
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let guard = ProcessGuard(child);
    wait_for_file(&root.join("ready"));
    guard
}

#[cfg(unix)]
#[test]
fn reconcile_refuses_reused_identity_without_signalling_live_process() {
    let root = tempdir().unwrap();
    let worker = grouped_worker(root.path(), "stubborn");
    let record = process::Identity {
        pid: worker.0.id(),
        identity: "different-start-identity".into(),
    };
    let path = root.path().join("process.json");
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(
        process::reconcile_process(&path)
            .unwrap_err()
            .to_string()
            .contains("process_identity_unknown")
    );
    assert!(is_running(worker.0.id()));
}

#[cfg(unix)]
#[test]
fn reconcile_does_not_release_a_stubborn_worker_to_keep_writing() {
    let root = tempdir().unwrap();
    let worker = grouped_worker(root.path(), "stubborn");
    let record = process::Identity {
        pid: worker.0.id(),
        identity: process::identity(worker.0.id()),
    };
    assert!(!record.identity.is_empty());
    let path = root.path().join("process.json");
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    let result = process::reconcile_process(&path);
    assert!(
        result.is_ok(),
        "known stale worker should be terminated: {result:?}"
    );
    assert_stopped(worker.0.id());
}

#[cfg(unix)]
#[test]
fn reconcile_cleans_stubborn_descendants_even_if_group_leader_exits_on_term() {
    let root = tempdir().unwrap();
    let worker = grouped_worker(root.path(), "tree-stubborn");
    let record = process::Identity {
        pid: worker.0.id(),
        identity: process::identity(worker.0.id()),
    };
    assert!(!record.identity.is_empty());
    let path = root.path().join("process.json");
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    process::reconcile_process(&path).unwrap();
    assert_stopped(worker.0.id());
    assert_descendants_stopped(root.path());
}

#[cfg(unix)]
#[test]
fn reconcile_never_treats_an_unavailable_identity_probe_as_a_dead_worker() {
    let root = tempdir().unwrap();
    let worker = grouped_worker(root.path(), "stubborn");
    let record = process::Identity {
        pid: worker.0.id(),
        identity: process::identity(worker.0.id()),
    };
    assert!(!record.identity.is_empty());
    fs::write(
        root.path().join("process.json"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    let argv = test_argv();
    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .envs(fixture_env(root.path(), "reconcile-record"))
        .env("PATH", root.path())
        .current_dir(root.path())
        .output()
        .unwrap();
    if output.status.success() {
        // A platform-native or absolute-path identity probe may still succeed.
        assert_stopped(worker.0.id());
    } else {
        assert_eq!(output.status.code(), Some(24));
        assert!(String::from_utf8_lossy(&output.stderr).contains("process_"));
    }
}

#[test]
fn runner_result_contract_rejects_wrong_identity_schema_and_completion_claims() {
    let input = worker_input("attempt-contract");
    runner::validate_result(&input, &valid_result(&input)).unwrap();
    for field in [
        "run_id",
        "task_id",
        "attempt_id",
        "schema_version",
        "status",
        "summary",
    ] {
        let mut value = serde_json::to_value(valid_result(&input)).unwrap();
        value[field] = match field {
            "schema_version" => json!(2),
            "status" => json!("integrated"),
            "summary" => json!("x".repeat(8193)),
            _ => json!("wrong-identity"),
        };
        let result: WorkerResult = serde_json::from_value(value).unwrap();
        assert!(
            runner::validate_result(&input, &result)
                .unwrap_err()
                .to_string()
                .contains("worker_protocol_error"),
            "field {field}"
        );
    }
    let mut blocked = valid_result(&input);
    blocked.blockers.push("unresolved product decision".into());
    assert!(
        runner::validate_result(&input, &blocked)
            .unwrap_err()
            .to_string()
            .contains("needs_input")
    );
}

#[test]
fn runner_rejects_bad_transport_results_even_when_worker_exits_successfully() {
    for (behavior, expected) in [
        ("missing", "worker_protocol_error"),
        ("invalid-json", "worker_protocol_error"),
        ("oversized", "worker_protocol_error"),
        ("wrong-attempt", "worker_protocol_error"),
        ("nonzero", "worker_failed"),
        ("blocked", "needs_input"),
    ] {
        let root = tempdir().unwrap();
        let error = runner::run(
            &worker_input("attempt-contract"),
            root.path(),
            &root.path().join("attempt"),
            &runner_config(root.path(), behavior),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
        assert!(
            format!("{error:#}").contains(expected),
            "{behavior}: {error:#}"
        );
    }
}

#[test]
fn runner_bounds_context_before_launch_and_refuses_reusing_result_files() {
    let root = tempdir().unwrap();
    let mut input = worker_input("attempt-contract");
    let mut config = runner_config(root.path(), "");
    config.execution.max_context_bytes = 1024;
    input.instruction = "x".repeat(2048);
    let dir = root.path().join("attempt");
    assert!(
        runner::run(
            &input,
            root.path(),
            &dir,
            &config,
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .to_string()
        .contains("context_too_large")
    );
    assert!(!dir.exists());
    input.instruction = "bounded".into();
    config.execution.max_context_bytes = 4096;
    runner::run(
        &input,
        root.path(),
        &dir,
        &config,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let before = fs::read(dir.join("result.json")).unwrap();
    let original_input = fs::read(dir.join("input.json")).unwrap();
    let original_prompt = fs::read(dir.join("prompt.md")).unwrap();
    input.goal = "different goal on a reused attempt".into();
    assert!(
        runner::run(
            &input,
            root.path(),
            &dir,
            &config,
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .to_string()
        .contains("attempt_reused")
    );
    assert_eq!(fs::read(dir.join("result.json")).unwrap(), before);
    assert_eq!(fs::read(dir.join("input.json")).unwrap(), original_input);
    assert_eq!(fs::read(dir.join("prompt.md")).unwrap(), original_prompt);
}

#[test]
fn runner_doctor_rejects_nonfresh_command_profiles() {
    let root = tempdir().unwrap();
    let mut config = runner_config(root.path(), "");
    assert!(runner::doctor(&config).is_ok());
    config.runner.fresh_session = false;
    assert!(
        runner::doctor(&config)
            .unwrap_err()
            .to_string()
            .contains("runner_not_configured")
    );
    config.runner.fresh_session = true;
    for flag in ["resume", "--resume", "--last"] {
        config.runner.command = vec![test_argv()[0].clone(), flag.into()];
        assert!(
            runner::doctor(&config)
                .unwrap_err()
                .to_string()
                .contains("runner_not_fresh")
        );
    }
}

#[test]
fn hundred_mock_attempts_have_fresh_packets_and_bounded_results() {
    let root = tempdir().unwrap();
    let config = runner_config(root.path(), "");
    let mut attempts = BTreeSet::new();
    for index in 0..100 {
        let input = worker_input(&format!("attempt-{index:03}"));
        let dir = root.path().join(&input.attempt_id);
        let result = runner::run(
            &input,
            root.path(),
            &dir,
            &config,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        let persisted: WorkerInput =
            serde_json::from_slice(&fs::read(dir.join("input.json")).unwrap()).unwrap();
        assert_eq!(persisted.attempt_id, input.attempt_id);
        assert_eq!(result.attempt_id, input.attempt_id);
        assert!(result.summary.len() <= 8192);
        assert!(fs::metadata(dir.join("result.json")).unwrap().len() <= 65536);
        let identity: process::Identity =
            serde_json::from_slice(&fs::read(dir.join("process.json")).unwrap()).unwrap();
        assert!(attempts.insert(input.attempt_id));
        // OS PIDs may be reused after exit; attempt identities must not be.
        assert_ne!(identity.pid, std::process::id());
        assert_stopped(identity.pid);
    }
    assert_eq!(attempts.len(), 100);
}

#[test]
fn legacy_ledger_is_backed_up_before_transactional_upgrade() {
    let (_dir, repo) = repository();
    let runtime = repo.runtime().unwrap();
    fs::create_dir_all(&runtime).unwrap();
    let db = Connection::open(runtime.join("state.db")).unwrap();
    db.execute_batch(
        "CREATE TABLE legacy(value TEXT); INSERT INTO legacy VALUES('preserve this');",
    )
    .unwrap();
    drop(db);
    drop(Store::open(&repo, true).unwrap());
    let backups: Vec<_> = fs::read_dir(&runtime)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".backup.db"))
        .collect();
    assert_eq!(backups.len(), 1);
    let backup = Connection::open(&backups[0]).unwrap();
    let value: String = backup
        .query_row("SELECT value FROM legacy", [], |r| r.get(0))
        .unwrap();
    assert_eq!(value, "preserve this");
    let version: u32 = backup
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 0);
    let upgraded = Connection::open(runtime.join("state.db")).unwrap();
    let version: u32 = upgraded
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 1);
    assert_eq!(
        upgraded
            .query_row("SELECT value FROM legacy", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "preserve this"
    );
}
