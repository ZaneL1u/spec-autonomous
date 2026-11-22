//! Release hardening for 0.2.0: write-scope grants, interrupted verification, submodule
//! cleanup, doctor hints, ledger versioning, failure summaries and active-time budgets.
//! Real Git and SQLite, no agents.
#![allow(dead_code)]
use serde_json::{Value, json};
use spec_autonomous_core::{
    capabilities as api,
    config::Config,
    engine,
    git::{self, Repository},
    model::*,
    state::Store,
};
use std::{fs, path::Path};

fn put(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}
fn value_check() -> Check {
    Check {
        argv: vec!["node".into(), "--test".into(), "tests/value.mjs".into()],
        cwd: ".".into(),
        env: Default::default(),
    }
}
/// A Spec Kit feature with two parallel native tasks; the plan maps them to `value`
/// (src/value.txt) and `other` (src/other.txt).
fn fixture(configure: impl FnOnce(&mut Config)) -> tempfile::TempDir {
    fixture_with(configure, |_| {})
}
fn fixture_with(
    configure: impl FnOnce(&mut Config),
    before_commit: impl FnOnce(&Path),
) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    put(
        root.path(),
        ".specify/memory/constitution.md",
        "# Constitution\nUse executable assertions.\n",
    );
    put(
        root.path(),
        "specs/feature/spec.md",
        "# Specification\n- FR-001: src/value.txt contains 42.\n",
    );
    put(
        root.path(),
        "specs/feature/plan.md",
        "# Plan\nUse a plain UTF-8 file and Node assertions.\n",
    );
    put(
        root.path(),
        "specs/feature/tasks.md",
        "## Implementation\n- [ ] T001 [P] Write 42 in src/value.txt\n- [ ] T002 [P] Write src/other.txt\n",
    );
    put(
        root.path(),
        "tests/value.mjs",
        "import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';assert.equal(readFileSync('src/value.txt','utf8'),'42');\n",
    );
    put(
        root.path(),
        ".gitignore",
        "dist/\n.spec-autonomous/*\n!.spec-autonomous/config.toml\n!.spec-autonomous/milestones/\n!.spec-autonomous/plans/\n",
    );
    let mut config = Config::default();
    config.host.max_concurrency = 2;
    config.verification = vec![Check {
        argv: vec!["node".into(), "--version".into()],
        cwd: ".".into(),
        env: Default::default(),
    }];
    configure(&mut config);
    put(
        root.path(),
        ".spec-autonomous/config.toml",
        &toml::to_string(&config).unwrap(),
    );
    git::command(root.path(), &["init", "-q", "-b", "main"], None).unwrap();
    before_commit(root.path());
    git::commit(root.path(), "fixture").unwrap();
    root
}
fn host() -> Value {
    json!({"host_id":"codex-local","session_id":"main","fresh_context":true})
}
fn packet(root: &Path, request: &Value) -> WorkerInput {
    serde_json::from_value(
        engine::work_context(
            root,
            request["run_id"].as_str().unwrap(),
            request["request_id"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn receipt(input: &WorkerInput) -> WorkerResult {
    WorkerResult {
        schema_version: 1,
        run_id: input.run_id.clone(),
        task_id: input.task_id.clone(),
        attempt_id: input.attempt_id.clone(),
        status: "candidate".into(),
        summary: "host result".into(),
        blockers: vec![],
        milestone: None,
        plan: None,
        audit: vec![],
        requested_writes: vec![],
        deferred_evidence: vec![],
    }
}
fn submit(root: &Path, request: &Value, result: WorkerResult) -> Value {
    api::invoke(
        root,
        "apply-result",
        &json!({"token":request["token"],"host":request["owner"],"result":result,"claim_next":true,"view":"full"}),
    )
    .unwrap()
}
fn task(id: &str, source: &str, write: &str) -> Task {
    Task {
        id: id.into(),
        description: format!("write {write}"),
        source_ids: vec![source.into()],
        depends_on: vec![],
        reads: vec![],
        writes: vec![write.into()],
        verify_only: false,
        verification: vec![if id == "value" {
            value_check()
        } else {
            Check {
                argv: vec!["node".into(), "--version".into()],
                cwd: ".".into(),
                env: Default::default(),
            }
        }],
    }
}
fn answer_plan(root: &Path, request: &Value) -> Value {
    let input = packet(root, request);
    let snapshot = input.snapshot.clone().unwrap();
    let mut result = receipt(&input);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        tasks: vec![
            task("value", &snapshot.tasks[0].id, "src/value.txt"),
            task("other", &snapshot.tasks[1].id, "src/other.txt"),
        ],
        milestone: input.milestone.clone(),
    });
    submit(root, request, result)
}
fn saved(root: &Path, id: &str) -> Run {
    let repo = Repository::discover(root).unwrap();
    Store::open(&repo, false).unwrap().unwrap().get(id).unwrap()
}
fn work<'a>(response: &'a Value, task: &str) -> &'a Value {
    response["work"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["kind"] == "implement" && w["task_id"] == task)
        .unwrap_or_else(|| panic!("no implement work for {task}: {response}"))
}
/// Start a run, answer planning and return (run id, implement wave response).
fn start(root: &Path) -> (String, Value) {
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    let run_id = first["id"]
        .as_str()
        .unwrap_or_else(|| panic!("{first}"))
        .to_owned();
    assert!(first["work"][0]["run_id"].is_string(), "{first}");
    let wave = answer_plan(root, &first["work"][0]);
    (run_id, wave)
}
fn write_value(request: &Value) {
    let project = Path::new(request["project"].as_str().unwrap());
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/value.txt"), "42").unwrap();
}

/// Lets `git submodule` clone a local file:// repository in tests.
fn allow_file_protocol(config: &mut Config) {
    for (key, value) in [
        ("GIT_CONFIG_COUNT", "1"),
        ("GIT_CONFIG_KEY_0", "protocol.file.allow"),
        ("GIT_CONFIG_VALUE_0", "always"),
    ] {
        config.environment.insert(key.into(), value.into());
    }
}
/// A separate repository added as the `vendor/lib` submodule.
fn add_submodule(root: &Path) -> tempfile::TempDir {
    let library = tempfile::tempdir().unwrap();
    put(library.path(), "lib.txt", "shared library\n");
    git::command(library.path(), &["init", "-q", "-b", "main"], None).unwrap();
    git::commit(library.path(), "library").unwrap();
    git::command(
        root,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            library.path().to_str().unwrap(),
            "vendor/lib",
        ],
        None,
    )
    .unwrap();
    library
}

fn blocked(input: &WorkerInput, requested: &[&str]) -> WorkerResult {
    let mut result = receipt(input);
    result.status = "blocked".into();
    result.blockers = vec!["needs more files".into()];
    result.requested_writes = requested.iter().map(|s| s.to_string()).collect();
    result
}

#[test]
fn a_parent_directory_or_pattern_of_native_context_is_never_granted() {
    let root = fixture(|_| {});
    let root = root.path();
    let (run_id, wave) = start(root);
    let request = work(&wave, "value").clone();
    let stopped = submit(root, &request, blocked(&packet(root, &request), &[]));
    assert_eq!(stopped["status"], "needs_input", "{stopped}");
    for parent in [
        "specs/feature",
        "specs/feature/",
        "specs",
        "./specs//feature",
    ] {
        let error = api::invoke(
            root,
            "run.revise",
            &json!({"run_id":run_id,"reason":"widen","apply":true,"task_writes":[{"phase_id":"P001","task_id":"value","add":[parent]}]}),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("write_scope_forbidden"), "{parent}: {error}");
        assert!(error.contains("specs/feature/"), "{parent}: {error}");
    }
    assert_eq!(
        saved(root, &run_id).plans["P001"].tasks[0].writes,
        ["src/value.txt"]
    );
}

#[test]
fn blocked_workers_get_only_literal_file_paths_without_review() {
    for (requested, code) in [
        ("specs/feature", "write_scope_not_literal"),
        ("specs", "write_scope_not_literal"),
        ("specs/*/tasks.md", "write_scope_not_literal"),
        ("tests/*.mjs", "write_scope_not_literal"),
        ("tests/**", "write_scope_not_literal"),
        ("tests", "write_scope_not_literal"),
        ("tests/", "write_scope_not_literal"),
        // Created by the worker only: still a directory.
        ("fresh", "write_scope_not_literal"),
        // A literal file inside native context stays forbidden.
        ("specs/feature/tasks.md", "write_scope_forbidden"),
    ] {
        let root = fixture(|_| {});
        let root = root.path();
        let (run_id, wave) = start(root);
        let request = work(&wave, "value").clone();
        put(
            Path::new(request["project"].as_str().unwrap()),
            "fresh/a.txt",
            "x",
        );
        let stopped = submit(
            root,
            &request,
            blocked(&packet(root, &request), &[requested]),
        );
        assert_eq!(stopped["status"], "needs_input", "{requested}: {stopped}");
        let blocker = stopped["blocker"].as_str().unwrap();
        assert!(blocker.contains(code), "{requested}: {blocker}");
        assert!(blocker.contains("tools call run.revise"), "{blocker}");
        let run = saved(root, &run_id);
        assert_eq!(
            run.plans["P001"].tasks[0].writes,
            ["src/value.txt"],
            "{requested}: scope unchanged"
        );
        assert!(
            !run.host
                .as_ref()
                .unwrap()
                .verification_revisions
                .iter()
                .any(|r| r["automatic"] == true),
            "{requested}"
        );
    }
}

/// Arm a debug-build failpoint that fires once after the first executed check of a stage.
fn arm(root: &Path, name: &str) {
    put(root, &format!(".git/spec-autonomous/failpoints/{name}"), "");
}

#[test]
fn an_interruption_between_integration_checks_keeps_the_candidate() {
    for (failpoint, status, resume) in [
        ("pause_after_check-combined", "paused", json!({})),
        (
            "budget_after_check-combined",
            "paused",
            json!({"extend_seconds":3600}),
        ),
    ] {
        let root = fixture(|_| {});
        let root = root.path();
        let first = api::invoke(
            root,
            "prepare",
            &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
        )
        .unwrap();
        let run_id = first["id"].as_str().unwrap().to_owned();
        let planning = &first["work"][0];
        let input = packet(root, planning);
        let snapshot = input.snapshot.clone().unwrap();
        // Two checks, so the interruption lands after the first and before the second.
        let mut value = task("value", &snapshot.tasks[0].id, "src/value.txt");
        value.verification.push(Check {
            argv: vec!["node".into(), "-e".into(), "process.exit(0)".into()],
            cwd: ".".into(),
            env: Default::default(),
        });
        let mut result = receipt(&input);
        result.plan = Some(Plan {
            schema_version: 1,
            phase_id: "P001".into(),
            source_hash: snapshot.source_hash,
            tasks: vec![value, task("other", &snapshot.tasks[1].id, "src/other.txt")],
            milestone: input.milestone.clone(),
        });
        let wave = submit(root, planning, result);
        let request = work(&wave, "value").clone();
        write_value(&request);
        arm(root, failpoint);
        let stopped = submit(root, &request, receipt(&packet(root, &request)));
        assert_eq!(stopped["status"], status, "{failpoint}: {stopped}");
        assert!(
            !root
                .join(".git/spec-autonomous/failpoints")
                .join(failpoint)
                .exists(),
            "{failpoint} fired"
        );
        let run = saved(root, &run_id);
        let implement: Vec<_> = run
            .attempts
            .iter()
            .filter(|a| a.kind == "implement" && a.task_id == "value")
            .collect();
        assert_eq!(implement.len(), 1, "{failpoint}");
        assert_eq!(
            implement[0].status, "submitted",
            "{failpoint}: {:?}",
            implement[0]
        );
        assert!(implement[0].error.is_none(), "{failpoint}");
        assert!(
            run.evidence.iter().all(|e| e.exit_code == 0),
            "{failpoint}: only the first check ran"
        );

        let mut args = json!({"run_id":run_id,"host":host(),"view":"full"});
        args.as_object_mut()
            .unwrap()
            .extend(resume.as_object().unwrap().clone());
        let resumed = api::invoke(root, "prepare", &args).unwrap();
        let run = saved(root, &run_id);
        assert!(
            run.completed_tasks.contains(&Run::key("P001", "value")),
            "{failpoint}: {resumed}"
        );
        let implement: Vec<_> = run
            .attempts
            .iter()
            .filter(|a| a.kind == "implement" && a.task_id == "value")
            .collect();
        assert_eq!(implement.len(), 1, "{failpoint}: no re-implementation");
        assert_eq!(implement[0].status, "integrated");
        assert!(!run.attempts.iter().any(|a| a.status == "failed"));
    }
}

#[test]
fn cleanup_removes_clean_checkouts_with_initialized_submodules() {
    let library = std::cell::RefCell::new(None);
    let root = fixture_with(
        |c| {
            c.execution.init_submodules = true;
            c.execution.automatic_cleanup = false;
            allow_file_protocol(c);
        },
        |root| *library.borrow_mut() = Some(add_submodule(root)),
    );
    let root = root.path();
    let (run_id, wave) = start(root);
    let request = work(&wave, "value").clone();
    let worktree = request["worktree"].as_str().unwrap().to_owned();
    assert!(Path::new(&worktree).join("vendor/lib/lib.txt").is_file());
    write_value(&request);
    let accepted = submit(root, &request, receipt(&packet(root, &request)));
    assert_ne!(accepted["status"], "needs_input", "{accepted}");
    let other = work(&accepted, "other").clone();
    let dirty = other["worktree"].as_str().unwrap().to_owned();
    // The other worker left an untracked file inside its submodule.
    put(Path::new(&dirty), "vendor/lib/scratch.txt", "local");
    api::invoke(
        root,
        "work.revoke",
        &json!({"run_id":run_id,"request_id":other["request_id"],"host_stopped":true,"reason":"stop","host":other["owner"]}),
    )
    .unwrap();
    api::invoke(root, "run.cancel", &json!({"run_id":run_id})).unwrap();
    let integration = saved(root, &run_id).integration;
    assert!(Path::new(&integration).join("vendor/lib/lib.txt").is_file());

    let args = json!({"run_id":run_id,"delete_integration":true});
    let preview = api::invoke(root, "run.cleanup", &args).unwrap();
    let result = api::invoke(
        root,
        "run.cleanup",
        &json!({"run_id":run_id,"delete_integration":true,"apply":true,"plan_hash":preview["plan_hash"]}),
    )
    .unwrap();
    let removed: Vec<String> = serde_json::from_value(result["removed"].clone()).unwrap();
    let same = |a: &str, b: &str| {
        fs::canonicalize(a)
            .ok()
            .zip(fs::canonicalize(b).ok())
            .is_some_and(|(a, b)| a == b)
            || a == b
    };
    assert!(removed.iter().any(|p| same(p, &worktree)), "{result}");
    assert!(removed.iter().any(|p| same(p, &integration)), "{result}");
    assert!(!Path::new(&worktree).exists() && !Path::new(&integration).exists());
    // A submodule with local changes is never forced away.
    assert!(
        Path::new(&dirty).join("vendor/lib/scratch.txt").is_file(),
        "{result}"
    );
    assert!(!removed.iter().any(|p| same(p, &dirty)), "{result}");
    // The user's checkout keeps its initialized submodule.
    let status = git::command(root, &["submodule", "status"], None).unwrap();
    assert!(status.starts_with(' '), "{status}");
}

#[test]
fn doctor_warns_about_submodules_until_workers_get_them() {
    let library = std::cell::RefCell::new(None);
    let root = fixture_with(
        |_| {},
        |root| *library.borrow_mut() = Some(add_submodule(root)),
    );
    let root = root.path();
    let hook = |argv: &str| spec_autonomous_core::config::Hook {
        argv: vec!["sh".into(), argv.into()],
        idempotent: true,
    };
    let warning = |configure: &dyn Fn(&mut Config)| -> Option<String> {
        let mut config = Config::load(root).unwrap();
        configure(&mut config);
        put(
            root,
            ".spec-autonomous/config.toml",
            &toml::to_string(&config).unwrap(),
        );
        let report: Vec<String> = serde_json::from_value(
            api::invoke(root, "doctor", &json!({})).unwrap()["diagnostics"].clone(),
        )
        .unwrap();
        report
            .into_iter()
            .find(|d| d.starts_with("submodules_warning"))
    };
    // Friday's shape: verification checkouts are prepared, workers are not.
    let only_verification = warning(&|c| {
        c.hooks
            .insert("verification_prepare".into(), hook("scripts/v.sh"));
    })
    .expect("verification_prepare alone still leaves workers without submodules");
    assert!(only_verification.contains("worker"), "{only_verification}");
    assert!(only_verification.contains("[hooks.worker_prepare]"));
    assert!(
        warning(&|c| {
            c.hooks
                .insert("worker_prepare".into(), hook("scripts/w.sh"));
        })
        .is_none()
    );
    assert!(
        warning(&|c| {
            c.hooks.clear();
            c.execution.init_submodules = true;
        })
        .is_none()
    );
}

#[test]
fn skills_name_run_id_resumes_and_environment_blockers() {
    for name in ["autonomous", "auto", "resume"] {
        let body = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/cli/skills")
                .join(name)
                .join("SKILL.md"),
        )
        .unwrap();
        assert!(!body.contains("same `run_id`"), "{name}");
        assert!(!body.contains("same run_id"), "{name}");
        for expected in [
            "prepare --run-id <id>",
            "--change` / `--feature",
            "verification_environment_unavailable",
            "nothing is replanned",
            "worker_prepare_failed",
            "config_drift:",
            "reload_config: true",
        ] {
            assert!(body.contains(expected), "{name} must mention {expected}");
        }
    }
}

#[test]
fn an_automatic_grant_starts_a_new_retry_epoch_with_a_bounded_total() {
    // One attempt per epoch: without the reset the blocked attempt alone exhausts it.
    let root = fixture(|c| c.execution.max_attempts = 1);
    let root = root.path();
    let (run_id, wave) = start(root);
    let request = work(&wave, "value").clone();
    write_value(&request);
    let next = submit(
        root,
        &request,
        blocked(&packet(root, &request), &["tests/value.mjs"]),
    );
    assert_ne!(next["status"], "needs_input", "{next}");
    let retry = work(&next, "value").clone();
    assert_ne!(retry["request_id"], request["request_id"]);
    let run = saved(root, &run_id);
    assert_eq!(
        run.host.as_ref().unwrap().task_retry_epochs[&Run::key("P001", "value")],
        run.attempts
            .iter()
            .position(|a| a.id == retry["request_id"].as_str().unwrap())
            .unwrap(),
        "the epoch starts with the re-issued attempt"
    );
    // A real failure in the new epoch still counts and stops the task.
    put(
        Path::new(retry["project"].as_str().unwrap()),
        "src/value.txt",
        "41",
    );
    let failed = submit(root, &retry, receipt(&packet(root, &retry)));
    assert!(
        failed["work"]
            .as_array()
            .unwrap()
            .iter()
            .all(|w| w["task_id"] != "value" || w["kind"] != "implement"),
        "{failed}"
    );

    // Grants add at most ten paths per task over the whole run.
    let root = fixture(|_| {});
    let root = root.path();
    let (run_id, wave) = start(root);
    let request = work(&wave, "value").clone();
    let first: Vec<String> = (1..=6).map(|i| format!("tests/a{i}.mjs")).collect();
    let first: Vec<&str> = first.iter().map(String::as_str).collect();
    let next = submit(root, &request, blocked(&packet(root, &request), &first));
    let retry = work(&next, "value").clone();
    let second: Vec<String> = (1..=5).map(|i| format!("tests/b{i}.mjs")).collect();
    let second: Vec<&str> = second.iter().map(String::as_str).collect();
    let stopped = submit(root, &retry, blocked(&packet(root, &retry), &second));
    assert_eq!(stopped["status"], "needs_input", "{stopped}");
    let blocker = stopped["blocker"].as_str().unwrap();
    assert!(blocker.contains("write_scope_limit"), "{blocker}");
    assert!(blocker.contains("already received 6 paths"), "{blocker}");
    assert_eq!(saved(root, &run_id).plans["P001"].tasks[0].writes.len(), 7);
}

#[test]
fn a_failed_check_names_its_command_exit_and_redacted_output() {
    let script = "console.log('ran 3 cases');console.error('API_TOKEN=abc123secretvalue');console.error('using '+process.env.SA_TEST_SECRET);console.error('AssertionError: expected 42, got 41');process.exit(3)";
    let root = fixture(|c| {
        c.environment
            .insert("SA_TEST_SECRET".into(), "hunter2-secret-value".into());
    });
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    let run_id = first["id"].as_str().unwrap().to_owned();
    let planning = &first["work"][0];
    let input = packet(root, planning);
    let snapshot = input.snapshot.clone().unwrap();
    let mut value = task("value", &snapshot.tasks[0].id, "src/value.txt");
    value.verification = vec![Check {
        argv: vec!["node".into(), "-e".into(), script.into()],
        cwd: "src".into(),
        env: Default::default(),
    }];
    let mut result = receipt(&input);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        tasks: vec![value, task("other", &snapshot.tasks[1].id, "src/other.txt")],
        milestone: input.milestone.clone(),
    });
    put(root, "src/.keep", "");
    git::commit(root, "src dir").unwrap();
    let wave = submit(root, planning, result);
    let request = work(&wave, "value").clone();
    write_value(&request);
    let response = submit(root, &request, receipt(&packet(root, &request)));
    let run = saved(root, &run_id);
    let error = run
        .attempts
        .iter()
        .find(|a| a.kind == "implement" && a.task_id == "value" && a.status == "failed")
        .and_then(|a| a.error.clone())
        .unwrap_or_else(|| panic!("{response}"));
    assert!(error.starts_with("verification_failed: "), "{error}");
    for expected in [
        "command [\"node\", \"-e\", ",
        " in src exited 3",
        "stderr tail: ",
        "AssertionError: expected 42, got 41",
        "stdout tail: ran 3 cases",
        "; diagnostics ",
    ] {
        assert!(error.contains(expected), "{expected}: {error}");
    }
    assert!(!error.contains("abc123secretvalue"), "{error}");
    assert!(!error.contains("hunter2-secret-value"), "{error}");
    assert!(error.contains("API_TOKEN=[redacted]"), "{error}");
    // no_progress blockers keep the first 600 characters of each failure: the reason
    // comes before the long diagnostics paths.
    let head: String = error.chars().take(600).collect();
    assert!(
        head.contains("AssertionError: expected 42, got 41"),
        "{head}"
    );
    // The re-issued packet carries the same text as its failure.
    let retry = packet(root, work(&response, "value"));
    assert_eq!(retry.failure.as_deref(), Some(error.as_str()));
}

#[test]
fn planners_and_workers_learn_how_a_mapped_openspec_runs() {
    use std::os::unix::fs::PermissionsExt;
    let tools = tempfile::tempdir().unwrap();
    let fake = tools.path().join("managed-openspec");
    fs::write(&fake, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    // Checks see node and git but no openspec on PATH.
    let bin = tools.path().join("bin");
    fs::create_dir(&bin).unwrap();
    for name in ["node", "git"] {
        let found = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
            .unwrap();
        std::os::unix::fs::symlink(found, bin.join(name)).unwrap();
    }
    let fake_argv = fake.to_string_lossy().into_owned();
    let root = fixture(|c| {
        c.provider.openspec_command = vec![fake_argv.clone()];
        c.runner
            .environment
            .insert("PATH".into(), bin.to_string_lossy().into_owned());
    });
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    let planning = &first["work"][0];
    let plan_input: Value =
        serde_json::from_slice(&fs::read(planning["input_path"].as_str().unwrap()).unwrap())
            .unwrap();
    let environment = &plan_input["snapshot"]["metadata"]["verification_environment"];
    for name in ["openspec", "uv", "pnpm"] {
        assert!(
            environment["executables"].get(name).is_some(),
            "{name}: {environment}"
        );
    }
    assert_eq!(
        environment["managed_commands"]["openspec"]["argv"],
        json!([fake_argv]),
        "{environment}"
    );
    let plan_prompt = fs::read_to_string(planning["prompt_path"].as_str().unwrap()).unwrap();
    assert!(
        plan_prompt.contains("TOOLS: `openspec` is not on PATH"),
        "{plan_prompt}"
    );

    let wave = answer_plan(root, planning);
    let request = work(&wave, "value");
    let input: Value =
        serde_json::from_slice(&fs::read(request["input_path"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(
        input["snapshot"]["metadata"]["managed_commands"]["openspec"]["argv"],
        json!([fake_argv])
    );
    let prompt = fs::read_to_string(request["prompt_path"].as_str().unwrap()).unwrap();
    for expected in [
        "TOOLS: `openspec` is not on PATH",
        fake_argv.as_str(),
        "spec-autonomous providers exec openspec -- <args>",
    ] {
        assert!(prompt.contains(expected), "{expected}\n{prompt}");
    }
    // The packet still verifies against its recorded input hash.
    assert_eq!(packet(root, request).task.unwrap().id, "value");
}

/// Rewrite a stored run, as an older binary or a long absence would have left it.
fn edit_run(root: &Path, id: &str, edit: impl FnOnce(&mut Run)) {
    let repo = Repository::discover(root).unwrap();
    let _lease = spec_autonomous_core::state::Lease::acquire(&repo).unwrap();
    let mut store = Store::open(&repo, true).unwrap().unwrap();
    let mut run = store.get(id).unwrap();
    edit(&mut run);
    store.save(&run, "test_edit").unwrap();
}
fn resume(root: &Path, id: &str) -> Value {
    api::invoke(
        root,
        "prepare",
        &json!({"run_id":id,"host":host(),"view":"full"}),
    )
    .unwrap()
}
fn exhausted(response: &Value) -> bool {
    response["blocker"]
        .as_str()
        .is_some_and(|b| b.starts_with("budget_exhausted:"))
}

#[test]
fn the_run_budget_counts_active_time_only() {
    // Time stopped in paused (or needs_input) is not spent.
    let root = fixture(|c| c.execution.run_timeout_seconds = 6);
    let root = root.path();
    let (run_id, _) = start(root);
    api::invoke(root, "run.pause", &json!({"run_id":run_id})).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(6500));
    let resumed = resume(root, &run_id);
    assert!(!exhausted(&resumed), "{resumed}");
    let run = saved(root, &run_id);
    assert!(run.elapsed_ms < 6000, "{}", run.elapsed_ms);
    assert!(run.active_at.is_some());
    let remaining = resumed["work"][0]["limits"]["run_remaining_ms"]
        .as_u64()
        .unwrap();
    assert!(remaining > 0 && remaining <= 6000, "{resumed}");

    // Time the host holds issued packets is active and does spend the budget.
    std::thread::sleep(std::time::Duration::from_millis(6500));
    let stopped = resume(root, &run_id);
    assert!(exhausted(&stopped), "{stopped}");
    let blocker = stopped["blocker"].as_str().unwrap();
    assert!(blocker.contains("s elapsed (active time:"), "{blocker}");
    assert!(
        blocker.contains("stopped for input or paused is not counted"),
        "{blocker}"
    );
    assert!(
        !blocker.contains("unsafe_budgets = true in the project config"),
        "{blocker}"
    );
}

#[test]
fn legacy_runs_resume_from_recorded_time_and_cap_hints_name_unsafe_budgets() {
    let root = fixture(|c| c.execution.run_timeout_seconds = 3600);
    let root = root.path();
    let (run_id, _) = start(root);
    // A run written before active-time accounting: started long ago, no active_at.
    edit_run(root, &run_id, |run| {
        run.started_at = "2026-01-01T00:00:00.000Z".into();
        run.elapsed_ms = 1000;
        run.active_at = None;
    });
    let resumed = resume(root, &run_id);
    assert!(!exhausted(&resumed), "{resumed}");
    assert!(saved(root, &run_id).elapsed_ms < 60_000);

    // At the 72h cap the extension hint says unsafe_budgets is required.
    let cap = spec_autonomous_core::config::RUN_TIMEOUT_CAP_SECONDS;
    edit_run(root, &run_id, |run| {
        run.config.execution.run_timeout_seconds = cap;
        run.elapsed_ms = cap * 1000 + 5000;
        run.status = "paused".into();
    });
    let stopped = resume(root, &run_id);
    assert!(exhausted(&stopped), "{stopped}");
    let blocker = stopped["blocker"].as_str().unwrap();
    assert!(
        blocker.contains(
            "without [execution] unsafe_budgets = true, so this run needs unsafe_budgets = true"
        ),
        "{blocker}"
    );
    // Below the cap it says how much an extension may still add.
    edit_run(root, &run_id, |run| {
        run.config.execution.run_timeout_seconds = cap - 100;
        run.elapsed_ms = (cap - 100) * 1000 + 5000;
    });
    let stopped = resume(root, &run_id);
    let blocker = stopped["blocker"].as_str().unwrap();
    assert!(
        blocker.contains("extend_seconds can add at most 100"),
        "{blocker}"
    );
}

#[test]
fn baseline_checks_report_failures_that_predate_the_plan() {
    let stale = "import assert from 'node:assert/strict';import {writeFileSync} from 'node:fs';writeFileSync('leftover.txt','x');assert.equal('volar','gopls','stale expectation in settings test');\n";
    for enabled in [true, false] {
        let root = fixture_with(
            |c| c.execution.baseline_checks = enabled,
            |root| put(root, "tests/stale.mjs", stale),
        );
        let root = root.path();
        let first = api::invoke(
            root,
            "prepare",
            &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
        )
        .unwrap();
        let run_id = first["id"].as_str().unwrap().to_owned();
        let planning = &first["work"][0];
        let input = packet(root, planning);
        let snapshot = input.snapshot.clone().unwrap();
        let mut other = task("other", &snapshot.tasks[1].id, "src/other.txt");
        other.writes.push("tests/new.mjs".into());
        other.verification = vec![
            Check {
                argv: vec!["node".into(), "tests/stale.mjs".into()],
                cwd: ".".into(),
                env: Default::default(),
            },
            // Written by the task itself: deferred, so the baseline skips it.
            Check {
                argv: vec!["node".into(), "tests/new.mjs".into()],
                cwd: ".".into(),
                env: Default::default(),
            },
        ];
        let mut result = receipt(&input);
        result.plan = Some(Plan {
            schema_version: 1,
            phase_id: "P001".into(),
            source_hash: snapshot.source_hash,
            tasks: vec![task("value", &snapshot.tasks[0].id, "src/value.txt"), other],
            milestone: input.milestone.clone(),
        });
        let wave = submit(root, planning, result);
        let run = saved(root, &run_id);
        let warnings: Vec<String> =
            serde_json::from_value(wave["warnings"].clone()).unwrap_or_default();
        if !enabled {
            assert!(run.host.as_ref().unwrap().baselines.is_empty());
            assert!(
                !warnings.iter().any(|w| w.starts_with("baseline_failure")),
                "{wave}"
            );
            continue;
        }
        let baseline = &run.host.as_ref().unwrap().baselines["P001"];
        assert_eq!(baseline["head"], json!(run.accepted_head), "{baseline}");
        // node --version, tests/value.mjs (src/value.txt is not written yet) and the stale test.
        assert_eq!(baseline["checks_run"], 3, "{baseline}");
        let failed: Vec<Value> = baseline["failures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["argv"].clone())
            .collect();
        assert_eq!(
            failed,
            [
                json!(["node", "--test", "tests/value.mjs"]),
                json!(["node", "tests/stale.mjs"])
            ],
            "{baseline}"
        );
        let stale_warning = warnings
            .iter()
            .find(|w| w.contains("tests/stale.mjs") && w.contains("actual: 'volar'"))
            .unwrap_or_else(|| panic!("{wave}"));
        assert!(
            stale_warning.starts_with("baseline_failure: phase P001: "),
            "{stale_warning}"
        );
        assert!(
            !warnings.iter().any(|w| w.contains("tests/new.mjs")),
            "{warnings:?}"
        );
        assert!(
            git::clean(Path::new(&run.integration)).unwrap(),
            "the baseline leaves the integration checkout clean"
        );
        let implement = packet(root, work(&wave, "other"));
        let known = &implement.snapshot.unwrap().metadata["baseline_failures"];
        assert_eq!(known.as_array().unwrap().len(), 2, "{known}");
        // Nothing failed or retried because of the baseline.
        assert!(!run.attempts.iter().any(|a| a.status == "failed"));
        assert_ne!(wave["status"], "needs_input", "{wave}");
    }
}

#[test]
fn ledger_schema_5_upgrades_a_schema_4_ledger_without_losing_rows() {
    assert_eq!(spec_autonomous_core::state::SCHEMA_VERSION, 5);
    let root = fixture(|c| c.execution.init_submodules = false);
    let root = root.path();
    let (run_id, wave) = start(root);
    let request = work(&wave, "value").clone();
    write_value(&request);
    submit(root, &request, receipt(&packet(root, &request)));
    let before = saved(root, &run_id);
    assert!(before.attempts.len() >= 3 && !before.evidence.is_empty());
    let repo = Repository::discover(root).unwrap();
    let runtime = repo.runtime().unwrap();
    let version = |path: &Path| -> u32 {
        rusqlite::Connection::open(path)
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap()
    };
    assert_eq!(version(&runtime.join("state.db")), 5);
    // The same tables as alpha.16 (schema 4) wrote them.
    rusqlite::Connection::open(runtime.join("state.db"))
        .unwrap()
        .pragma_update(None, "user_version", 4)
        .unwrap();

    // Readers and control writers use the split rows of a schema-4 ledger as they are.
    let reader = Store::open(&repo, false).unwrap().unwrap();
    let read = reader.get(&run_id).unwrap();
    assert_eq!(read.attempts.len(), before.attempts.len());
    assert_eq!(read.evidence.len(), before.evidence.len());
    drop(reader);
    spec_autonomous_core::state::Store::for_control(&repo)
        .unwrap()
        .control(&run_id, "")
        .unwrap();
    assert_eq!(
        version(&runtime.join("state.db")),
        4,
        "reading never migrates"
    );

    // The first writer backs the ledger up and only stamps the new version.
    let store = Store::open(&repo, true).unwrap().unwrap();
    assert_eq!(store.user_version().unwrap(), 5);
    let migrated = store.get(&run_id).unwrap();
    assert_eq!(
        migrated.attempts.len(),
        before.attempts.len(),
        "no attempt rows lost"
    );
    assert_eq!(
        migrated.evidence.len(),
        before.evidence.len(),
        "no evidence rows lost"
    );
    drop(store);
    let backups: Vec<_> = fs::read_dir(&runtime)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            name.starts_with("state.v4.") && name.ends_with(".backup.db")
        })
        .collect();
    assert_eq!(backups.len(), 1, "{backups:?}");
    assert_eq!(version(&backups[0]), 4);
    // The run continues normally on the upgraded ledger.
    let resumed = api::invoke(
        root,
        "prepare",
        &json!({"run_id":run_id,"host":host(),"view":"full"}),
    )
    .unwrap();
    assert_ne!(resumed["status"], "needs_input", "{resumed}");
}
