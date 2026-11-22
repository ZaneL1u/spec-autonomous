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
