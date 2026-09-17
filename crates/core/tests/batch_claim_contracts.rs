use serde_json::{Value, json};
use spec_autonomous_core::{
    capabilities as api,
    config::Config,
    engine::{self, ClaimRequest},
    git,
    model::*,
    state::Store,
};
use std::{fs, path::Path};

fn put(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}

fn fixture() -> (tempfile::TempDir, Value) {
    let root = tempfile::tempdir().unwrap();
    for (path, contents) in [
        (
            ".specify/memory/constitution.md",
            "# Constitution\nUse tests.\n",
        ),
        (
            "specs/feature/spec.md",
            "# Specification\nCreate two independent files.\n",
        ),
        (
            "specs/feature/plan.md",
            "# Plan\nUse independent text files.\n",
        ),
        (
            "specs/feature/tasks.md",
            "## Implementation\n- [ ] T001 [P] Create a.txt\n- [ ] T002 [P] Create b.txt\n",
        ),
        (
            ".gitignore",
            ".spec-autonomous/*\n!.spec-autonomous/config.toml\n",
        ),
    ] {
        put(root.path(), path, contents);
    }
    let mut config = Config::default();
    config.execution.max_workers = 2;
    config.host.max_concurrency = 2;
    config.verification = vec![Check {
        argv: vec!["git".into(), "status".into(), "--porcelain".into()],
        cwd: ".".into(),
    }];
    put(
        root.path(),
        ".spec-autonomous/config.toml",
        &toml::to_string(&config).unwrap(),
    );
    git::command(root.path(), &["init", "-q", "-b", "main"], None).unwrap();
    git::commit(root.path(), "fixture").unwrap();
    let mut prepared = api::invoke(
        root.path(),
        "prepare",
        &json!({"feature":"specs/feature", "view":"full"}),
    )
    .unwrap();
    if prepared["work"].as_array().is_some_and(Vec::is_empty) {
        assert!(
            prepared["blocker"]
                .as_str()
                .is_some_and(|blocker| blocker.contains("discussion_required")),
            "{prepared}"
        );
        let preview = api::invoke(
            root.path(),
            "discussion.next",
            &json!({"run_id":prepared["id"],"phase_id":"P001"}),
        )
        .unwrap();
        let applied = api::invoke(
            root.path(),
            "discussion.apply",
            &json!({
                "run_id": prepared["id"],
                "phase_id": "P001",
                "source_hash": preview["source_hash"],
                "selections": [],
                "auto": true
            }),
        )
        .unwrap();
        assert_eq!(applied["unresolved"], json!([]), "{applied}");
        prepared = api::invoke(
            root.path(),
            "prepare",
            &json!({"run_id":prepared["id"], "view":"full"}),
        )
        .unwrap();
    }
    let request = prepared["work"]
        .as_array()
        .and_then(|work| work.first())
        .unwrap_or_else(|| panic!("prepare returned no work request: {prepared}"));
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
    let tasks = snapshot
        .tasks
        .iter()
        .enumerate()
        .map(|(index, source)| Task {
            id: format!("task-{index}"),
            description: source.description.clone(),
            source_ids: vec![source.id.clone()],
            depends_on: vec![],
            reads: vec![],
            writes: vec![format!("{index}.txt")],
            verification: vec![Check {
                argv: vec!["git".into(), "status".into(), "--porcelain".into()],
                cwd: ".".into(),
            }],
        })
        .collect();
    let result = WorkerResult {
        schema_version: 1,
        run_id: input.run_id,
        task_id: input.task_id,
        attempt_id: input.attempt_id,
        status: "candidate".into(),
        summary: "plan independent files".into(),
        blockers: vec![],
        milestone: None,
        plan: Some(Plan {
            schema_version: 1,
            phase_id: "P001".into(),
            source_hash: snapshot.source_hash,
            tasks,
            milestone: input.milestone,
        }),
        audit: vec![],
    };
    let wave = api::invoke(root.path(), "apply-result", &json!({"token":request["token"], "host":{"host_id":"test-host", "session_id":"planner", "fresh_context":true}, "result":result, "view":"full"})).unwrap();
    assert_eq!(wave["work"].as_array().unwrap().len(), 2, "{wave}");
    (root, wave)
}

fn requests(wave: &Value) -> Vec<ClaimRequest> {
    wave["work"]
        .as_array()
        .unwrap()
        .iter()
        .map(|work| ClaimRequest {
            request_id: work["request_id"].as_str().unwrap().into(),
            token: work["token"].as_str().unwrap().into(),
            host: HostIdentity {
                host_id: "test-host".into(),
                session_id: work["request_id"].as_str().unwrap().into(),
                fresh_context: true,
            },
        })
        .collect()
}

fn run(root: &Path, id: &str) -> Value {
    let repo = git::Repository::discover(root).unwrap();
    serde_json::to_value(Store::open(&repo, false).unwrap().unwrap().get(id).unwrap()).unwrap()
}

fn event_count(root: &Path) -> u64 {
    let repo = git::Repository::discover(root).unwrap();
    rusqlite::Connection::open(repo.common.join("spec-autonomous/state.db"))
        .unwrap()
        .query_row("SELECT count(*) FROM events", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn claims_a_wave_in_one_save_and_replays_without_extending_leases() {
    let (root, wave) = fixture();
    let id = wave["id"].as_str().unwrap();
    let before = event_count(root.path());
    let claimed = engine::claim_batch(root.path(), id, requests(&wave)).unwrap();
    assert!(
        claimed["work"]
            .as_array()
            .unwrap()
            .iter()
            .all(|work| work["status"] == "claimed")
    );
    assert_eq!(event_count(root.path()), before + 1);
    let state = run(root.path(), id);
    engine::claim_batch(root.path(), id, requests(&wave)).unwrap();
    assert_eq!(run(root.path(), id), state);
    assert_eq!(event_count(root.path()), before + 1);
}

#[test]
fn invalid_batch_entries_never_partially_claim_work() {
    let (root, wave) = fixture();
    let id = wave["id"].as_str().unwrap();
    let state = run(root.path(), id);
    let before = event_count(root.path());
    let valid = requests(&wave);
    let mut token = valid.clone();
    token[1].token = "incorrect".into();
    let mut session = valid.clone();
    session[1].host = session[0].host.clone();
    let mut fresh = valid.clone();
    fresh[1].host.fresh_context = false;
    for (batch, code) in [
        (vec![], "invalid_arguments"),
        (token, "request_ownership_mismatch"),
        (session, "host_session_reused"),
        (fresh, "fresh_context_required"),
        (
            vec![valid[0].clone(), valid[0].clone()],
            "duplicate_request",
        ),
    ] {
        assert!(
            engine::claim_batch(root.path(), id, batch)
                .unwrap_err()
                .to_string()
                .contains(code)
        );
        assert_eq!(run(root.path(), id), state);
        assert_eq!(event_count(root.path()), before);
    }
}

#[test]
fn batch_checks_prior_ownership_and_cross_run_session_reuse() {
    let (root, wave) = fixture();
    let id = wave["id"].as_str().unwrap();
    let valid = requests(&wave);
    engine::claim(
        root.path(),
        id,
        &valid[0].request_id,
        &valid[0].token,
        valid[0].host.clone(),
    )
    .unwrap();
    let state = run(root.path(), id);
    let mut changed_owner = valid.clone();
    changed_owner[0].host.session_id = "different-owner".into();
    assert!(
        engine::claim_batch(root.path(), id, changed_owner)
            .unwrap_err()
            .to_string()
            .contains("request_ownership_mismatch")
    );
    assert_eq!(run(root.path(), id), state);
    let repo = git::Repository::discover(root.path()).unwrap();
    let mut store = Store::open(&repo, true).unwrap().unwrap();
    let mut other = store.get(id).unwrap();
    other.id = "other-run".into();
    other
        .host
        .as_mut()
        .unwrap()
        .requests
        .values_mut()
        .next()
        .unwrap()
        .owner = Some(valid[1].host.clone());
    store.save(&other, "fixture-other-run").unwrap();
    drop(store);
    assert!(
        engine::claim_batch(root.path(), id, valid)
            .unwrap_err()
            .to_string()
            .contains("host_session_reused")
    );
    assert_eq!(run(root.path(), id), state);
}

#[test]
fn corrupt_packet_and_pending_stop_leave_batch_unclaimed() {
    let (root, wave) = fixture();
    let id = wave["id"].as_str().unwrap();
    let state = run(root.path(), id);
    let path = Path::new(wave["work"][1]["input_path"].as_str().unwrap());
    let original = fs::read(path).unwrap();
    fs::write(path, "{}").unwrap();
    assert!(
        engine::claim_batch(root.path(), id, requests(&wave))
            .unwrap_err()
            .to_string()
            .contains("context_corrupt")
    );
    assert_eq!(run(root.path(), id), state);
    fs::write(path, original).unwrap();
    let repo = git::Repository::discover(root.path()).unwrap();
    Store::for_control(&repo)
        .unwrap()
        .control(id, "pause")
        .unwrap();
    assert!(
        engine::claim_batch(root.path(), id, requests(&wave))
            .unwrap_err()
            .to_string()
            .contains("request_not_claimable")
    );
    assert_eq!(run(root.path(), id), state);
}
