//! Split ledger rows, automatic worktree reclamation, run.forensics and host capacity.
//! Real Git and SQLite, no agents.
use serde_json::{Value, json};
use spec_autonomous_core::{
    capabilities as api,
    config::Config,
    engine,
    git::{self, Repository},
    model::*,
    state::{SCHEMA_VERSION, Store},
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
fn fixture(configure: impl FnOnce(&mut Config)) -> tempfile::TempDir {
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
        "dist/\n.spec-autonomous/*\n!.spec-autonomous/config.toml\n!.spec-autonomous/milestones/\n!.spec-autonomous/plans/\n",
    );
    let mut config = Config::default();
    config.runner.profile = "command".into();
    config.runner.command = vec!["must-never-start-this-agent".into()];
    config.runner.fresh_session = true;
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
fn answer_plan(root: &Path, request: &Value) -> Value {
    let input = packet(root, request);
    let snapshot = input.snapshot.clone().unwrap();
    let mut result = receipt(&input);
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
            verification: vec![value_check()],
        }],
        milestone: input.milestone.clone(),
    });
    submit(root, request, result)
}
fn saved(root: &Path, id: &str) -> Run {
    let repo = Repository::discover(root).unwrap();
    Store::open(&repo, false).unwrap().unwrap().get(id).unwrap()
}
fn branch_exists(root: &Path, branch: &str) -> bool {
    git::command(
        root,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
        None,
    )
    .is_ok()
}

#[test]
fn ledger_stores_attempts_and_evidence_as_rows_and_migrates_monolithic_payloads() {
    let temp = tempfile::tempdir().unwrap();
    git::command(temp.path(), &["init", "-q", "-b", "main"], None).unwrap();
    fs::write(temp.path().join("README.md"), "ledger\n").unwrap();
    let head = git::commit(temp.path(), "initial").unwrap();
    let repo = Repository::discover(temp.path()).unwrap();
    let runtime = repo.runtime().unwrap();
    fs::create_dir_all(&runtime).unwrap();
    // A schema-3 ledger as alpha.14 wrote it: one JSON payload per run.
    let legacy: Run = serde_json::from_value(json!({
        "schema_version":1,"id":"run-legacy-monolithic-payload","milestone":{"schema_version":1,"id":"M1","goal":"Legacy","framework":"openspec","revision":1,"phases":[],"verification":[]},
        "mode":"autonomous","range":{},"selected_phases":[],"origin":repo.root,"origin_head":head,"origin_branch":"main","project_relative":".",
        "integration":repo.root,"integration_branch":"main","accepted_head":head,"status":"paused","stage":"executing_phase","current_phase":null,
        "started_at":"2026-10-20T00:00:00Z","updated_at":"2026-10-20T00:00:00Z","elapsed_ms":0,"blocker":null,"config":Config::default(),
        "attempts":[{"id":"attempt-legacy-one","task_id":"t","phase_id":"P1","kind":"implement","worktree":"/tmp/none","branch":"codex/sa/x","base_commit":head,"status":"failed","started_at":"2026-10-20T00:00:00Z","finished_at":null,"summary":"","error":"verification_failed: x","pid":null,"process_identity":null}],
        "evidence":[{"argv":["node","--version"],"cwd":".","revision":head,"exit_code":0,"log":"/tmp/log","log_hash":"h","finished_at":"2026-10-20T00:00:00Z","tree_unchanged":true}],
        "host":{"protocol_version":1,"requests":{}}
    }))
    .unwrap();
    {
        let db = rusqlite::Connection::open(runtime.join("state.db")).unwrap();
        db.execute_batch("CREATE TABLE runs(id TEXT PRIMARY KEY,payload TEXT NOT NULL,updated_at TEXT NOT NULL);CREATE TABLE events(seq INTEGER PRIMARY KEY AUTOINCREMENT,run_id TEXT NOT NULL,kind TEXT NOT NULL,payload TEXT NOT NULL,created_at TEXT NOT NULL);CREATE TABLE controls(run_id TEXT PRIMARY KEY,action TEXT NOT NULL);PRAGMA user_version=3;").unwrap();
        db.execute(
            "INSERT INTO runs(id,payload,updated_at) VALUES(?1,?2,?3)",
            rusqlite::params![
                legacy.id,
                serde_json::to_string(&legacy).unwrap(),
                legacy.updated_at
            ],
        )
        .unwrap();
    }
    let mut store = Store::open(&repo, true).unwrap().unwrap();
    assert_eq!(store.user_version().unwrap(), SCHEMA_VERSION);
    let mut run = store.get(&legacy.id).unwrap();
    assert_eq!(run.attempts.len(), 1, "attempts migrated into rows");
    assert_eq!(run.evidence.len(), 1, "evidence migrated into rows");
    assert_eq!(run.attempts[0].id, "attempt-legacy-one");
    // Appending evidence and touching one attempt writes only those rows.
    run.evidence.push(run.evidence[0].clone());
    run.attempts[0].status = "revoked".into();
    store.save(&run, "test").unwrap();
    let db = rusqlite::Connection::open(runtime.join("state.db")).unwrap();
    let core: String = db
        .query_row("SELECT payload FROM runs WHERE id=?1", [&run.id], |r| {
            r.get(0)
        })
        .unwrap();
    let core: Value = serde_json::from_str(&core).unwrap();
    assert!(core["attempts"].as_array().unwrap().is_empty());
    assert!(core["evidence"].as_array().unwrap().is_empty());
    let attempts: i64 = db
        .query_row(
            "SELECT count(*) FROM run_attempts WHERE run_id=?1",
            [&run.id],
            |r| r.get(0),
        )
        .unwrap();
    let evidence: i64 = db
        .query_row(
            "SELECT count(*) FROM run_evidence WHERE run_id=?1",
            [&run.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((attempts, evidence), (1, 2));
    let reloaded = Store::open(&repo, false)
        .unwrap()
        .unwrap()
        .get(&run.id)
        .unwrap();
    assert_eq!(reloaded.attempts[0].status, "revoked");
    assert_eq!(reloaded.evidence.len(), 2);
    let backups: Vec<_> = fs::read_dir(&runtime)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".backup.db"))
        .collect();
    assert_eq!(backups.len(), 1, "a consistent backup precedes the upgrade");
}

#[test]
fn closed_worktrees_are_reclaimed_after_their_patch_is_stored() {
    let root = fixture(|_| {});
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full","host":host()}),
    )
    .unwrap();
    let id = first["id"].as_str().unwrap().to_string();
    let planner = first["work"][0].clone();
    let planned = answer_plan(root, &planner);
    let implement = planned["work"][0].clone();
    let planner_worktree = planner["worktree"].as_str().unwrap().to_string();
    assert!(
        !Path::new(&planner_worktree).exists(),
        "an accepted planner worktree is reclaimed at once"
    );
    let worktree = Path::new(implement["worktree"].as_str().unwrap());
    put(worktree, "src/value.txt", "42");
    fs::create_dir_all(worktree.join("dist")).unwrap();
    fs::write(worktree.join("dist/bundle.js"), "ignored build output").unwrap();
    let input = packet(root, &implement);
    let audit = submit(root, &implement, receipt(&input));
    assert_eq!(audit["work"][0]["kind"], "audit", "{audit}");
    let run = saved(root, &id);
    let integrated = run
        .attempts
        .iter()
        .find(|a| a.id == implement["request_id"])
        .unwrap();
    assert_eq!(integrated.status, "integrated");
    assert_eq!(integrated.worktree_state.as_deref(), Some("reclaimed"));
    assert!(integrated.patch_hash.is_some());
    assert!(!worktree.exists(), "the integrated worker checkout is gone");
    assert!(
        branch_exists(root, &integrated.branch),
        "its branch ref stays for recovery and audit"
    );
    let repo = Repository::discover(root).unwrap();
    let patch = repo
        .runtime()
        .unwrap()
        .join(format!("runs/{id}/attempts/{}/patch.diff", integrated.id));
    assert!(patch.is_file(), "the worker patch is durable evidence");
    assert!(fs::read_to_string(&patch).unwrap().contains("42"));
    for intent in run.intents.iter().filter(|i| i.state == "accepted") {
        assert!(
            !Path::new(&intent.worktree).exists(),
            "accepted candidate checkouts are reclaimed"
        );
    }
    let pending = run
        .attempts
        .iter()
        .find(|a| a.id == audit["work"][0]["request_id"])
        .unwrap();
    assert!(
        Path::new(&pending.worktree).exists(),
        "live work keeps its checkout"
    );
    assert!(
        git::command(root, &["worktree", "list", "--porcelain"], None)
            .unwrap()
            .matches("worktree ")
            .count()
            <= 3,
        "root, integration and the live audit worktree only"
    );
}

#[test]
fn reclaim_can_be_disabled_per_project() {
    let root = fixture(|config| {
        config.execution.reclaim_worktrees = "never".into();
    });
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full","host":host()}),
    )
    .unwrap();
    let planner = first["work"][0].clone();
    let planned = answer_plan(root, &planner);
    assert_eq!(planned["work"][0]["kind"], "implement");
    assert!(Path::new(planner["worktree"].as_str().unwrap()).exists());
    let mut bad = Config::default();
    bad.execution.reclaim_worktrees = "sometimes".into();
    assert!(
        bad.validate()
            .unwrap_err()
            .to_string()
            .contains("invalid_config")
    );
}

#[test]
fn forensics_reports_repeated_failures_attempt_churn_and_idle_requests() {
    let temp = tempfile::tempdir().unwrap();
    git::command(temp.path(), &["init", "-q", "-b", "main"], None).unwrap();
    fs::write(temp.path().join("README.md"), "forensics\n").unwrap();
    let head = git::commit(temp.path(), "initial").unwrap();
    let repo = Repository::discover(temp.path()).unwrap();
    let attempt = |id: &str, status: &str| json!({"id":id,"task_id":"T1","phase_id":"P1","kind":"implement","worktree":format!("/nonexistent/{id}"),"branch":format!("codex/sa/{id}"),"base_commit":head,"status":status,"started_at":"2026-10-20T00:00:00Z","finished_at":null,"summary":"","error":null,"pid":null,"process_identity":null});
    let failing = |index: u32| json!({"argv":["make","race"],"cwd":"/work","revision":head,"exit_code":2,"log":format!("/tmp/log{index}"),"log_hash":"h","finished_at":"2026-10-20T00:00:00Z","tree_unchanged":true});
    let run: Run = serde_json::from_value(json!({
        "schema_version":1,"id":"run-forensics-fixture-0001","milestone":{"schema_version":1,"id":"M1","goal":"Forensics","framework":"openspec","revision":1,"phases":[],"verification":[]},
        "mode":"autonomous","range":{},"selected_phases":["P1"],"origin":repo.root,"origin_head":head,"origin_branch":"main","project_relative":".",
        "integration":repo.root,"integration_branch":"main","accepted_head":head,"status":"awaiting_host","stage":"executing_phase","current_phase":"P1",
        "started_at":"2026-10-20T00:00:00Z","updated_at":"2026-10-20T00:00:00Z","elapsed_ms":0,"blocker":null,"config":Config::default(),
        "attempts":[attempt("attempt-one","failed"),attempt("attempt-two","failed"),attempt("attempt-three","claimed")],
        "evidence":[failing(1),failing(2)],
        "host":{"protocol_version":1,"requests":{"attempt-three":{"token":"lease-x","input_hash":"i","request_key":"k","issued_at_ms":0,"heartbeat_at_ms":0,"owner":{"host_id":"codex","session_id":"s","fresh_context":true},"receipt_hash":null,"revoked":false}}}
    }))
    .unwrap();
    Store::open(&repo, true)
        .unwrap()
        .unwrap()
        .save(&run, "fixture")
        .unwrap();
    let report = api::invoke(&repo.root, "run.forensics", &json!({"run_id":run.id})).unwrap();
    let codes: Vec<&str> = report["anomalies"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["code"].as_str())
        .collect();
    assert!(codes.contains(&"repeated_failing_check"), "{report}");
    assert!(codes.contains(&"task_attempt_churn"), "{report}");
    assert!(codes.contains(&"idle_outstanding_request"), "{report}");
    assert_eq!(report["metrics"]["failed_checks"], 2);
    assert_eq!(report["metrics"]["attempts"], 3);
    assert_eq!(report["read_only"], true);
}

#[test]
fn prepare_echoes_declared_host_capacity() {
    let root = fixture(|config| {
        config
            .host
            .capabilities
            .insert("isolation".into(), "worktree".into());
        config
            .host
            .capabilities
            .insert("background_dispatch".into(), "true".into());
    });
    let first = api::invoke(
        root.path(),
        "prepare",
        &json!({"feature":"specs/feature","view":"full"}),
    )
    .unwrap();
    assert_eq!(first["host_capacity"]["max_concurrency"], 1, "{first}");
    assert_eq!(first["host_capacity"]["lease_seconds"], 1800);
    assert_eq!(
        first["host_capacity"]["capabilities"]["isolation"],
        "worktree"
    );
    assert_eq!(
        first["host_capacity"]["capabilities"]["background_dispatch"],
        "true"
    );
}
