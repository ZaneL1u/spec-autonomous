//! Host protocol ergonomics: auto-claimed work, token recovery, owner revocation,
//! request.get, bounded lock waits and one-step revise/cleanup. Real Git, no agents.
use serde_json::{Value, json};
use spec_autonomous_core::{
    capabilities as api,
    config::Config,
    engine,
    git::{self, Repository},
    model::*,
    state::{Lease, Store},
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
fn fixture(tasks: &str) -> tempfile::TempDir {
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
    put(root.path(), "specs/feature/tasks.md", tasks);
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
    config.verification = vec![value_check()];
    put(
        root.path(),
        ".spec-autonomous/config.toml",
        &toml::to_string(&config).unwrap(),
    );
    git::command(root.path(), &["init", "-q", "-b", "main"], None).unwrap();
    git::commit(root.path(), "fixture").unwrap();
    root
}
fn host() -> Value {
    json!({"host_id":"codex-local","session_id":"main-conversation","fresh_context":true})
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
/// Submit with the identity the coordinator derived for this packet.
fn submit_owned(root: &Path, request: &Value, result: WorkerResult) -> Value {
    api::invoke(
        root,
        "apply-result",
        &json!({"token":request["token"],"host":request["owner"],"result":result,"claim_next":true,"view":"full"}),
    )
    .unwrap()
}
fn plan_for(input: &WorkerInput, tasks: Vec<Task>) -> WorkerResult {
    let snapshot = input.snapshot.clone().unwrap();
    let mut result = receipt(input);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        tasks,
        milestone: input.milestone.clone(),
    });
    result
}
fn task(id: &str, source: &str, writes: &[&str], depends: &[&str], check: Check) -> Task {
    Task {
        id: id.into(),
        description: format!("do {id}"),
        source_ids: vec![source.into()],
        depends_on: depends.iter().map(|d| d.to_string()).collect(),
        reads: vec![],
        writes: writes.iter().map(|w| w.to_string()).collect(),
        verification: vec![check],
    }
}
fn saved(root: &Path, id: &str) -> Run {
    let repo = Repository::discover(root).unwrap();
    Store::open(&repo, false).unwrap().unwrap().get(id).unwrap()
}

#[test]
fn prepare_with_host_claims_issued_work_and_receipts_claim_the_follow_up() {
    let root = fixture("## Implementation\n- [ ] T001 Write value\n");
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full","host":host()}),
    )
    .unwrap();
    let planner = first["work"][0].clone();
    assert_eq!(planner["status"], "claimed", "{first}");
    assert_eq!(first["auto_claimed"], true);
    assert_eq!(planner["owner"]["host_id"], "codex-local");
    let session = planner["owner"]["session_id"].as_str().unwrap();
    assert!(
        session.starts_with("main-conversation-attempt-"),
        "{session}"
    );
    assert!(
        planner["token"].as_str().is_some(),
        "tokens come back with the claim"
    );
    // The receipt uses the derived identity; the next packet is claimed for it too.
    let input = packet(root, &planner);
    let planned = submit_owned(
        root,
        &planner,
        plan_for(
            &input,
            vec![task(
                "value",
                &input.snapshot.as_ref().unwrap().tasks[0].id,
                &["src/value.txt"],
                &[],
                value_check(),
            )],
        ),
    );
    let implement = planned["work"][0].clone();
    assert_eq!(implement["kind"], "implement");
    assert_eq!(implement["status"], "claimed", "{planned}");
    assert_eq!(implement["owner"]["host_id"], "codex-local");
    assert_ne!(
        implement["owner"]["session_id"],
        planner["owner"]["session_id"]
    );
    assert!(implement["token"].as_str().is_some());
    // A different host does not take over owned work.
    let other = api::invoke(root,"prepare",&json!({"run_id":first["id"],"view":"full","host":{"host_id":"other-host","session_id":"s","fresh_context":true}})).unwrap();
    assert_eq!(other["auto_claimed"], false);
    assert_eq!(other["work"][0]["owner"]["host_id"], "codex-local");
    put(
        Path::new(implement["worktree"].as_str().unwrap()),
        "src/value.txt",
        "42",
    );
    let input = packet(root, &implement);
    let audit = submit_owned(root, &implement, receipt(&input));
    assert_eq!(audit["work"][0]["kind"], "audit", "{audit}");
    assert_eq!(audit["work"][0]["status"], "claimed");
    let run = saved(root, first["id"].as_str().unwrap());
    assert_eq!(run.completed_tasks.len(), 1);
    assert!(run.attempts.iter().all(|a| a.status != "issued"));
}

#[test]
fn next_and_request_get_return_tokens_only_to_the_owning_host() {
    let root = fixture("## Implementation\n- [ ] T001 Write value\n");
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full","host":host()}),
    )
    .unwrap();
    let id = first["id"].as_str().unwrap();
    let request = first["work"][0].clone();
    let anonymous = api::invoke(root, "next", &json!({"run_id":id,"view":"full"})).unwrap();
    assert!(anonymous["work"][0].get("token").is_none(), "{anonymous}");
    let recovered = api::invoke(root,"next",&json!({"run_id":id,"view":"full","host":{"host_id":"codex-local","session_id":"any-later-context","fresh_context":true}})).unwrap();
    assert_eq!(
        recovered["work"][0]["token"], request["token"],
        "{recovered}"
    );
    let stranger = api::invoke(root,"next",&json!({"run_id":id,"view":"full","host":{"host_id":"other","session_id":"x","fresh_context":true}})).unwrap();
    assert!(stranger["work"][0].get("token").is_none());
    let detail = api::invoke(root,"request.get",&json!({"run_id":id,"request_id":request["request_id"],"host":{"host_id":"codex-local","session_id":"any","fresh_context":true}})).unwrap();
    assert_eq!(detail["status"], "claimed");
    assert_eq!(detail["kind"], "plan-tasks");
    assert_eq!(detail["token"], request["token"]);
    assert_eq!(detail["pending"], true);
    assert!(detail["heartbeat_age_ms"].as_u64().is_some());
    assert!(Path::new(detail["input_path"].as_str().unwrap()).is_file());
    let public = api::invoke(
        root,
        "request.get",
        &json!({"run_id":id,"request_id":request["request_id"]}),
    )
    .unwrap();
    assert!(public["token"].is_null());
    assert_eq!(public["owner"]["host_id"], "codex-local");
}

#[test]
fn owning_host_can_revoke_without_its_token_and_stale_receipts_are_still_accepted() {
    let root = fixture("## Implementation\n- [ ] T001 Write value\n");
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full","host":host()}),
    )
    .unwrap();
    let id = first["id"].as_str().unwrap();
    let request = first["work"][0].clone();
    // A stranger cannot revoke by identity; the owner can.
    let denied = api::invoke(root,"work.revoke",&json!({"run_id":id,"request_id":request["request_id"],"host":{"host_id":"other","session_id":"x","fresh_context":true},"host_stopped":true,"reason":"not mine"})).unwrap_err();
    assert!(denied.to_string().contains("request_ownership_mismatch"));
    let neither = api::invoke(root,"work.revoke",&json!({"run_id":id,"request_id":request["request_id"],"host_stopped":true,"reason":"no proof"})).unwrap_err();
    assert!(neither.to_string().contains("invalid_arguments"));
    api::invoke(root,"work.revoke",&json!({"run_id":id,"request_id":request["request_id"],"host":{"host_id":"codex-local","session_id":"lost-context","fresh_context":true},"host_stopped":true,"reason":"session gone"})).unwrap();
    let run = saved(root, id);
    assert_eq!(
        run.attempts
            .iter()
            .find(|a| a.id == request["request_id"])
            .unwrap()
            .status,
        "revoked"
    );
    // Reprepare, then let the lease lapse: the owned receipt still lands.
    let again = api::invoke(
        root,
        "prepare",
        &json!({"run_id":id,"view":"full","host":host()}),
    )
    .unwrap();
    let planner = again["work"][0].clone();
    assert_ne!(planner["request_id"], request["request_id"]);
    let repo = Repository::discover(root).unwrap();
    let mut store = Store::open(&repo, true).unwrap().unwrap();
    let mut run = store.get(id).unwrap();
    run.host
        .as_mut()
        .unwrap()
        .requests
        .get_mut(planner["request_id"].as_str().unwrap())
        .unwrap()
        .heartbeat_at_ms = 0;
    store.save(&run, "test-stale").unwrap();
    drop(store);
    let input = packet(root, &planner);
    let planned = submit_owned(
        root,
        &planner,
        plan_for(
            &input,
            vec![task(
                "value",
                &input.snapshot.as_ref().unwrap().tasks[0].id,
                &["src/value.txt"],
                &[],
                value_check(),
            )],
        ),
    );
    assert_eq!(planned["work"][0]["kind"], "implement", "{planned}");
}

#[test]
fn coordinator_lock_waits_briefly_instead_of_failing_concurrent_calls() {
    let root = fixture("## Implementation\n- [ ] T001 Write value\n");
    let root = root.path();
    let repo = Repository::discover(root).unwrap();
    let held = Lease::acquire(&repo).unwrap();
    let holder = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        drop(held);
    });
    let started = std::time::Instant::now();
    let prepared = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full"}),
    )
    .unwrap();
    assert_eq!(prepared["status"], "awaiting_host", "{prepared}");
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(1000),
        "the call waited for the lock instead of failing"
    );
    holder.join().unwrap();
}

#[test]
fn revise_and_cleanup_apply_in_one_step_when_nothing_relevant_is_outstanding() {
    let root = fixture("## Implementation\n- [ ] T001 Write value\n- [ ] T002 Verify value\n");
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full","host":host()}),
    )
    .unwrap();
    let id = first["id"].as_str().unwrap().to_string();
    let planner = first["work"][0].clone();
    let input = packet(root, &planner);
    let ids: Vec<String> = input
        .snapshot
        .as_ref()
        .unwrap()
        .tasks
        .iter()
        .map(|t| t.id.clone())
        .collect();
    let wrong = Check {
        argv: vec![
            "node".into(),
            "-e".into(),
            "require('node:assert/strict').equal(24,27)".into(),
        ],
        cwd: ".".into(),
    };
    let planned = submit_owned(
        root,
        &planner,
        plan_for(
            &input,
            vec![
                task("value", &ids[0], &["src/value.txt"], &[], value_check()),
                task("check-value", &ids[1], &[], &["value"], wrong),
            ],
        ),
    );
    let implement = planned["work"][0].clone();
    assert_eq!(implement["task_id"], "value");
    // One call: no preview, no plan_hash, because check-value has no outstanding work.
    let applied = api::invoke(root,"run.revise",&json!({"run_id":id,"reason":"placeholder arithmetic replaced by the native value test","apply":true,"task_checks":[{"phase_id":"P001","task_id":"check-value","checks":[value_check()]}]})).unwrap();
    assert_eq!(applied["applied"], true, "{applied}");
    // A revision of the outstanding task still needs its work to stop first.
    let busy = api::invoke(root,"run.revise",&json!({"run_id":id,"reason":"touch the running task","apply":true,"task_checks":[{"phase_id":"P001","task_id":"value","checks":[{"argv":["node","--version"],"cwd":"."}]}]})).unwrap_err();
    assert!(busy.to_string().contains("revision_conflict"), "{busy}");
    put(
        Path::new(implement["worktree"].as_str().unwrap()),
        "src/value.txt",
        "42",
    );
    let input = packet(root, &implement);
    let next = submit_owned(root, &implement, receipt(&input));
    let second = next["work"][0].clone();
    assert_eq!(second["task_id"], "check-value", "{next}");
    let input = packet(root, &second);
    assert_eq!(
        input.task.as_ref().unwrap().verification,
        vec![value_check()]
    );
    let audit = submit_owned(root, &second, receipt(&input));
    assert_eq!(audit["work"][0]["kind"], "audit", "{audit}");
    // Closed worker worktrees can be reclaimed in one step; deleting refs still needs review.
    let cleaned = api::invoke(
        root,
        "run.cleanup",
        &json!({"run_id":id,"active":true,"apply":true}),
    )
    .unwrap();
    assert_eq!(cleaned["applied"], true, "{cleaned}");
    assert!(
        !cleaned["removed"].as_array().unwrap().is_empty(),
        "{cleaned}"
    );
    let needs_hash = api::invoke(
        root,
        "run.cleanup",
        &json!({"run_id":id,"active":true,"apply":true,"delete_branches":true}),
    )
    .unwrap_err();
    assert!(
        needs_hash.to_string().contains("source_drift"),
        "{needs_hash}"
    );
}
