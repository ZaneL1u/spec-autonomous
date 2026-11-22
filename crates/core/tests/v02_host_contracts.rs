//! 0.2.0 host contracts: check environments, glob write sets, deferred real-environment
//! evidence, configurable worktree roots, open-ended acceptance and MCP launch
//! commands. Real Git and SQLite, no agents.
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
                env: Default::default(),
            }
        }],
    }
}
#[allow(dead_code)]
fn answer_plan(root: &Path, request: &Value) -> Value {
    answer_plan_with(root, request, |_| {})
}
fn answer_plan_with(root: &Path, request: &Value, edit: impl FnOnce(&mut Vec<Task>)) -> Value {
    let input = packet(root, request);
    let snapshot = input.snapshot.clone().unwrap();
    let mut result = receipt(&input);
    let mut tasks = vec![
        task("value", &snapshot.tasks[0].id, "src/value.txt"),
        task("other", &snapshot.tasks[1].id, "src/other.txt"),
    ];
    edit(&mut tasks);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        tasks,
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
#[allow(dead_code)]
fn start(root: &Path) -> (String, Value) {
    start_with(root, |_| {})
}
fn start_with(root: &Path, edit: impl FnOnce(&mut Vec<Task>)) -> (String, Value) {
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
    let wave = answer_plan_with(root, &first["work"][0], edit);
    (run_id, wave)
}
fn write_value(request: &Value) {
    let project = Path::new(request["project"].as_str().unwrap());
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/value.txt"), "42").unwrap();
}

// ---------------------------------------------------------------------------------
// #40 check-level environment variables

fn env_check(pairs: &[(&str, &str)]) -> Check {
    Check {
        argv: vec!["node".into(), "--version".into()],
        cwd: ".".into(),
        env: pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

#[test]
fn check_env_names_are_validated_and_secrets_refused() {
    use spec_autonomous_core::config::validate_check;
    assert!(validate_check(&env_check(&[("PYTHONDONTWRITEBYTECODE", "1")])).is_ok());
    assert!(validate_check(&env_check(&[("_x9", "")])).is_ok());
    for (name, code) in [
        ("PATH", "is reserved"),
        ("path", "is reserved"),
        ("HOME", "is reserved"),
        ("SA_VERIFY_STAGE", "is reserved"),
        ("GIT_DIR", "is reserved"),
        ("DYLD_INSERT_LIBRARIES", "is reserved"),
        ("GITHUB_TOKEN", "looks like a credential"),
        ("DB_PASSWORD", "looks like a credential"),
        ("OPENAI_API_KEY", "looks like a credential"),
        ("1ABC", "not a valid variable name"),
        ("A-B", "not a valid variable name"),
        ("", "not a valid variable name"),
    ] {
        let error = validate_check(&env_check(&[(name, "1")]))
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with("invalid_command: check_env"),
            "{name}: {error}"
        );
        assert!(error.contains(code), "{name}: {error}");
    }
    assert!(
        validate_check(&env_check(&[("BIG", &"x".repeat(2000))]))
            .unwrap_err()
            .to_string()
            .contains("at most 1024 bytes")
    );
    let many: Vec<(String, String)> = (0..33).map(|i| (format!("V{i}"), "1".into())).collect();
    let many: Vec<(&str, &str)> = many.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    assert!(validate_check(&env_check(&many)).is_err());
    // Without env a check keeps its bytes, so old plans and identities do not change.
    assert_eq!(
        serde_json::to_value(value_check()).unwrap(),
        json!({"argv":["node","--test","tests/value.mjs"],"cwd":"."})
    );
    let parsed: Check = serde_json::from_value(json!({"argv":["x"],"env":{"A":"1"}})).unwrap();
    assert_eq!(parsed.env["A"], "1");
    assert_eq!(parsed.cwd, ".");
    let config: Config = toml::from_str(
        "verification = [{ argv = [\"pytest\"], env = { PYTHONDONTWRITEBYTECODE = \"1\" } }]\n",
    )
    .unwrap();
    config.validate().unwrap();
    assert_eq!(config.verification[0].env["PYTHONDONTWRITEBYTECODE"], "1");
}

#[test]
fn a_task_check_runs_with_its_env_and_logs_only_the_names() {
    let root = fixture(|_| {});
    let root = root.path();
    let probe = Check {
        argv: vec![
            "node".into(),
            "-e".into(),
            "require('node:assert/strict').equal(process.env.CHECK_PROBE,'on'+'-'+'42')".into(),
        ],
        cwd: ".".into(),
        env: [("CHECK_PROBE".to_owned(), "on-42".to_owned())].into(),
    };
    let (run_id, wave) = start_with(root, |tasks| tasks[0].verification.push(probe.clone()));
    let request = work(&wave, "value").clone();
    let input = packet(root, &request);
    let planned = &input.task.as_ref().unwrap().verification;
    assert_eq!(
        planned[1].env["CHECK_PROBE"], "on-42",
        "packet keeps the env"
    );
    let prompt = fs::read_to_string(request["prompt_path"].as_str().unwrap()).unwrap();
    assert!(prompt.contains("env CHECK_PROBE=on-42"), "{prompt}");
    let schema: Value =
        serde_json::from_slice(&fs::read(request["result_schema_path"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(
        schema["$defs"]["check"]["properties"]["env"]["additionalProperties"]["type"],
        "string"
    );
    assert!(
        !schema["$defs"]["check"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("env"))
    );
    write_value(&request);
    submit(root, &request, receipt(&input));
    let run = saved(root, &run_id);
    assert!(
        run.completed_tasks.contains(&Run::key("P001", "value")),
        "{:?}",
        run.attempts
            .iter()
            .map(|a| (&a.task_id, &a.status, &a.error))
            .collect::<Vec<_>>()
    );
    let evidence = run
        .evidence
        .iter()
        .find(|e| e.argv == probe.argv)
        .expect("probe evidence");
    assert_eq!(evidence.exit_code, 0);
    let details = evidence.details.as_ref().unwrap();
    assert_eq!(details["env_keys"], json!(["CHECK_PROBE"]));
    let manifest = fs::read_to_string(details["manifest"].as_str().unwrap()).unwrap();
    assert!(
        !manifest.contains("on-42"),
        "values never reach the manifest"
    );
}

#[test]
fn a_plan_with_a_reserved_check_env_is_rejected() {
    let root = fixture(|_| {});
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    let response = answer_plan_with(root, &first["work"][0], |tasks| {
        tasks[0].verification[0].env = [("PATH".to_owned(), "/tmp".to_owned())].into();
    });
    let text = response.to_string();
    assert!(text.contains("check_env"), "{text}");
    assert!(
        response["work"]
            .as_array()
            .unwrap()
            .iter()
            .all(|w| w["kind"] != "implement"),
        "{response}"
    );
}

#[test]
fn planning_instruction_documents_check_env() {
    let root = fixture(|_| {});
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    let input = packet(root, &first["work"][0]);
    assert!(
        input
            .instruction
            .contains(spec_autonomous_core::verification_preflight::planning_guidance_addendum()),
        "{}",
        input.instruction
    );
}
