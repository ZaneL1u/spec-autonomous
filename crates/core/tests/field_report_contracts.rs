//! 0.2.0 field report from a real OpenSpec run: delivery past unrelated untracked files,
//! prepared read-only checkouts, usable audit execution evidence and OpenSpec planning
//! guidance. Real Git and SQLite, no agents.
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
        env: Default::default(),
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
        env: Default::default(),
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
        deferred_evidence: vec![],
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
        verify_only: false,
        verification: vec![if id == "value" {
            value_check()
        } else {
            Check {
                argv: vec!["node".into(), "--version".into()],
                cwd: ".".into(),
                env: Default::default(),
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
/// Integrate both planned tasks and return the response that carries the phase audit.
fn finish_tasks(root: &Path, wave: &Value) -> Value {
    let request = work(wave, "value").clone();
    write_value(&request);
    submit(root, &request, receipt(&packet(root, &request)));
    let other = work(wave, "other").clone();
    put(
        Path::new(other["project"].as_str().unwrap()),
        "src/other.txt",
        "other",
    );
    submit(root, &other, receipt(&packet(root, &other)))
}
fn find_kind<'a>(response: &'a Value, kind: &str) -> Option<&'a Value> {
    response["work"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["kind"] == kind)
}
fn acceptance_ids(input: &WorkerInput) -> Vec<String> {
    input.snapshot.as_ref().unwrap().metadata["acceptance"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_owned())
        .collect()
}
/// Pass every audit until no audit work remains; returns the last response.
fn pass_audits(root: &Path, mut response: Value) -> Value {
    let mut rounds = 0;
    while let Some(audit) = find_kind(&response, "audit").cloned() {
        rounds += 1;
        assert!(rounds < 5, "{response}");
        let input = packet(root, &audit);
        let mut result = receipt(&input);
        result.audit = acceptance_ids(&input)
            .into_iter()
            .map(|id| AuditItem {
                requirement: id,
                evidence: "src/value.txt contains 42; node --test tests/value.mjs".into(),
                passed: true,
                files: vec![],
                carried_over: None,
            })
            .collect();
        response = submit(root, &audit, result);
    }
    response
}
fn resume(root: &Path, run_id: &str) -> Value {
    api::invoke(
        root,
        "prepare",
        &json!({"run_id":run_id,"host":host(),"view":"full"}),
    )
    .unwrap()
}

// ---------------------------------------------------------------------------------
// 1. ff-original delivery and untracked files in the user's checkout

fn origin_head(root: &Path) -> String {
    git::head(root).unwrap()
}

#[test]
fn delivery_passes_untracked_files_outside_the_delivered_diff() {
    let root = fixture(|_| {});
    let root = root.path();
    let (run_id, wave) = start(root);
    let before = origin_head(root);
    // The user keeps research notes next to the run, including inside a delivered directory.
    put(root, ".planning/research/notes.md", "field notes\n");
    put(root, "src/scratch.md", "not part of the change\n");
    let done = pass_audits(root, finish_tasks(root, &wave));
    assert_eq!(done["status"], "completed", "{done}");
    let run = saved(root, &run_id);
    assert_ne!(run.accepted_head, before);
    assert_eq!(origin_head(root), run.accepted_head, "{done}");
    assert_eq!(
        fs::read_to_string(root.join("src/value.txt")).unwrap(),
        "42"
    );
    assert_eq!(
        fs::read_to_string(root.join(".planning/research/notes.md")).unwrap(),
        "field notes\n"
    );
    assert!(root.join("src/scratch.md").is_file());
}

#[test]
fn delivery_names_untracked_collisions_and_tracked_changes_that_block_it() {
    let root = fixture(|_| {});
    let root = root.path();
    let (run_id, wave) = start(root);
    let before = origin_head(root);
    put(root, ".planning/research/notes.md", "field notes\n");
    // Same path as a delivered file, and a file where the delivered file needs to be.
    put(root, "src/value.txt", "draft\n");
    put(root, "src/other.txt/notes.md", "draft\n");
    let pending = pass_audits(root, finish_tasks(root, &wave));
    assert_eq!(pending["status"], "delivery_pending", "{pending}");
    let blocker = pending["blocker"].as_str().unwrap_or_default().to_owned();
    assert!(
        blocker.starts_with(
            "delivery_pending: original checkout changed; verified branch retained; blocking: "
        ),
        "{blocker}"
    );
    let blocking: Value =
        serde_json::from_str(blocker.split_once("; blocking: ").unwrap().1).unwrap();
    let mut untracked: Vec<String> =
        serde_json::from_value(blocking["untracked"]["paths"].clone()).unwrap();
    untracked.sort();
    assert_eq!(
        untracked,
        ["src/other.txt/notes.md", "src/value.txt"],
        "{blocker}"
    );
    assert_eq!(blocking["ignored_untracked"], 1, "{blocker}");
    assert_eq!(blocking["modified"]["count"], 0, "{blocker}");
    // Nothing in the user's checkout moved or was overwritten.
    assert_eq!(origin_head(root), before);
    assert_eq!(
        fs::read_to_string(root.join("src/value.txt")).unwrap(),
        "draft\n"
    );

    // A tracked edit still blocks, even with the collisions gone.
    fs::remove_file(root.join("src/value.txt")).unwrap();
    fs::remove_dir_all(root.join("src/other.txt")).unwrap();
    put(root, "tests/value.mjs", "// local edit\n");
    let pending = resume(root, &run_id);
    assert_eq!(pending["status"], "delivery_pending", "{pending}");
    let blocker = pending["blocker"].as_str().unwrap_or_default();
    let blocking: Value =
        serde_json::from_str(blocker.split_once("; blocking: ").unwrap().1).unwrap();
    assert_eq!(
        blocking["modified"]["paths"],
        json!(["tests/value.mjs"]),
        "{blocker}"
    );
    assert_eq!(blocking["untracked"]["count"], 0, "{blocker}");
    assert_eq!(origin_head(root), before);

    // The operator reverts the edit and fast-forwards by hand; the notes stay untracked.
    // Resuming recognizes the landed delivery instead of reporting a moved HEAD.
    git::command(root, &["checkout", "--", "tests/value.mjs"], None).unwrap();
    let accepted = saved(root, &run_id).accepted_head;
    git::command(root, &["merge", "-q", "--ff-only", &accepted], None).unwrap();
    let done = resume(root, &run_id);
    assert_eq!(done["status"], "completed", "{done}");
    assert_eq!(origin_head(root), saved(root, &run_id).accepted_head);
    assert!(root.join(".planning/research/notes.md").is_file());
}

#[test]
fn fast_forward_blockers_compare_untracked_files_with_the_target_diff() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    put(root, "README.md", "base\n");
    git::command(root, &["init", "-q", "-b", "main"], None).unwrap();
    let base = git::commit(root, "base").unwrap();
    put(root, "docs/guide/intro.md", "intro\n");
    put(root, "README.md", "changed\n");
    let target = git::commit(root, "target").unwrap();
    git::command(root, &["reset", "-q", "--hard", &base], None).unwrap();
    git::command(root, &["clean", "-qfdx"], None).unwrap();
    assert_eq!(git::fast_forward_blockers(root, &target).unwrap(), None);

    put(root, "notes/todo.md", "unrelated\n");
    put(root, "docs/other.md", "sibling of a delivered directory\n");
    assert_eq!(git::fast_forward_blockers(root, &target).unwrap(), None);

    // A file where the target needs a directory, and a file under a path the target adds.
    put(root, "docs/guide", "file in the way\n");
    git::command(root, &["checkout", "-q", &target, "--", "README.md"], None).unwrap();
    git::command(root, &["reset", "-q", "--", "README.md"], None).unwrap();
    let blocking = git::fast_forward_blockers(root, &target).unwrap().unwrap();
    let mut paths: Vec<String> =
        serde_json::from_value(blocking["untracked"]["paths"].clone()).unwrap();
    paths.sort();
    assert_eq!(paths, ["docs/guide"], "{blocking}");
    assert_eq!(blocking["ignored_untracked"], 2, "{blocking}");
    // Tracked edits always block, related to the target or not.
    assert_eq!(
        blocking["modified"]["paths"],
        json!(["README.md"]),
        "{blocking}"
    );
    assert!(git::advance_delivery(root, &target).is_err());
    assert_eq!(git::head(root).unwrap(), base);

    fs::remove_file(root.join("docs/guide")).unwrap();
    git::command(root, &["checkout", "-q", "--", "README.md"], None).unwrap();
    git::advance_delivery(root, &target).unwrap();
    assert_eq!(git::head(root).unwrap(), target);
    assert!(root.join("notes/todo.md").is_file() && root.join("docs/other.md").is_file());
}
