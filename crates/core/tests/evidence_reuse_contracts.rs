//! Evidence reuse and reset-free recovery use real Git trees, real Node checks and the
//! durable ledger; nothing here starts an agent.
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
fn feature(root: &Path, dir: &str, task: &str) {
    put(
        root,
        &format!("{dir}/spec.md"),
        "# Specification\n- FR-001: src/value.txt contains 42.\n",
    );
    put(
        root,
        &format!("{dir}/plan.md"),
        "# Plan\nUse a plain UTF-8 file and Node assertions.\n",
    );
    put(
        root,
        &format!("{dir}/tasks.md"),
        &format!("## Implementation\n- {task} T001 Write 42 in src/value.txt\n"),
    );
}
fn value_check() -> Check {
    Check {
        argv: vec!["node".into(), "--test".into(), "tests/value.mjs".into()],
        cwd: ".".into(),
    }
}
fn counter_check() -> Check {
    Check {
        argv: vec![
            "node".into(),
            "-e".into(),
            "require('fs').appendFileSync(process.env.SA_TEST_COUNTER,process.env.SA_VERIFY_STAGE+'\\n')".into(),
        ],
        cwd: ".".into(),
    }
}
/// A Spec Kit repository whose verification appends the executing stage to `counter`.
fn fixture(counter: &Path, reuse: &str, value_done: bool) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    put(
        root.path(),
        ".specify/memory/constitution.md",
        "# Constitution\nUse executable assertions.\n",
    );
    feature(
        root.path(),
        "specs/feature",
        if value_done { "[x]" } else { "[ ]" },
    );
    if value_done {
        put(root.path(), "src/value.txt", "42");
    }
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
    config.runner.environment.insert(
        "SA_TEST_COUNTER".into(),
        counter.to_string_lossy().into_owned(),
    );
    config.execution.verification_reuse = reuse.into();
    config.verification = vec![counter_check(), value_check()];
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
fn input(root: &Path, request: &Value) -> WorkerInput {
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
fn answer_plan(root: &Path, request: &Value) -> Value {
    let input = input(root, request);
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
        milestone: input.milestone,
    });
    submit(root, request, result)
}
fn answer_audit(root: &Path, request: &Value) -> Value {
    assert_eq!(request["kind"], "audit", "{request}");
    let input = input(root, request);
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
fn by_stage<'a>(run: &'a Run, stage: &str) -> Vec<&'a Evidence> {
    run.evidence
        .iter()
        .filter(|e| e.details.as_ref().is_some_and(|d| d["stage"] == stage))
        .collect()
}
fn reused_from(e: &Evidence) -> Option<&str> {
    e.details.as_ref()?["reused_from"]["stage"].as_str()
}

#[test]
fn identical_trees_reuse_evidence_across_stages_and_skip_the_empty_post_hook_wave() {
    let scratch = tempfile::tempdir().unwrap();
    let counter = scratch.path().join("stages.log");
    let root = fixture(&counter, "same-tree", false);
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full"}),
    )
    .unwrap();
    let planned = answer_plan(root, &first["work"][0]);
    let request = &planned["work"][0];
    assert_eq!(request["kind"], "implement");
    put(
        Path::new(request["worktree"].as_str().unwrap()),
        "src/value.txt",
        "42",
    );
    let input = input(root, request);
    let audit = submit(root, request, receipt(&input));
    let id = audit["id"].as_str().unwrap();
    assert_eq!(audit["status"], "awaiting_host", "{audit}");
    assert_eq!(audit["work"][0]["kind"], "audit");
    let run = saved(root, id);
    assert_eq!(run.completed_tasks.len(), 1);
    // Worker executes; the candidate tree is identical, so combined reuses it.
    let worker = by_stage(&run, "worker");
    assert_eq!(worker.len(), 2);
    assert!(worker.iter().all(|e| reused_from(e).is_none()));
    let combined = by_stage(&run, "combined");
    assert_eq!(combined.len(), 2);
    assert!(combined.iter().all(|e| reused_from(e) == Some("worker")));
    assert!(
        combined
            .iter()
            .all(|e| e.exit_code == 0 && e.tree_unchanged)
    );
    // The integration checkout is another class: the phase gate executes once.
    let phase = by_stage(&run, "phase");
    assert_eq!(phase.len(), 2);
    assert!(phase.iter().all(|e| reused_from(e).is_none()));
    assert_eq!(stages(&counter), ["worker", "phase"]);

    let milestone = answer_audit(root, &audit["work"][0]);
    assert_eq!(milestone["work"][0]["kind"], "audit", "{milestone}");
    let run = saved(root, id);
    assert!(by_stage(&run, "post-hook").is_empty(), "no hook moved HEAD");
    let gate = by_stage(&run, "milestone");
    assert_eq!(gate.len(), 2);
    assert!(gate.iter().all(|e| reused_from(e) == Some("phase")));
    assert_eq!(stages(&counter), ["worker", "phase"]);
    for e in &run.evidence {
        let d = e.details.as_ref().unwrap();
        if d.get("reused_from").is_some() {
            assert_eq!(d["duration_ms"], 0);
            assert!(Path::new(d["manifest"].as_str().unwrap()).is_file());
            assert!(d["reused_from"]["wave_id"].as_str().is_some());
        }
    }
    let done = answer_audit(root, &milestone["work"][0]);
    assert_eq!(done["status"], "completed", "{done}");
    let progress = api::invoke(root, "progress", &json!({"view":"full"})).unwrap();
    let overhead = &progress["runs"][0]["overhead"];
    // Every host round trip re-enters the phase and milestone gates; with identical
    // trees those re-entries are all reuses, so only four checks ever executed.
    assert_eq!(overhead["executed_checks"], 4, "{overhead}");
    assert_eq!(overhead["reused_checks"], 8);
    assert_eq!(overhead["verification_checks"], 12);
    assert_eq!(stages(&counter), ["worker", "phase"]);
    assert_eq!(overhead["duration_coverage"], "complete");
}

#[test]
fn verification_reuse_none_executes_every_wave() {
    let scratch = tempfile::tempdir().unwrap();
    let counter = scratch.path().join("stages.log");
    let root = fixture(&counter, "none", false);
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","view":"full"}),
    )
    .unwrap();
    let planned = answer_plan(root, &first["work"][0]);
    let request = &planned["work"][0];
    put(
        Path::new(request["worktree"].as_str().unwrap()),
        "src/value.txt",
        "42",
    );
    let input = input(root, request);
    let audit = submit(root, request, receipt(&input));
    let run = saved(root, audit["id"].as_str().unwrap());
    assert!(run.evidence.iter().all(|e| reused_from(e).is_none()));
    assert_eq!(stages(&counter), ["worker", "combined", "phase"]);
    assert!(
        Config::load(root).is_ok(),
        "none is a valid verification_reuse value"
    );
    let mut bad = Config::default();
    bad.execution.verification_reuse = "sometimes".into();
    assert!(
        bad.validate()
            .unwrap_err()
            .to_string()
            .contains("invalid_config")
    );
}

/// A durable run whose phases already completed, as left behind by an earlier coordinator.
const RUN: &str = "run-stale-phases-fixture";
struct Resumable {
    _temp: tempfile::TempDir,
    root: std::path::PathBuf,
    counter: std::path::PathBuf,
    repo: Repository,
    run: Run,
}
impl Resumable {
    fn new(phases: &[(&str, &str, &[&str])]) -> Self {
        let scratch = tempfile::tempdir().unwrap();
        let counter = scratch.path().join("stages.log");
        let temp = fixture(&counter, "same-tree", true);
        let root = temp.path().to_path_buf();
        for (_, dir, _) in phases {
            if *dir != "specs/feature" {
                feature(&root, dir, "[x]");
            }
        }
        git::commit(&root, "all native tasks done").unwrap();
        let head = git::head(&root).unwrap();
        let repo = Repository::discover(&root).unwrap();
        let (integration, branch) = repo.add_worktree(RUN, &head).unwrap();
        let phase_values: Vec<Value> = phases
            .iter()
            .map(|(id, dir, deps)| {
                json!({"id":id,"label":id,"title":dir,"depends_on":deps,"source":{"kind":"speckit-feature","selector":dir},"verification":[]})
            })
            .collect();
        let ids: Vec<&str> = phases.iter().map(|(id, _, _)| *id).collect();
        let run: Run = serde_json::from_value(json!({
            "schema_version":1,"id":RUN,
            "milestone":{"schema_version":1,"id":"M1","goal":"Keep progress","framework":"speckit","revision":1,"phases":phase_values,"verification":[]},
            "mode":"autonomous","range":{},"selected_phases":ids,"origin":repo.root,"origin_head":head,"origin_branch":"main","project_relative":".",
            "integration":integration,"integration_branch":branch,"accepted_head":head,"status":"paused","stage":"advancing","current_phase":null,
            "started_at":spec_autonomous_core::paths::now(),"updated_at":spec_autonomous_core::paths::now(),"elapsed_ms":0,"blocker":null,
            "config":Config::load(&root).unwrap(),"completed_phases":ids,
            "host":{"protocol_version":1,"requests":{}}
        }))
        .unwrap();
        let _ = scratch.keep();
        Self {
            _temp: temp,
            root,
            counter,
            repo,
            run,
        }
    }
    fn save(&self) {
        Store::open(&self.repo, true)
            .unwrap()
            .unwrap()
            .save(&self.run, "fixture")
            .unwrap();
    }
    fn resume(&self) -> Value {
        api::invoke(&self.root, "prepare", &json!({"run_id":RUN,"view":"full"})).unwrap()
    }
    fn saved(&self) -> Run {
        saved(&self.root, RUN)
    }
}

#[test]
fn foreign_origin_commit_keeps_completed_phases_and_reruns_only_their_gates() {
    let f = Resumable::new(&[("P001", "specs/feature", &[])]);
    f.save();
    put(&f.root, "README.md", "unrelated documentation\n");
    git::commit(&f.root, "docs: foreign commit").unwrap();
    let resumed = f.resume();
    assert_eq!(resumed["status"], "awaiting_host", "{resumed}");
    assert_eq!(
        resumed["work"][0]["kind"], "audit",
        "straight to milestone audit"
    );
    let run = f.saved();
    assert_eq!(run.completed_phases, ["P001"]);
    assert!(
        run.stale_phases.is_empty(),
        "stale gate ran before delivery"
    );
    assert!(
        run.intents
            .iter()
            .any(|i| i.task_id == "native-baseline" && i.state == "accepted"),
        "foreign history was merged into integration"
    );
    assert!(run.attempts.iter().all(|a| a.kind != "plan-tasks"));
    let phase = by_stage(&run, "phase");
    assert_eq!(phase.len(), 2);
    assert!(phase.iter().all(|e| reused_from(e).is_none()));
    let milestone = by_stage(&run, "milestone");
    assert_eq!(milestone.len(), 2);
    assert!(milestone.iter().all(|e| reused_from(e) == Some("phase")));
    assert_eq!(stages(&f.counter), ["phase"]);
}

#[test]
fn fast_forwarded_origin_is_recognized_without_merge_or_invalidation() {
    let mut f = Resumable::new(&[("P001", "specs/feature", &[])]);
    put(
        Path::new(&f.run.integration),
        ".spec-autonomous/plans/M1-P001.toml",
        "schema_version = 1\n",
    );
    let accepted = git::commit(Path::new(&f.run.integration), "sa: record execution plan").unwrap();
    f.run.accepted_head = accepted.clone();
    f.save();
    git::command(&f.root, &["merge", "--ff-only", &accepted], None).unwrap();
    assert_eq!(git::head(&f.root).unwrap(), accepted);
    let resumed = f.resume();
    assert_eq!(resumed["status"], "awaiting_host", "{resumed}");
    assert_eq!(resumed["work"][0]["kind"], "audit");
    let run = f.saved();
    assert_eq!(run.origin_head, accepted);
    assert_eq!(run.accepted_head, accepted);
    assert_eq!(run.completed_phases, ["P001"]);
    assert!(run.stale_phases.is_empty());
    assert!(run.intents.is_empty(), "nothing foreign to reconcile");
    assert!(by_stage(&run, "phase").is_empty(), "no stale gate");
    assert_eq!(by_stage(&run, "milestone").len(), 2);
    assert_eq!(stages(&f.counter), ["milestone"]);
}

#[test]
fn native_source_change_reopens_only_its_phase_and_marks_dependents_stale() {
    let f = Resumable::new(&[
        ("P001", "specs/feature", &[]),
        ("P002", "specs/feature2", &["P001"]),
    ]);
    f.save();
    put(
        &f.root,
        "specs/feature/tasks.md",
        "## Implementation\n- [x] T001 Write 42 in src/value.txt\n- [ ] T002 Record the value in docs/value.md\n",
    );
    git::commit(&f.root, "spec: add a native task to the first phase").unwrap();
    let resumed = f.resume();
    assert_eq!(resumed["status"], "awaiting_host", "{resumed}");
    assert_eq!(resumed["work"][0]["kind"], "plan-tasks", "{resumed}");
    assert_eq!(resumed["work"][0]["phase_id"], "P001");
    let run = f.saved();
    assert_eq!(
        run.completed_phases,
        ["P002"],
        "dependent keeps its accepted work"
    );
    assert_eq!(
        run.stale_phases,
        ["P002"],
        "dependent only owes a fresh gate"
    );
    assert!(
        stages(&f.counter).is_empty(),
        "nothing re-verified before P001 lands"
    );
}
