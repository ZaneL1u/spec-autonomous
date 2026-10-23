//! Stage-scoped project checks, inline-script rejection, preflight for future scripts,
//! planner environment facts and budget caps. Real Git, real Node, no agents.
use serde_json::{Value, json};
use spec_autonomous_core::{
    capabilities as api,
    config::{self, Config},
    engine,
    git::{self, Repository},
    model::*,
    state::Store,
    verification_preflight,
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
fn tagged_counter(tag: &str) -> Check {
    Check {
        argv: vec![
            "node".into(),
            "-e".into(),
            format!(
                "require('fs').appendFileSync(process.env.SA_TEST_COUNTER,process.env.SA_VERIFY_STAGE+':{tag}\\n')"
            ),
        ],
        cwd: ".".into(),
    }
}
fn fixture(counter: &Path, configure: impl FnOnce(&mut Config)) -> tempfile::TempDir {
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
        "## Implementation\n- [ ] T001 Write 42 in src/value.txt\n",
    );
    put(
        root.path(),
        "tests/value.mjs",
        "import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';assert.equal(readFileSync('src/value.txt','utf8'),'42');\n",
    );
    put(
        root.path(),
        "package.json",
        "{\"name\":\"fixture\",\"private\":true,\"scripts\":{\"test\":\"node --test tests/value.mjs\",\"lint\":\"true\"}}\n",
    );
    put(
        root.path(),
        ".gitignore",
        "node_modules/\n.spec-autonomous/*\n!.spec-autonomous/config.toml\n!.spec-autonomous/milestones/\n!.spec-autonomous/plans/\n",
    );
    let mut config = Config::default();
    config.runner.profile = "command".into();
    config.runner.command = vec!["must-never-start-this-agent".into()];
    config.runner.fresh_session = true;
    config.runner.environment.insert(
        "SA_TEST_COUNTER".into(),
        counter.to_string_lossy().into_owned(),
    );
    config.verification = vec![value_check()];
    configure(&mut config);
    put(
        root.path(),
        ".spec-autonomous/config.toml",
        &toml::to_string(&config).unwrap(),
    );
    git::command(root.path(), &["init", "-q", "-b", "main"], None).unwrap();
    git::commit(root.path(), "fixture").unwrap();
    root
}
fn stages(counter: &Path) -> Vec<String> {
    fs::read_to_string(counter)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}
fn owner(request: &Value) -> Value {
    json!({"host_id":"test-host","session_id":request["request_id"],"fresh_context":true})
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
    }
}
fn submit(root: &Path, request: &Value, result: WorkerResult) -> Value {
    api::invoke(
        root,
        "apply-result",
        &json!({"token":request["token"],"host":owner(request),"result":result,"view":"full"}),
    )
    .unwrap()
}
fn plan_with(input: &WorkerInput, checks: Vec<Check>, writes: Vec<String>) -> WorkerResult {
    let snapshot = input.snapshot.clone().unwrap();
    let mut result = receipt(input);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        tasks: vec![Task {
            id: "value".into(),
            description: "write value".into(),
            source_ids: vec![snapshot.tasks[0].id.clone()],
            depends_on: vec![],
            reads: vec![],
            writes,
            verification: checks,
        }],
        milestone: input.milestone.clone(),
    });
    result
}
fn answer_audit(root: &Path, request: &Value) -> Value {
    let input = packet(root, request);
    let snapshot = input.snapshot.clone().unwrap();
    let mut result = receipt(&input);
    result.audit = snapshot.metadata["acceptance"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| AuditItem {
            requirement: r["id"].as_str().unwrap().into(),
            evidence: "src/value.txt".into(),
            passed: true,
        })
        .collect();
    submit(root, request, result)
}
fn saved(root: &Path, id: &str) -> Run {
    let repo = Repository::discover(root).unwrap();
    Store::open(&repo, false).unwrap().unwrap().get(id).unwrap()
}

#[test]
fn stage_scoped_project_checks_run_only_at_their_stage() {
    let scratch = tempfile::tempdir().unwrap();
    let counter = scratch.path().join("stages.log");
    let root = fixture(&counter, |config| {
        config.task_verification = vec![tagged_counter("task")];
        config.phase_verification = vec![tagged_counter("phase")];
        config.milestone_verification = vec![tagged_counter("milestone")];
    });
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full"}),
    )
    .unwrap();
    let planner = first["work"][0].clone();
    let input = packet(root, &planner);
    let planned = submit(
        root,
        &planner,
        plan_with(&input, vec![value_check()], vec!["src/value.txt".into()]),
    );
    let request = planned["work"][0].clone();
    put(
        Path::new(request["worktree"].as_str().unwrap()),
        "src/value.txt",
        "42",
    );
    let input = packet(root, &request);
    let audit = submit(root, &request, receipt(&input));
    assert_eq!(audit["work"][0]["kind"], "audit", "{audit}");
    // Task-scope checks ran with the worker (combined reused them); the phase gate ran
    // its own scope once; nothing milestone-scoped yet.
    assert_eq!(stages(&counter), ["worker:task", "phase:phase"]);
    let milestone = answer_audit(root, &audit["work"][0]);
    assert_eq!(milestone["work"][0]["kind"], "audit", "{milestone}");
    assert_eq!(
        stages(&counter),
        ["worker:task", "phase:phase", "milestone:milestone"]
    );
    let done = answer_audit(root, &milestone["work"][0]);
    assert_eq!(done["status"], "completed", "{done}");
    let run = saved(root, done["id"].as_str().unwrap());
    let argv_at = |stage: &str| -> Vec<String> {
        run.evidence
            .iter()
            .filter(|e| e.details.as_ref().is_some_and(|d| d["stage"] == stage))
            .map(|e| e.argv.join(" "))
            .collect()
    };
    assert!(argv_at("worker").iter().any(|a| a.contains(":task")));
    assert!(argv_at("worker").iter().all(|a| !a.contains(":phase")));
    assert!(argv_at("phase").iter().any(|a| a.contains(":phase")));
    assert!(argv_at("phase").iter().all(|a| !a.contains(":task")));
    assert!(
        argv_at("milestone")
            .iter()
            .any(|a| a.contains(":milestone"))
    );
    assert!(
        argv_at("milestone")
            .iter()
            .all(|a| !a.contains(":phase") && !a.contains(":task"))
    );
}

#[test]
fn inline_encoded_scripts_are_rejected_as_checks() {
    let smuggled = Check {
        argv: vec![
            "python3".into(),
            "-c".into(),
            "import base64,zlib;exec(zlib.decompress(base64.b85decode('c-oa#O^@3)5')))".into(),
        ],
        cwd: ".".into(),
    };
    let error = config::validate_check(&smuggled).unwrap_err().to_string();
    assert!(error.contains("inline_script_argv"), "{error}");
    let oversized = Check {
        argv: vec!["node".into(), "-e".into(), "x".repeat(4000)],
        cwd: ".".into(),
    };
    assert!(
        config::validate_check(&oversized)
            .unwrap_err()
            .to_string()
            .contains("inline_script_argv")
    );
    // Short inline assertions and ordinary file checks stay valid.
    config::validate_check(&tagged_counter("ok")).unwrap();
    config::validate_check(&value_check()).unwrap();
    let config = Config {
        verification: vec![smuggled],
        ..Default::default()
    };
    assert!(
        config
            .validate()
            .unwrap_err()
            .to_string()
            .contains("inline_script_argv")
    );
}

#[test]
fn preflight_defers_scripts_and_executables_the_task_will_write() {
    let scratch = tempfile::tempdir().unwrap();
    let root = fixture(&scratch.path().join("unused.log"), |_| {});
    let root = root.path();
    let task = |writes: Vec<&str>, argv: Vec<&str>| Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: "s".into(),
        tasks: vec![Task {
            id: "t".into(),
            description: "d".into(),
            source_ids: vec!["T001".into()],
            depends_on: vec![],
            reads: vec![],
            writes: writes.into_iter().map(String::from).collect(),
            verification: vec![Check {
                argv: argv.into_iter().map(String::from).collect(),
                cwd: ".".into(),
            }],
        }],
        milestone: None,
    };
    let env = std::collections::BTreeMap::new();
    let deferred = verification_preflight::inspect_plan_with_environment(
        root,
        &task(
            vec!["src/value.txt", "tests/check_value.py"],
            vec!["python3", "tests/check_value.py"],
        ),
        &[],
        &env,
    );
    assert!(deferred.ready, "{deferred:?}");
    assert_eq!(deferred.status, "deferred");
    assert!(
        deferred
            .diagnostics
            .iter()
            .any(|d| d.code == "check_script_deferred")
    );
    let missing = verification_preflight::inspect_plan_with_environment(
        root,
        &task(
            vec!["src/value.txt"],
            vec!["python3", "tests/check_value.py"],
        ),
        &[],
        &env,
    );
    assert!(!missing.ready, "{missing:?}");
    assert!(
        missing
            .diagnostics
            .iter()
            .any(|d| d.code == "check_script_missing" && d.severity == "error")
    );
    let future_tool = verification_preflight::inspect_plan_with_environment(
        root,
        &task(vec!["run-checks"], vec!["run-checks"]),
        &[],
        &env,
    );
    assert!(future_tool.ready, "{future_tool:?}");
    assert!(
        future_tool
            .diagnostics
            .iter()
            .any(|d| d.code == "check_executable_deferred")
    );
}

#[test]
fn planner_environment_reports_real_versions_scripts_tests_and_ignores() {
    let scratch = tempfile::tempdir().unwrap();
    let root = fixture(&scratch.path().join("unused.log"), |_| {});
    let env = verification_preflight::environment(root.path());
    assert!(
        env["runtime_versions"]["node"]
            .as_str()
            .is_some_and(|v| v.starts_with('v')),
        "{env}"
    );
    assert!(
        env["runtime_versions"]["git"]
            .as_str()
            .is_some_and(|v| v.contains("git"))
    );
    let scripts: Vec<&str> = env["package_scripts"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(scripts, ["lint", "test"]);
    assert!(
        env["test_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "tests/value.mjs")
    );
    assert_eq!(env["test_file_count"], 1);
    assert_eq!(env["ignored_directories"]["node_modules"], true);
    assert_eq!(env["ignored_directories"]["__pycache__"], false);
    assert!(
        env["manifests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "package.json")
    );
}

#[test]
fn planner_packets_carry_every_earlier_rejection_by_diagnostic_code() {
    let scratch = tempfile::tempdir().unwrap();
    let root = fixture(&scratch.path().join("unused.log"), |_| {});
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full"}),
    )
    .unwrap();
    let mut request = first["work"][0].clone();
    let id = first["id"].as_str().unwrap().to_string();
    for missing in ["sa-missing-check-one", "sa-missing-check-two"] {
        let input = packet(root, &request);
        let rejected = submit(
            root,
            &request,
            plan_with(
                &input,
                vec![Check {
                    argv: vec![missing.into()],
                    cwd: ".".into(),
                }],
                vec!["src/value.txt".into()],
            ),
        );
        assert_eq!(rejected["status"], "needs_input", "{rejected}");
        let resumed = api::invoke(root, "prepare", &json!({"run_id":id,"view":"full"})).unwrap();
        request = resumed["work"][0].clone();
        assert_eq!(request["kind"], "plan-tasks");
    }
    let input = packet(root, &request);
    let metadata = &input.snapshot.unwrap().metadata;
    let history = metadata["previous_planning_failures"].as_array().unwrap();
    assert_eq!(history.len(), 2, "{metadata}");
    assert!(history.iter().all(|h| {
        h["codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "check_executable_missing")
    }));
    assert!(
        metadata["verification_environment"]["runtime_versions"]["node"]
            .as_str()
            .is_some()
    );
}

#[test]
fn budgets_are_capped_unless_explicitly_unsafe_and_doctor_warns_above_twice_default() {
    let mut config = Config::default();
    config.execution.max_attempts = 20;
    config.execution.max_repair_rounds = 8;
    config.execution.run_timeout_seconds = 878_400;
    let error = config.validate().unwrap_err().to_string();
    assert!(
        error.contains("budget_cap") && error.contains("unsafe_budgets"),
        "{error}"
    );
    config.execution.unsafe_budgets = true;
    config.validate().unwrap();
    let warnings = config.budget_warnings();
    assert!(warnings.iter().any(|w| w.contains("max_attempts=20")));
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("run_timeout_seconds=878400"))
    );
    assert!(warnings.iter().any(|w| w.contains("unsafe_budgets=true")));
    let mut modest = Config::default();
    modest.execution.max_attempts = 8;
    modest.validate().unwrap();
    assert_eq!(modest.budget_warnings().len(), 1);
    assert!(Config::default().budget_warnings().is_empty());

    let scratch = tempfile::tempdir().unwrap();
    let root = fixture(&scratch.path().join("unused.log"), |config| {
        config.execution.max_attempts = 8;
    });
    let report = api::invoke(root.path(), "doctor", &json!({})).unwrap();
    let diagnostics: Vec<&str> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        diagnostics
            .iter()
            .any(|d| d.starts_with("budget_warning: max_attempts=8")),
        "{diagnostics:?}"
    );
}
