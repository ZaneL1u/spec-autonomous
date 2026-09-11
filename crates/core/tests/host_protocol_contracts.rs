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
