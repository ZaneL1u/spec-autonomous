//! Verification and audit scope that shrinks with the size of a change: incremental
//! audits, milestone audits reusing phase conclusions, affected-scope verification and
//! verify-only tasks. Real Git, real Node checks and the durable ledger; no agents.
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
/// A check that appends `<stage>:<tag>` to the counter file and then runs `script`.
fn counted(tag: &str, script: &str) -> Check {
    Check {
        argv: vec![
            "node".into(),
            "-e".into(),
            format!(
                "require('fs').appendFileSync(process.env.SA_TEST_COUNTER,process.env.SA_VERIFY_STAGE+':{tag}\\n');{script}"
            ),
        ],
        cwd: ".".into(),
    }
}
const VALUE: &str =
    "require('assert').ok(require('fs').readFileSync('src/value.txt','utf8').startsWith('42'))";
const OTHER: &str = "require('assert').ok(require('fs').existsSync('src/other.txt'))";
/// A Spec Kit feature with two parallel native tasks and two functional requirements.
/// The plan maps them to `value` (src/value.txt) and `other` (src/other.txt).
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
        "# Specification\n- FR-001: src/value.txt contains 42.\n- FR-002: src/other.txt exists.\n",
    );
    put(
        root.path(),
        "specs/feature/plan.md",
        "# Plan\nUse plain UTF-8 files and Node assertions.\n",
    );
    put(
        root.path(),
        "specs/feature/tasks.md",
        "## Implementation\n- [ ] T001 [P] Write 42 in src/value.txt\n- [ ] T002 [P] Write src/other.txt\n",
    );
    put(
        root.path(),
        ".gitignore",
        ".spec-autonomous/*\n!.spec-autonomous/config.toml\n!.spec-autonomous/milestones/\n!.spec-autonomous/plans/\n",
    );
    let mut config = Config::default();
    config.host.max_concurrency = 2;
    config.runner.environment.insert(
        "SA_TEST_COUNTER".into(),
        counter.to_string_lossy().into_owned(),
    );
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
    git::commit(root.path(), "fixture").unwrap();
    root
}
fn counts(counter: &Path) -> Vec<String> {
    fs::read_to_string(counter)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
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
fn task(id: &str, source: &str, write: &str, check: Check) -> Task {
    Task {
        id: id.into(),
        description: format!("write {write}"),
        source_ids: vec![source.into()],
        depends_on: vec![],
        reads: vec![write.into()],
        writes: vec![write.into()],
        verification: vec![check],
    }
}
fn saved(root: &Path, id: &str) -> Run {
    let repo = Repository::discover(root).unwrap();
    Store::open(&repo, false).unwrap().unwrap().get(id).unwrap()
}
fn find<'a>(response: &'a Value, kind: &str, task: Option<&str>) -> &'a Value {
    response["work"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["kind"] == kind && task.is_none_or(|t| w["task_id"] == t))
        .unwrap_or_else(|| panic!("no {kind} {task:?} work: {response}"))
}
fn acceptance_ids(input: &WorkerInput) -> Vec<String> {
    input.snapshot.as_ref().unwrap().metadata["acceptance"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_owned())
        .collect()
}
/// Answer an audit packet; `verdict` maps an acceptance id to (passed, files).
fn answer_audit(
    root: &Path,
    request: &Value,
    verdict: impl Fn(&str) -> (bool, Vec<&'static str>),
) -> Value {
    assert_eq!(request["kind"], "audit", "{request}");
    let input = packet(root, request);
    let mut result = receipt(&input);
    result.audit = acceptance_ids(&input)
        .iter()
        .map(|id| {
            let (passed, files) = verdict(id);
            AuditItem {
                requirement: id.clone(),
                evidence: format!("checked {id}"),
                passed,
                files: files.into_iter().map(String::from).collect(),
                carried_over: None,
            }
        })
        .collect();
    submit(root, request, result)
}
const FR1: &str = "specs/feature/spec.md#FR-001";
const FR2: &str = "specs/feature/spec.md#FR-002";
const CONSTITUTION: &str = ".specify/memory/constitution.md#document";

/// Start a run, plan both tasks, integrate them and return (run id, response carrying
/// the phase audit).
fn through_first_audit(root: &Path) -> (String, Value) {
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
    let mut result = receipt(&input);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        tasks: vec![
            task(
                "value",
                &snapshot.tasks[0].id,
                "src/value.txt",
                counted("value", VALUE),
            ),
            task(
                "other",
                &snapshot.tasks[1].id,
                "src/other.txt",
                counted("other", OTHER),
            ),
        ],
        milestone: input.milestone.clone(),
    });
    let wave = submit(root, planning, result);
    let value = find(&wave, "implement", Some("value")).clone();
    put(
        Path::new(value["project"].as_str().unwrap()),
        "src/value.txt",
        "42",
    );
    submit(root, &value, receipt(&packet(root, &value)));
    let other = find(&wave, "implement", Some("other")).clone();
    put(
        Path::new(other["project"].as_str().unwrap()),
        "src/other.txt",
        "other",
    );
    let audited = submit(root, &other, receipt(&packet(root, &other)));
    (run_id, audited)
}
/// Fail FR-002 in the first audit, append a repair task through converge, plan it to
/// write `repair_write` and integrate it. Returns the response after the repair.
fn repair_round(root: &Path, audit: &Value, repair_write: &str) -> Value {
    let failed = answer_audit(root, audit, |id| match id {
        FR1 => (true, vec!["src/value.txt"]),
        FR2 => (false, vec!["src/other.txt"]),
        _ => (true, vec!["src/value.txt", "src/other.txt"]),
    });
    let converge = find(&failed, "converge", None).clone();
    let tracking = Path::new(converge["project"].as_str().unwrap()).join("specs/feature/tasks.md");
    let before = fs::read_to_string(&tracking).unwrap();
    fs::write(
        &tracking,
        format!("{before}- [ ] T003 Make src/other.txt satisfy FR-002\n"),
    )
    .unwrap();
    let planning = submit(root, &converge, receipt(&packet(root, &converge)));
    let planner = find(&planning, "plan-tasks", None).clone();
    let input = packet(root, &planner);
    let partial = input.snapshot.clone().unwrap();
    let mut result = receipt(&input);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: partial.source_hash.clone(),
        tasks: vec![task(
            "repair",
            &partial.tasks[0].id,
            repair_write,
            counted("repair", OTHER),
        )],
        milestone: input.milestone.clone(),
    });
    let implementing = submit(root, &planner, result);
    let repair = find(&implementing, "implement", Some("repair")).clone();
    put(
        Path::new(repair["project"].as_str().unwrap()),
        repair_write,
        if repair_write == "src/value.txt" {
            "42\n"
        } else {
            "repaired"
        },
    );
    submit(root, &repair, receipt(&packet(root, &repair)))
}

#[test]
fn a_repeated_phase_audit_asks_only_for_items_whose_inputs_changed() {
    let scratch = tempfile::tempdir().unwrap();
    let counter = scratch.path().join("checks.log");
    let root = fixture(&counter, |_| {});
    let root = root.path();
    let (run_id, audited) = through_first_audit(root);
    let audit = find(&audited, "audit", None).clone();
    let first = packet(root, &audit);
    assert_eq!(acceptance_ids(&first), [CONSTITUTION, FR1, FR2]);
    assert!(
        first.snapshot.as_ref().unwrap().metadata["carried_over"].is_null(),
        "a first audit carries nothing"
    );
    let schema: Value =
        serde_json::from_slice(&fs::read(audit["result_schema_path"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(
        schema["$defs"]["audit"]["properties"]["files"]["maxItems"],
        50
    );
    assert!(
        !schema["$defs"]["audit"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("files")),
        "files stays optional"
    );

    // The repair writes src/other.txt: FR-002 failed and the constitution item names
    // that file, so both are asked again; FR-001 rests on src/value.txt only.
    let repaired = repair_round(root, &audit, "src/other.txt");
    let again = find(&repaired, "audit", None).clone();
    let input = packet(root, &again);
    assert_eq!(acceptance_ids(&input), [CONSTITUTION, FR2]);
    let carried = &input.snapshot.as_ref().unwrap().metadata["carried_over"];
    assert_eq!(carried[0]["id"], FR1, "{carried}");
    assert_eq!(carried[0]["from"]["attempt_id"], audit["request_id"]);
    assert_eq!(carried[0]["from"]["scope"], "phase");
    assert!(
        input
            .instruction
            .contains("cover ONLY the 2 ids in snapshot.metadata.acceptance"),
        "{}",
        input.instruction
    );
    // An auditor may not claim a conclusion was carried over.
    let mut forged = receipt(&input);
    forged.audit = acceptance_ids(&input)
        .iter()
        .map(|id| AuditItem {
            requirement: id.clone(),
            evidence: "ok".into(),
            passed: true,
            files: vec![],
            carried_over: Some(CarriedAudit {
                attempt_id: "x".into(),
                phase_id: "P001".into(),
                scope: "phase".into(),
                commit: "c".into(),
                tree: "t".into(),
            }),
        })
        .collect();
    let rejected = api::invoke(
        root,
        "apply-result",
        &json!({"token":again["token"],"host":again["owner"],"result":forged,"claim_next":true,"view":"full"}),
    );
    assert!(
        format!("{rejected:?}").contains("carried_over is set by the coordinator"),
        "{rejected:?}"
    );

    // A fresh packet for the same ids is issued after the rejection; once it passes,
    // the phase conclusion keeps FR-001 with the attempt it was established in.
    let resumed = api::invoke(
        root,
        "prepare",
        &json!({"run_id":run_id,"host":host(),"view":"full"}),
    )
    .unwrap();
    let retry = find(&resumed, "audit", None).clone();
    assert_eq!(acceptance_ids(&packet(root, &retry)), [CONSTITUTION, FR2]);
    answer_audit(root, &retry, |_| (true, vec!["src/other.txt"]));
    let run = saved(root, &run_id);
    assert!(run.completed_phases.contains(&"P001".to_string()));
    let fr1 = run
        .host
        .as_ref()
        .unwrap()
        .audit_cache
        .values()
        .flatten()
        .find(|item| item.requirement == FR1)
        .expect("phase audit cached");
    let from = fr1.carried_over.as_ref().expect("FR-001 carried over");
    assert_eq!(from.attempt_id, audit["request_id"].as_str().unwrap());
    assert!(fr1.evidence.starts_with("carried over from audit attempt"));
}

#[test]
fn an_item_without_files_or_with_changed_files_is_audited_again() {
    let scratch = tempfile::tempdir().unwrap();
    let counter = scratch.path().join("checks.log");
    let root = fixture(&counter, |_| {});
    let root = root.path();
    let (run_id, audited) = through_first_audit(root);
    let audit = find(&audited, "audit", None).clone();
    // The repair writes src/value.txt, which FR-001 rests on.
    let repaired = repair_round(root, &audit, "src/value.txt");
    let again = find(&repaired, "audit", None).clone();
    let input = packet(root, &again);
    assert_eq!(acceptance_ids(&input), [CONSTITUTION, FR1, FR2]);
    assert!(input.snapshot.as_ref().unwrap().metadata["carried_over"].is_null());
    let run = saved(root, &run_id);
    let conclusions = &run.host.as_ref().unwrap().audit_conclusions;
    let fr1 = &conclusions[&format!("P001|phase|{FR1}")];
    assert!(fr1.passed);
    assert_eq!(fr1.files, ["src/value.txt"]);
    assert_eq!(fr1.attempt_id, audit["request_id"].as_str().unwrap());
}

#[test]
fn a_single_phase_milestone_audits_only_its_own_items() {
    let scratch = tempfile::tempdir().unwrap();
    let counter = scratch.path().join("checks.log");
    let root = fixture(&counter, |_| {});
    let root = root.path();
    let (run_id, audited) = through_first_audit(root);
    let audit = find(&audited, "audit", None).clone();
    let milestone = answer_audit(root, &audit, |id| match id {
        FR1 => (true, vec!["src/value.txt"]),
        FR2 => (true, vec!["src/other.txt"]),
        _ => (true, vec!["src/value.txt", "src/other.txt"]),
    });
    let request = find(&milestone, "audit", None).clone();
    assert!(
        request["task_id"]
            .as_str()
            .unwrap()
            .starts_with("audit-milestone-"),
        "{request}"
    );
    let input = packet(root, &request);
    let ids = acceptance_ids(&input);
    assert_eq!(ids.len(), 1, "{ids:?}");
    assert!(ids[0].starts_with("milestone/") && ids[0].ends_with("/goal"));
    let carried = input.snapshot.as_ref().unwrap().metadata["carried_over"]
        .as_array()
        .unwrap()
        .clone();
    let carried_ids: Vec<&str> = carried.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(carried_ids, [CONSTITUTION, FR1, FR2]);
    assert!(
        carried.iter().all(
            |c| c["from"]["scope"] == "phase" && c["from"]["attempt_id"] == audit["request_id"]
        )
    );
    let done = answer_audit(root, &request, |_| (true, vec!["src/value.txt"]));
    assert_eq!(done["status"], "completed", "{done}");
    let run = saved(root, &run_id);
    assert_eq!(
        run.attempts.iter().filter(|a| a.kind == "audit").count(),
        2,
        "one phase audit and one milestone audit for the goal"
    );
}

/// Run a phase whose phase gate falls back to task checks through one repair round and
/// the milestone gate. Returns the run and the executed `<stage>:<task>` checks.
fn repaired_run(configure: impl FnOnce(&mut Config)) -> (tempfile::TempDir, Run, Vec<String>) {
    let scratch = tempfile::tempdir().unwrap();
    let counter = scratch.path().join("checks.log");
    let root = fixture(&counter, |config| {
        config.verification.clear();
        configure(config);
    });
    let path = root.path();
    let (run_id, audited) = through_first_audit(path);
    let audit = find(&audited, "audit", None).clone();
    let repaired = repair_round(path, &audit, "src/other.txt");
    let again = find(&repaired, "audit", None).clone();
    let milestone = answer_audit(path, &again, |_| (true, vec!["src/other.txt"]));
    let request = find(&milestone, "audit", None).clone();
    let done = answer_audit(path, &request, |_| (true, vec!["src/value.txt"]));
    assert_eq!(done["status"], "completed", "{done}");
    let run = saved(path, &run_id);
    let checks = counts(&counter);
    (root, run, checks)
}

#[test]
fn checks_of_tasks_the_change_does_not_reach_stand_on_their_last_pass() {
    let (_root, run, checks) = repaired_run(|_| {});
    assert_eq!(
        checks,
        [
            // `other` is integrated after `value`; value's check stands on its pass.
            "combined:value",
            "combined:other",
            // The first phase gate runs every task check in the integration checkout.
            "phase:value",
            "phase:other",
            // The repair writes src/other.txt: other's check runs again, value's not.
            "combined:repair",
            "combined:other",
            "phase:other",
            "phase:repair",
            // The milestone gate stays full: value's check runs once on the final tree.
            "milestone:value",
        ]
    );
    let scoped: Vec<&Value> = run
        .evidence
        .iter()
        .filter_map(|e| e.details.as_ref())
        .filter(|d| d["reused_from"]["scope"] == "affected")
        .collect();
    assert!(!scoped.is_empty());
    for details in &scoped {
        assert_eq!(
            details["reused_from"]["owners"],
            json!(["value"]),
            "{details}"
        );
        assert_eq!(details["duration_ms"], 0);
        assert!(details["reused_from"]["changed_files"].as_u64().unwrap() > 0);
        assert!(details["reused_from"]["revision"].is_string());
        assert!(Path::new(details["manifest"].as_str().unwrap()).is_file());
    }
    assert!(
        scoped.iter().any(|d| d["stage"] == "phase"),
        "the phase gate after the repair reused value's check"
    );
}

#[test]
fn full_scope_reruns_every_task_check_and_affected_milestone_skips_them() {
    let (_root, _, full) = repaired_run(|c| c.execution.verification_scope = "full".into());
    assert_eq!(
        full,
        [
            "combined:value",
            "combined:other",
            "combined:value",
            "phase:value",
            "phase:other",
            "combined:repair",
            "combined:value",
            "combined:other",
            "phase:value",
            "phase:other",
            "phase:repair",
        ],
        "full scope executes every check; the milestone gate reuses this tree's evidence"
    );
    let (_root, _, affected) =
        repaired_run(|c| c.execution.milestone_verification_scope = "affected".into());
    assert!(
        !affected.iter().any(|c| c.starts_with("milestone:")),
        "{affected:?}"
    );
    let mut bad = Config::default();
    bad.execution.verification_scope = "some".into();
    assert!(
        bad.validate()
            .unwrap_err()
            .to_string()
            .contains("verification_scope")
    );
    let mut bad = Config::default();
    bad.execution.milestone_verification_scope = "some".into();
    assert!(
        bad.validate()
            .unwrap_err()
            .to_string()
            .contains("milestone_verification_scope")
    );
    let text = toml::to_string(&Config::default()).unwrap();
    assert!(
        !text.contains("verification_scope"),
        "defaults stay out of serialized configuration: {text}"
    );
}

#[test]
fn a_check_whose_owner_declared_no_reads_always_runs_again() {
    let scratch = tempfile::tempdir().unwrap();
    let counter = scratch.path().join("checks.log");
    let root = fixture(&counter, |config| config.verification.clear());
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    let planning = &first["work"][0];
    let input = packet(root, planning);
    let snapshot = input.snapshot.clone().unwrap();
    let mut value = task(
        "value",
        &snapshot.tasks[0].id,
        "src/value.txt",
        counted("value", VALUE),
    );
    value.reads.clear();
    let mut result = receipt(&input);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        tasks: vec![
            value,
            task(
                "other",
                &snapshot.tasks[1].id,
                "src/other.txt",
                counted("other", OTHER),
            ),
        ],
        milestone: input.milestone.clone(),
    });
    let wave = submit(root, planning, result);
    let value = find(&wave, "implement", Some("value")).clone();
    put(
        Path::new(value["project"].as_str().unwrap()),
        "src/value.txt",
        "42",
    );
    submit(root, &value, receipt(&packet(root, &value)));
    let other = find(&wave, "implement", Some("other")).clone();
    put(
        Path::new(other["project"].as_str().unwrap()),
        "src/other.txt",
        "other",
    );
    submit(root, &other, receipt(&packet(root, &other)));
    assert_eq!(
        counts(&counter)[..3],
        ["combined:value", "combined:other", "combined:value"],
        "without declared reads, value's check is not proven independent of other's change"
    );
}

#[test]
fn bookkeeping_is_only_plans_and_checkbox_or_append_changes_to_tasks_md() {
    use spec_autonomous_core::scope;
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    git::command(root, &["init", "-q", "-b", "main"], None).unwrap();
    put(
        root,
        "specs/f/tasks.md",
        "## Work\n- [ ] T001 One\n- [ ] T002 Two\n",
    );
    put(root, "specs/f/spec.md", "# Spec\n");
    let base = git::commit(root, "base").unwrap();
    put(
        root,
        "specs/f/tasks.md",
        "## Work\n- [x] T001 One\n- [ ] T002 Two\n- [ ] T003 Repair\n",
    );
    put(root, ".spec-autonomous/plans/M-P001.toml", "x = 1\n");
    let flipped = git::commit(root, "check off and append").unwrap();
    let book = |from: &str, to: &str, path: &str| scope::bookkeeping(root, ".", from, to, path);
    assert!(book(&base, &flipped, "specs/f/tasks.md"));
    assert!(book(&base, &flipped, ".spec-autonomous/plans/M-P001.toml"));
    put(
        root,
        "specs/f/tasks.md",
        "## Work\n- [x] T001 One\n- [ ] T002 Two\n- [ ] T003 Repair the other file\n",
    );
    put(root, "specs/f/spec.md", "# Spec\n- [ ] Not a task list\n");
    let edited = git::commit(root, "edit the last line").unwrap();
    assert!(
        !book(&flipped, &edited, "specs/f/tasks.md"),
        "editing the last task line is not an append"
    );
    assert!(
        book(&base, &edited, "specs/f/tasks.md"),
        "appended in full since base"
    );
    assert!(!book(&base, &edited, "specs/f/spec.md"));
    assert_eq!(
        scope::changed_between(root, ".", &base, &flipped).unwrap(),
        [".spec-autonomous/plans/M-P001.toml", "specs/f/tasks.md"]
    );
    assert!(
        scope::changed_between(root, ".", &flipped, &base).is_none(),
        "not an ancestor"
    );
    for global in [
        "uv.lock",
        "tests/conftest.py",
        ".spec-autonomous/config.toml",
        ".gitmodules",
    ] {
        assert!(scope::global_input(global), "{global}");
    }
    assert!(!scope::global_input(".spec-autonomous/plans/M-P001.toml"));
    assert!(!scope::global_input("src/value.txt"));
}
