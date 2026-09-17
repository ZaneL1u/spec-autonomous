use serde_json::{Value, json};
use spec_autonomous_core::{
    capabilities as api, config::Config, engine, git, model::*, state::Store,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};
fn put(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}
fn fixture() -> tempfile::TempDir {
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
        ".gitignore",
        ".spec-autonomous/*\n!.spec-autonomous/config.toml\n!.spec-autonomous/milestones/\n!.spec-autonomous/plans/\n",
    );
    let mut config = Config::default();
    config.runner.profile = "command".into();
    config.runner.command = vec!["must-never-start-this-agent".into()];
    config.runner.fresh_session = true;
    config.verification = vec![Check {
        argv: vec!["node".into(), "--test".into(), "tests/value.mjs".into()],
        cwd: ".".into(),
    }];
    put(
        root.path(),
        ".spec-autonomous/config.toml",
        &toml::to_string(&config).unwrap(),
    );
    git::command(root.path(), &["init", "-q", "-b", "main"], None).unwrap();
    git::commit(root.path(), "fixture").unwrap();
    root
}
fn prepare(root: &Path) -> Value {
    api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full"}),
    )
    .unwrap()
}
fn owner(request: &Value) -> Value {
    json!({"host_id":"test-host","session_id":request["request_id"],"fresh_context":true})
}
fn receipt(root: &Path, request: &Value) -> WorkerResult {
    let input: WorkerInput = serde_json::from_value(
        engine::work_context(
            root,
            request["run_id"].as_str().unwrap(),
            request["request_id"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    WorkerResult {
        schema_version: 1,
        run_id: input.run_id,
        task_id: input.task_id,
        attempt_id: input.attempt_id,
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
fn answer_plan(root: &Path, request: &Value) -> Value {
    let input: WorkerInput = serde_json::from_value(
        engine::work_context(
            root,
            request["run_id"].as_str().unwrap(),
            request["request_id"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let snapshot = input.snapshot.unwrap();
    let mut result = receipt(root, request);
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
            writes: vec!["src/value.txt".into()],
            verification: vec![Check {
                argv: vec!["node".into(), "--test".into(), "tests/value.mjs".into()],
                cwd: ".".into(),
            }],
        }],
        milestone: input.milestone,
    });
    submit(root, request, result)
}
#[test]
fn prepare_is_idempotent_and_never_requires_an_agent_executable() {
    let root = fixture();
    let first = prepare(root.path());
    assert_eq!(first["status"], "awaiting_host");
    assert_eq!(first["starts_agents"], false);
    let second = prepare(root.path());
    assert_eq!(first["id"], second["id"]);
    for key in [
        "request_id",
        "token",
        "input_hash",
        "input_path",
        "worktree",
    ] {
        assert_eq!(first["work"][0][key], second["work"][0][key]);
    }
    assert_eq!(
        first["attempts"].as_array().unwrap().len(),
        second["attempts"].as_array().unwrap().len()
    );
    assert_eq!(first["work"][0]["kind"], "plan-tasks");
    let next = api::invoke(root.path(), "next", &json!({"run_id":first["id"]})).unwrap();
    assert!(next["work"][0].get("token").is_none());
    assert!(
        !Path::new(first["work"][0]["input_path"].as_str().unwrap())
            .with_file_name("process.json")
            .exists()
    );
}
#[test]
fn pause_mailbox_is_acknowledged_when_a_receipt_arrives_after_lock_contention() {
    let root = fixture();
    let first = prepare(root.path());
    let request = &first["work"][0];
    let repo = git::Repository::discover(root.path()).unwrap();
    let coordinator = spec_autonomous_core::state::Lease::acquire(&repo).unwrap();
    engine::host_control(root.path(), first["id"].as_str().unwrap(), "pause").unwrap();
    assert_eq!(
        Store::open(&repo, false)
            .unwrap()
            .unwrap()
            .get(first["id"].as_str().unwrap())
            .unwrap()
            .status,
        "awaiting_host"
    );
    drop(coordinator);
    let paused = answer_plan(root.path(), request);
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["attempts"][0]["status"], "submitted");
    let replay = answer_plan(root.path(), request);
    assert_eq!(replay["status"], "paused");
    assert_eq!(replay["attempts"].as_array().unwrap().len(), 1);
    let next = api::invoke(root.path(), "next", &json!({"run_id":first["id"]})).unwrap();
    assert_eq!(next["status"], "paused");
}
#[test]
fn receipts_bind_identity_and_replay_without_duplicate_integration() {
    let root = fixture();
    let first = prepare(root.path());
    let p = &first["work"][0];
    let next = answer_plan(root.path(), p);
    let work = &next["work"][0];
    put(
        Path::new(work["project"].as_str().unwrap()),
        "src/value.txt",
        "42",
    );
    let result = receipt(root.path(), work);
    let accepted = submit(root.path(), work, result.clone());
    assert_eq!(accepted["status"], "awaiting_host");
    assert_eq!(accepted["completed_tasks"].as_array().unwrap().len(), 1);
    let head = accepted["accepted_head"].clone();
    let replay = submit(root.path(), work, result.clone());
    assert_eq!(replay["receipt_replayed"], true);
    assert_eq!(replay["accepted_head"], head);
    let mut changed = result;
    changed.summary = "different receipt".into();
    let error = api::invoke(
        root.path(),
        "apply-result",
        &json!({"token":work["token"],"host":owner(work),"result":changed}),
    )
    .unwrap_err();
    assert!(error.to_string().contains("receipt_conflict"));
    assert!(
        !root.path().join("src/value.txt").exists(),
        "final audit has not delivered the original checkout"
    );
}
#[test]
fn reused_sessions_and_corrupt_packets_are_rejected_before_acceptance() {
    let root = fixture();
    let first = prepare(root.path());
    let p = &first["work"][0];
    let next = answer_plan(root.path(), p);
    let work = &next["work"][0];
    let result = receipt(root.path(), work);
    let error = api::invoke(
        root.path(),
        "apply-result",
        &json!({"token":work["token"],"host":owner(p),"result":result}),
    )
    .unwrap_err();
    assert!(error.to_string().contains("host_session_reused"));
    let file = PathBuf::from(work["input_path"].as_str().unwrap());
    fs::write(&file, "{}").unwrap();
    assert!(
        engine::work_context(
            root.path(),
            work["run_id"].as_str().unwrap(),
            work["request_id"].as_str().unwrap()
        )
        .unwrap_err()
        .to_string()
        .contains("context_corrupt")
    );
}
#[test]
fn stale_work_requires_liveness_or_stop_acknowledgement_and_is_not_redispatched() {
    let root = fixture();
    let first = prepare(root.path());
    let request = &first["work"][0];
    let repo = git::Repository::discover(root.path()).unwrap();
    let mut store = Store::open(&repo, true).unwrap().unwrap();
    let mut run = store.get(first["id"].as_str().unwrap()).unwrap();
    run.host
        .as_mut()
        .unwrap()
        .requests
        .get_mut(request["request_id"].as_str().unwrap())
        .unwrap()
        .heartbeat_at_ms = 0;
    store.save(&run, "test-stale").unwrap();
    drop(store);
    let again = prepare(root.path());
    assert_eq!(again["work"][0]["stale"], true);
    assert_eq!(again["work"][0]["request_id"], request["request_id"]);
    let bad = api::invoke(
        root.path(),
        "work.revoke",
        &json!({"run_id":first["id"],"request_id":request["request_id"],"token":request["token"],"host_stopped":false,"reason":"unknown"}),
    );
    assert!(bad.is_err());
    api::invoke(root.path(),"work.revoke",&json!({"run_id":first["id"],"request_id":request["request_id"],"token":request["token"],"host_stopped":true,"reason":"host confirmed stopped"})).unwrap();
    let new = api::invoke(
        root.path(),
        "prepare",
        &json!({"run_id":first["id"],"view":"full"}),
    )
    .unwrap();
    assert_ne!(new["work"][0]["request_id"], request["request_id"]);
}
#[test]
fn pause_cancel_and_manual_blockers_preserve_host_ownership() {
    let root = fixture();
    let first = prepare(root.path());
    let req = &first["work"][0];
    api::invoke(
        root.path(),
        "state.block",
        &json!({"run_id":first["id"],"blocker_id":"decision","reason":"need a product decision"}),
    )
    .unwrap();
    let stopped = api::invoke(root.path(), "prepare", &json!({"run_id":first["id"]})).unwrap();
    assert_eq!(stopped["status"], "needs_input");
    api::invoke(
        root.path(),
        "state.unblock",
        &json!({"run_id":first["id"],"blocker_id":"decision"}),
    )
    .unwrap();
    api::invoke(root.path(), "run.cancel", &json!({"run_id":first["id"]})).unwrap();
    let next = api::invoke(root.path(), "next", &json!({"run_id":first["id"]})).unwrap();
    assert_eq!(next["action"], "stop_host_work");
    let result = receipt(root.path(), req);
    assert!(
        api::invoke(
            root.path(),
            "apply-result",
            &json!({"token":req["token"],"host":owner(req),"result":result})
        )
        .is_err()
    );
}
#[test]
fn legacy_records_remain_readable_but_never_reactivate_a_launcher() {
    let root = fixture();
    let first = prepare(root.path());
    let repo = git::Repository::discover(root.path()).unwrap();
    let mut store = Store::open(&repo, true).unwrap().unwrap();
    let mut run = store.get(first["id"].as_str().unwrap()).unwrap();
    run.host = None;
    store.save(&run, "legacy-fixture").unwrap();
    drop(store);
    assert!(api::invoke(root.path(), "state.get", &json!({"run_id":run.id})).is_ok());
    let error = engine::resume(
        root.path(),
        &run.id,
        None,
        Arc::new(AtomicBool::new(false)),
        &mut |_, _| {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("legacy_run_read_only"));
}

#[test]
fn a_prepared_plan_is_not_evidence_that_its_phase_is_complete() {
    let root = fixture();
    let first = api::invoke(
        root.path(),
        "prepare",
        &json!({"feature":"specs/feature","mode":"plan","view":"full"}),
    )
    .unwrap();
    let planned = answer_plan(root.path(), &first["work"][0]);
    assert_eq!(planned["status"], "plan_ready");
    assert!(planned["completed_phases"].as_array().unwrap().is_empty());
    assert!(planned["phase_hashes"].as_object().unwrap().is_empty());
    let work = prepare(root.path());
    assert_eq!(work["current_phase"], "P001");
    assert!(work["completed_phases"].as_array().unwrap().is_empty());
    assert_eq!(work["work"][0]["kind"], "plan-tasks");
}

#[test]
fn rejected_verification_plan_gets_a_fresh_correctable_planner_packet() {
    let root = fixture();
    let first = prepare(root.path());
    let request = &first["work"][0];
    let input: WorkerInput = serde_json::from_value(
        engine::work_context(
            root.path(),
            request["run_id"].as_str().unwrap(),
            request["request_id"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let snapshot = input.snapshot.unwrap();
    let mut result = receipt(root.path(), request);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        tasks: vec![Task {
            id: "value".into(),
            description: "implement value and tests".into(),
            source_ids: vec![snapshot.tasks[0].id.clone()],
            depends_on: vec![],
            reads: vec![],
            writes: vec!["src/value.txt".into()],
            verification: vec![Check {
                argv: vec!["sa-missing-plan-check-118739".into()],
                cwd: ".".into(),
            }],
        }],
        milestone: input.milestone,
    });
    let rejected = submit(root.path(), request, result);
    assert_eq!(rejected["status"], "needs_input", "{rejected}");
    assert!(
        rejected["blocker"]
            .as_str()
            .unwrap()
            .contains("check_executable_missing")
    );
    assert!(rejected["completed_phases"].as_array().unwrap().is_empty());
    let repo = git::Repository::discover(root.path()).unwrap();
    let store = Store::open(&repo, false).unwrap().unwrap();
    let run = store.get(first["id"].as_str().unwrap()).unwrap();
    assert_eq!(run.attempts[0].status, "failed");
    assert!(run.plans.is_empty());
    let resumed = api::invoke(
        root.path(),
        "prepare",
        &json!({"run_id":first["id"],"view":"full"}),
    )
    .unwrap();
    let retry = &resumed["work"][0];
    assert_eq!(resumed["status"], "awaiting_host");
    assert_ne!(retry["request_id"], request["request_id"]);
    assert_eq!(retry["kind"], "plan-tasks");
    let retry_input = engine::work_context(
        root.path(),
        first["id"].as_str().unwrap(),
        retry["request_id"].as_str().unwrap(),
    )
    .unwrap();
    assert!(
        retry_input["snapshot"]["metadata"]["previous_planning_failure"]["error"]
            .as_str()
            .unwrap()
            .contains("check_executable_missing")
    );
    let corrected = answer_plan(root.path(), retry);
    assert_eq!(corrected["id"], first["id"]);
    assert_eq!(corrected["work"][0]["kind"], "implement", "{corrected}");
    assert_eq!(
        fs::read_to_string(root.path().join("specs/feature/tasks.md"))
            .unwrap()
            .matches("[x]")
            .count(),
        0
    );
}

#[test]
fn pending_verification_revision_preserves_verified_work_and_resumes_same_run() {
    let f = fixture();
    let root = f.path();
    put(
        root,
        "specs/feature/tasks.md",
        "## Implementation\n- [ ] T001 Write value\n- [ ] T002 Verify value\n",
    );
    git::commit(root, "two native tasks").unwrap();
    let initial = prepare(root);
    let request = &initial["work"][0];
    let input: WorkerInput = serde_json::from_value(
        engine::work_context(
            root,
            request["run_id"].as_str().unwrap(),
            request["request_id"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let snapshot = input.snapshot.unwrap();
    let good = Check {
        argv: vec!["node".into(), "--test".into(), "tests/value.mjs".into()],
        cwd: ".".into(),
    };
    let mut result = receipt(root, request);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        milestone: input.milestone,
        tasks: vec![
            Task {
                id: "value".into(),
                description: "write and test value".into(),
                source_ids: vec![snapshot.tasks[0].id.clone()],
                depends_on: vec![],
                reads: vec![],
                writes: vec!["src/value.txt".into()],
                verification: vec![good.clone()],
            },
            Task {
                id: "check-value".into(),
                description: "verify native acceptance".into(),
                source_ids: vec![snapshot.tasks[1].id.clone()],
                depends_on: vec!["value".into()],
                reads: vec!["src/value.txt".into()],
                writes: vec![],
                verification: vec![Check {
                    argv: vec![
                        "node".into(),
                        "-e".into(),
                        "require('node:assert/strict').equal(24,27)".into(),
                    ],
                    cwd: ".".into(),
                }],
            },
        ],
    });
    let work = submit(root, request, result);
    let request = &work["work"][0];
    put(
        Path::new(request["project"].as_str().unwrap()),
        "src/value.txt",
        "42",
    );
    let work = submit(root, request, receipt(root, request));
    let id = work["id"].as_str().unwrap();
    let request = &work["work"][0];
    let blocked = submit(root, request, receipt(root, request));
    assert_eq!(blocked["status"], "needs_input");
    assert_eq!(blocked["completed_tasks"].as_array().unwrap().len(), 1);
    let before = blocked["accepted_head"].clone();
    let mut patch = json!({"run_id":id,"reason":"Native requirement checks value 42; replace the mistaken fixture arithmetic with its executable test.","task_checks":[{"phase_id":"P001","task_id":"check-value","checks":[good]}]});
    let preview = api::invoke(root, "run.revise", &patch).unwrap();
    assert_eq!(
        preview["retained_verified_tasks"].as_array().unwrap().len(),
        1
    );
    patch["apply"] = json!(true);
    patch["plan_hash"] = preview["plan_hash"].clone();
    let revised = api::invoke(root, "run.revise", &patch).unwrap();
    assert_eq!(revised["applied"], true);
    assert_eq!(revised["accepted_head"], before);
    assert_eq!(
        api::invoke(root, "run.revise", &patch).unwrap()["replayed"],
        true
    );
    let resumed = api::invoke(root, "prepare", &json!({"run_id":id,"view":"full"})).unwrap();
    assert_eq!(resumed["id"], id);
    assert_eq!(resumed["completed_tasks"].as_array().unwrap().len(), 1);
    let request = &resumed["work"][0];
    assert_eq!(request["task_id"], "check-value");
    let done = submit(root, request, receipt(root, request));
    assert_eq!(done["completed_tasks"].as_array().unwrap().len(), 2);
    assert!(
        done["attempts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["task_id"] == "check-value" && a["status"] == "failed")
    );
    let bad = api::invoke(root, "prepare", &json!({"run_id":id,"plan":{}})).unwrap_err();
    assert!(bad.to_string().contains("revision_required"));
    let completed=api::invoke(root,"run.revise",&json!({"run_id":id,"reason":"cannot change accepted task","task_checks":[{"phase_id":"P001","task_id":"value","checks":[{"argv":["node","--version"],"cwd":"."}]}]})).unwrap_err();
    assert!(completed.to_string().contains("revision_scope_violation"));
}

#[test]
fn phase_revision_supersedes_only_obsolete_verification_repair() {
    let f = fixture();
    let root = f.path();
    let snapshot = spec_autonomous_core::provider::inspect(
        root,
        spec_autonomous_core::Framework::Speckit,
        "specs/feature",
        &Config::load(root).unwrap(),
    )
    .unwrap();
    let good = Check {
        argv: vec!["node".into(), "--test".into(), "tests/value.mjs".into()],
        cwd: ".".into(),
    };
    let mut milestone =
        engine::source_milestone(spec_autonomous_core::Framework::Speckit, "specs/feature");
    milestone.phases[0].verification = vec![Check {
        argv: vec!["node".into(), "-e".into(), "process.exit(1)".into()],
        cwd: ".".into(),
    }];
    let plan = Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        milestone: Some(milestone),
        tasks: vec![Task {
            id: "value".into(),
            description: "write and verify value".into(),
            source_ids: vec![snapshot.tasks[0].id.clone()],
            depends_on: vec![],
            reads: vec![],
            writes: vec!["src/value.txt".into()],
            verification: vec![good.clone()],
        }],
    };
    let work = api::invoke(root, "prepare", &json!({"plan":plan,"view":"full"})).unwrap();
    let req = &work["work"][0];
    put(
        Path::new(req["project"].as_str().unwrap()),
        "src/value.txt",
        "42",
    );
    let failed = submit(root, req, receipt(root, req));
    let id = failed["id"].as_str().unwrap();
    let req = &failed["work"][0];
    assert_eq!(req["kind"], "converge");
    let mut patch = json!({"run_id":id,"reason":"Phase check command was an invalid exit fixture; use native requirement's actual value test.","phase_checks":[{"phase_id":"P001","checks":[good]}]});
    let busy = api::invoke(root, "run.revise", &patch).unwrap();
    assert_eq!(busy["can_apply"], false);
    patch["apply"] = json!(true);
    patch["plan_hash"] = busy["plan_hash"].clone();
    assert!(
        api::invoke(root, "run.revise", &patch)
            .unwrap_err()
            .to_string()
            .contains("revision_busy")
    );
    engine::revoke(
        root,
        id,
        req["request_id"].as_str().unwrap(),
        req["token"].as_str().unwrap(),
        true,
        "discard obsolete verification repair",
    )
    .unwrap();
    assert!(
        api::invoke(root, "run.revise", &patch)
            .unwrap_err()
            .to_string()
            .contains("revision_conflict")
    );
    patch.as_object_mut().unwrap().remove("apply");
    patch.as_object_mut().unwrap().remove("plan_hash");
    let preview = api::invoke(root, "run.revise", &patch).unwrap();
    patch["apply"] = json!(true);
    patch["plan_hash"] = preview["plan_hash"].clone();
    api::invoke(root, "run.revise", &patch).unwrap();
    let next = api::invoke(root, "prepare", &json!({"run_id":id,"view":"full"})).unwrap();
    assert_eq!(next["completed_tasks"].as_array().unwrap().len(), 1);
    assert_eq!(next["work"][0]["kind"], "audit");
    let repo = git::Repository::discover(root).unwrap();
    let store = Store::open(&repo, false).unwrap().unwrap();
    let saved = store.get(id).unwrap();
    assert!(saved.host.unwrap().pending_repair.is_none());
    let connection = rusqlite::Connection::open(repo.runtime().unwrap().join("state.db")).unwrap();
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(
        version, 3,
        "older v2 writers must not erase revision history"
    );
    let control = Store::for_control(&repo).unwrap();
    control.control(id, "pause").unwrap();
    assert_eq!(control.requested(id).unwrap(), "pause");
}

#[test]
fn readiness_agent_view_is_bounded_without_losing_counts() {
    let value = json!({"verification_readiness":{"ready":true,"status":"deferred","executed":false,"checks":vec![json!({"status":"deferred"});100],"diagnostics":vec![json!({"severity":"warning"});50]}});
    let compact = spec_autonomous_core::capabilities::views::compact(value);
    let report = &compact["verification_readiness"];
    assert_eq!(report["checks"].as_array().unwrap().len(), 8);
    assert_eq!(report["diagnostics"].as_array().unwrap().len(), 16);
    assert_eq!(report["counts"]["checks"], 100);
    assert_eq!(report["counts"]["diagnostics"], 50);
    assert_eq!(report["truncated"], true);
    assert_eq!(report["executed"], false);
}

#[test]
fn native_metadata_refresh_keeps_corrections_until_native_checks_change() {
    let mut m = engine::source_milestone(spec_autonomous_core::Framework::Speckit, "specs/feature");
    let check = |file: &str| Check {
        argv: vec!["node".into(), "--test".into(), file.into()],
        cwd: ".".into(),
    };
    let (old, corrected, native) = (
        check("test/"),
        check("test/value.mjs"),
        check("test/new-contract.mjs"),
    );
    let mut history = vec![
        json!({"diff":[{"scope":"phase","phase_id":"P001","before":[old],"after":[corrected]}]}),
    ];
    m.phases[0].verification = vec![old];
    spec_autonomous_core::run_revision::reconcile_milestone(&mut m, &mut history).unwrap();
    assert_eq!(m.phases[0].verification, vec![corrected]);
    m.phases[0].verification = vec![native.clone()];
    spec_autonomous_core::run_revision::reconcile_milestone(&mut m, &mut history).unwrap();
    assert_eq!(m.phases[0].verification, vec![native]);
    assert_eq!(history[0]["diff"][0]["superseded_by_native"], true);
}
