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
        verification: vec![if id == "value" {
            value_check()
        } else {
            Check {
                argv: vec!["node".into(), "--version".into()],
                cwd: ".".into(),
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
