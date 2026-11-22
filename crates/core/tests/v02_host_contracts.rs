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

// ---------------------------------------------------------------------------------
// #8 glob and directory write sets

#[test]
fn planning_guidance_allows_glob_and_directory_writes() {
    let guidance = spec_autonomous_core::verification_preflight::planning_guidance_addendum();
    assert!(guidance.contains("server/*/migrations/*.py"), "{guidance}");
    assert!(guidance.contains("tests/**"), "{guidance}");
    assert!(guidance.contains("directory prefixes"), "{guidance}");
}

#[test]
fn glob_writes_cover_their_matches_and_conflict_conservatively() {
    use spec_autonomous_core::plan;
    let writes = vec!["server/*/migrations/*.py".to_owned(), "tests/**".to_owned()];
    assert!(plan::allowed(
        "server/billing/migrations/0043_auto.py",
        &writes
    ));
    assert!(plan::allowed("tests/services/test_index.py", &writes));
    assert!(!plan::allowed("server/billing/models.py", &writes));
    assert!(!plan::allowed("web/tests/x.ts", &writes));
    // Directory entries cover everything below them.
    assert!(plan::allowed("fixtures/a/b.json", &["fixtures".into()]));
    // Overlap is judged on the literal prefix before the first wildcard: conservative.
    assert!(plan::overlaps(
        "server/*/migrations/*.py",
        "server/billing/models.py"
    ));
    assert!(plan::overlaps("tests/**", "tests/unit/a.py"));
    assert!(!plan::overlaps(
        "server/*/migrations/*.py",
        "web/src/app.ts"
    ));
    assert!(plan::overlaps("**/*.py", "web/src/app.ts"));
    let with = |id: &str, writes: &[&str]| Task {
        writes: writes.iter().map(|w| (*w).to_owned()).collect(),
        ..task(id, "T001", "")
    };
    let plan_of = |tasks: Vec<Task>| Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: "h".into(),
        tasks,
        milestone: None,
    };
    let disjoint = plan_of(vec![
        with("a", &["server/*/migrations/*.py"]),
        with("b", &["web/**"]),
    ]);
    assert_eq!(plan::ready(&disjoint, &[], &[], 4).len(), 2);
    let overlapping = plan_of(vec![
        with("a", &["server/*/migrations/*.py"]),
        with("b", &["server/billing/models.py"]),
        with("c", &["**/*.md"]),
    ]);
    let ready: Vec<_> = plan::ready(&overlapping, &[], &[], 4)
        .iter()
        .map(|t| t.id.clone())
        .collect();
    assert_eq!(ready, ["a"], "overlapping patterns serialize");
    // A glob never grants edits to native context; an exact entry still does.
    let context = ["specs/feature/spec.md"];
    assert!(plan::worker_write_forbidden(
        "specs/feature/spec.md",
        &["specs/**".into()],
        &context
    ));
    assert!(plan::worker_write_forbidden(
        "specs/feature/spec.md",
        &["specs/feature".into()],
        &context
    ));
    assert!(!plan::worker_write_forbidden(
        "specs/feature/spec.md",
        &["specs/feature/spec.md".into()],
        &context
    ));
    assert!(!plan::worker_write_forbidden(
        "specs/feature/notes.txt",
        &["specs/**".into()],
        &context
    ));
}

#[test]
fn a_planned_glob_write_is_accepted_and_serializes_overlapping_tasks() {
    let root = fixture(|_| {});
    let root = root.path();
    let (run_id, wave) = start_with(root, |tasks| {
        tasks[0].writes = vec!["src/*.txt".into()];
    });
    let issued: Vec<_> = wave["work"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["kind"] == "implement")
        .map(|w| w["task_id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        issued,
        ["value"],
        "src/*.txt overlaps src/other.txt: {wave}"
    );
    let request = work(&wave, "value").clone();
    write_value(&request);
    let project = Path::new(request["project"].as_str().unwrap());
    fs::write(project.join("src/extra.txt"), "covered by the glob").unwrap();
    let next = submit(root, &request, receipt(&packet(root, &request)));
    let run = saved(root, &run_id);
    assert!(
        run.completed_tasks.contains(&Run::key("P001", "value")),
        "{:?}",
        run.attempts
            .iter()
            .map(|a| (&a.task_id, &a.status, &a.error))
            .collect::<Vec<_>>()
    );
    work(&next, "other");
}

// ---------------------------------------------------------------------------------
// #35 deferred real-environment evidence

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
fn live(description: &str, requirement: Option<&str>) -> DeferredEvidence {
    DeferredEvidence {
        kind: "live".into(),
        description: description.into(),
        reason: "needs the staging deployment".into(),
        requirement: requirement.map(str::to_owned),
    }
}

#[test]
fn implement_receipts_carry_deferred_evidence_into_views_and_progress() {
    let root = fixture(|_| {});
    let root = root.path();
    let (run_id, wave) = start(root);
    let request = work(&wave, "value").clone();
    let schema: Value =
        serde_json::from_slice(&fs::read(request["result_schema_path"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(
        schema["properties"]["deferred_evidence"]["items"]["properties"]["kind"]["enum"],
        json!(["live", "manual", "external"])
    );
    assert!(
        !schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("deferred_evidence"))
    );
    let prompt = fs::read_to_string(request["prompt_path"].as_str().unwrap()).unwrap();
    assert!(prompt.contains("DEFERRED EVIDENCE"), "{prompt}");
    write_value(&request);
    let input = packet(root, &request);

    // A bad kind is a protocol error and nothing is integrated.
    let mut bad = receipt(&input);
    bad.deferred_evidence = vec![DeferredEvidence {
        kind: "maybe".into(),
        ..live("curl the staging endpoint", None)
    }];
    let rejected = api::invoke(
        root,
        "apply-result",
        &json!({"token":request["token"],"host":request["owner"],"result":bad,"view":"full"}),
    );
    let text = format!("{rejected:?}");
    assert!(text.contains("deferred_evidence kind"), "{text}");

    let request = work(
        &api::invoke(
            root,
            "prepare",
            &json!({"run_id":run_id,"host":host(),"view":"full"}),
        )
        .unwrap(),
        "value",
    )
    .clone();
    let project = Path::new(request["project"].as_str().unwrap());
    if !project.join("src/value.txt").exists() {
        write_value(&request);
    }
    let mut result = receipt(&packet(root, &request));
    result.deferred_evidence = vec![live("curl https://staging/value returns 42", None)];
    let response = submit(root, &request, result);
    let run = saved(root, &run_id);
    assert!(
        run.completed_tasks.contains(&Run::key("P001", "value")),
        "{response}"
    );
    let listed = response["deferred_evidence"]
        .as_array()
        .unwrap_or_else(|| panic!("{response}"));
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["task_id"], "value");
    assert_eq!(listed[0]["kind"], "live");
    assert_eq!(listed[0]["kind_of_work"], "implement");
    assert_ne!(
        response["status"], "needs_input",
        "not a failure: {response}"
    );
    let compact = api::invoke(root, "next", &json!({"run_id":run_id})).unwrap();
    assert_eq!(
        compact["deferred_evidence"][0]["task_id"], "value",
        "{compact}"
    );
    let progress = api::invoke(root, "progress", &json!({})).unwrap();
    let text = progress.to_string();
    assert!(
        text.contains("curl https://staging/value returns 42"),
        "{progress}"
    );
}

#[test]
fn audit_items_may_defer_live_evidence_and_the_run_completes_with_follow_ups() {
    let root = fixture(|_| {});
    let root = root.path();
    let (run_id, wave) = start(root);
    let mut response = finish_tasks(root, &wave);
    let mut first = true;
    let mut rounds = 0;
    while let Some(audit) = find_kind(&response, "audit").cloned() {
        rounds += 1;
        assert!(rounds < 5, "{response}");
        let input = packet(root, &audit);
        let schema: Value = serde_json::from_slice(
            &fs::read(audit["result_schema_path"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        assert!(schema["properties"]["deferred_evidence"].is_object());
        let ids = acceptance_ids(&input);
        let mut result = receipt(&input);
        result.audit = ids
            .iter()
            .map(|id| AuditItem {
                requirement: id.clone(),
                evidence: "src/value.txt contains 42; node --test tests/value.mjs".into(),
                passed: true,
            })
            .collect();
        if first {
            // An id outside this packet is refused.
            let mut wrong = result.clone();
            wrong.deferred_evidence = vec![live("x", Some("FR-999-not-here"))];
            let error = api::invoke(
                root,
                "apply-result",
                &json!({"token":audit["token"],"host":audit["owner"],"result":wrong,"view":"full"}),
            );
            assert!(
                format!("{error:?}").contains("is not an acceptance id"),
                "{error:?}"
            );
            let audit = find_kind(
                &api::invoke(
                    root,
                    "prepare",
                    &json!({"run_id":run_id,"host":host(),"view":"full"}),
                )
                .unwrap(),
                "audit",
            )
            .unwrap()
            .clone();
            let input = packet(root, &audit);
            result = receipt(&input);
            result.audit = acceptance_ids(&input)
                .iter()
                .map(|id| AuditItem {
                    requirement: id.clone(),
                    evidence: "verified in checkout".into(),
                    passed: true,
                })
                .collect();
            result.deferred_evidence = vec![live(
                "value visible on staging",
                Some(&acceptance_ids(&input)[0]),
            )];
            first = false;
            response = submit(root, &audit, result);
            continue;
        }
        response = submit(root, &audit, result);
    }
    assert_eq!(response["status"], "completed", "{response}");
    let warnings = response["warnings"].to_string();
    assert!(
        warnings.contains("deferred_evidence_pending: 1 item"),
        "{response}"
    );
    let items = response["deferred_evidence"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{response}");
    assert_eq!(items[0]["kind_of_work"], "audit");
    assert!(items[0]["requirement"].is_string());
}

#[test]
fn receipts_without_deferred_evidence_keep_their_bytes() {
    let result = WorkerResult {
        schema_version: 1,
        run_id: "r".into(),
        task_id: "t".into(),
        attempt_id: "a".into(),
        status: "candidate".into(),
        summary: "s".into(),
        blockers: vec![],
        milestone: None,
        plan: None,
        audit: vec![],
        requested_writes: vec![],
        deferred_evidence: vec![],
    };
    let text = serde_json::to_string(&result).unwrap();
    assert!(!text.contains("deferred_evidence"), "{text}");
    let old: WorkerResult = serde_json::from_str(&text).unwrap();
    assert!(old.deferred_evidence.is_empty());
}

// ---------------------------------------------------------------------------------
// #4/#28 configurable worktree root

fn doctor_diagnostics(root: &Path) -> Vec<String> {
    serde_json::from_value(api::invoke(root, "doctor", &json!({})).unwrap()["diagnostics"].clone())
        .unwrap()
}

#[test]
fn worktree_root_config_is_validated_and_omitted_while_default() {
    let config = Config::default();
    assert_eq!(config.execution.worktree_root, "git");
    let text = toml::to_string(&config).unwrap();
    assert!(!text.contains("worktree_root"), "{text}");
    let mut sibling = Config::default();
    sibling.execution.worktree_root = "sibling".into();
    sibling.validate().unwrap();
    assert!(
        toml::to_string(&sibling)
            .unwrap()
            .contains("worktree_root = \"sibling\"")
    );
    let mut relative = Config::default();
    relative.execution.worktree_root = "worktrees".into();
    assert!(
        relative
            .validate()
            .unwrap_err()
            .to_string()
            .contains("worktree_root must be git, sibling or an absolute directory")
    );
}

#[test]
fn sibling_worktree_root_puts_worker_checkouts_outside_git_and_reclaims_them() {
    let root = fixture(|config| config.execution.worktree_root = "sibling".into());
    let root = root.path();
    let repo = Repository::discover(root).unwrap();
    let sibling = repo.worktree_root("sibling").unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(sibling.clone());
    assert_eq!(
        sibling.file_name().unwrap().to_string_lossy(),
        format!(
            "{}.sa-worktrees",
            root.canonicalize()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
        )
    );
    let report = doctor_diagnostics(root);
    assert!(
        !report.iter().any(|d| d.starts_with("worktree_root")),
        "{report:?}"
    );
    let (run_id, wave) = start(root);
    let request = work(&wave, "value").clone();
    let worktree = Path::new(request["worktree"].as_str().unwrap());
    let sibling = sibling.canonicalize().unwrap();
    assert_eq!(worktree.parent().unwrap(), sibling, "{request}");
    assert!(!worktree.to_string_lossy().contains("/.git/"));
    let run = saved(root, &run_id);
    assert!(
        run.integration.contains("spec-autonomous/worktrees"),
        "coordinator checkouts stay in the runtime directory: {}",
        run.integration
    );
    write_value(&request);
    submit(root, &request, receipt(&packet(root, &request)));
    let run = saved(root, &run_id);
    let attempt = run
        .attempts
        .iter()
        .find(|a| a.id == request["request_id"].as_str().unwrap())
        .unwrap();
    assert_eq!(attempt.status, "integrated");
    assert_eq!(
        attempt.worktree_state.as_deref(),
        Some("reclaimed"),
        "terminal reclaim covers the configured root"
    );
    assert!(!worktree.exists());
}

#[test]
fn existing_runs_keep_their_worktree_root_and_doctor_explains_the_default() {
    let root = fixture(|_| {});
    let root = root.path();
    let report = doctor_diagnostics(root);
    let notice = report
        .iter()
        .find(|d| d.starts_with("worktree_root_notice"))
        .unwrap_or_else(|| panic!("{report:?}"));
    assert!(notice.contains("worktree_root = \"sibling\""), "{notice}");
    assert!(notice.contains(".sa-worktrees"), "{notice}");
    let (run_id, wave) = start(root);
    // The project switches to the sibling root; the running run keeps its frozen config.
    let mut config = Config::load(root).unwrap();
    config.execution.worktree_root = "sibling".into();
    put(
        root,
        ".spec-autonomous/config.toml",
        &toml::to_string(&config).unwrap(),
    );
    let request = work(&wave, "value").clone();
    write_value(&request);
    let next = submit(root, &request, receipt(&packet(root, &request)));
    let other = work(&next, "other");
    assert!(
        other["worktree"]
            .as_str()
            .unwrap()
            .contains("spec-autonomous/worktrees"),
        "{other}"
    );
    assert_eq!(saved(root, &run_id).config.execution.worktree_root, "git");
}

#[test]
fn absolute_worktree_roots_are_per_repository_and_never_inside_the_checkout() {
    let root = fixture(|_| {});
    let root = root.path();
    let repo = Repository::discover(root).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let path = repo
        .worktree_root(elsewhere.path().to_str().unwrap())
        .unwrap();
    assert_eq!(
        path.parent().unwrap(),
        elsewhere.path().canonicalize().unwrap()
    );
    let inside = root.canonicalize().unwrap().join("worktrees");
    let error = repo
        .worktree_root(inside.to_str().unwrap())
        .unwrap_err()
        .to_string();
    assert!(error.contains("inside the main checkout"), "{error}");
    let (path, branch) = repo
        .add_worktree_in(elsewhere.path().to_str().unwrap(), "tool-x", "HEAD")
        .unwrap();
    assert_eq!(branch, "codex/sa/tool-x");
    assert!(path.join("tests/value.mjs").is_file());
    assert!(repo.in_worktree_roots(elsewhere.path().to_str().unwrap(), &path));
    assert!(!repo.in_worktree_roots("git", &path));
}

// ---------------------------------------------------------------------------------
// #36 open-ended specifications: extensions are not gaps

fn audit_item(requirement: &str, evidence: &str, passed: bool) -> AuditItem {
    AuditItem {
        requirement: requirement.into(),
        evidence: evidence.into(),
        passed,
    }
}

#[test]
fn extension_evidence_marks_an_item_that_is_never_a_gap() {
    assert!(audit_item("a", "extension: also accepts modules/", true).is_extension());
    assert!(audit_item("a", "  Extension: plugins/ too", false).is_extension());
    assert!(audit_item("a", "[extension] services/", false).is_extension());
    assert!(!audit_item("a", "missing apps/ prefix; extension later", false).is_extension());
    assert!(audit_item("a", "missing apps/", false).is_gap());
    assert!(!audit_item("a", "extension: services/", false).is_gap());
    assert!(!audit_item("a", "ok", true).is_gap());
}

#[test]
fn an_audit_failing_only_on_extensions_passes_and_mixed_gaps_omit_them() {
    let root = fixture(|_| {});
    let root = root.path();
    let (_, wave) = start(root);
    let audited = finish_tasks(root, &wave);
    let audit = find_kind(&audited, "audit").unwrap().clone();
    let prompt = fs::read_to_string(audit["prompt_path"].as_str().unwrap()).unwrap();
    assert!(prompt.contains("OPEN-ENDED SPECIFICATIONS"), "{prompt}");
    assert!(
        prompt.contains("start evidence with `extension:`"),
        "{prompt}"
    );
    let input = packet(root, &audit);
    let ids = acceptance_ids(&input);
    assert!(ids.len() >= 2, "{ids:?}");
    let mut result = receipt(&input);
    result.audit = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            if i == 0 {
                audit_item(
                    id,
                    "extension: also accepts services/ beyond the listed prefixes",
                    false,
                )
            } else {
                audit_item(id, "src/value.txt contains 42", true)
            }
        })
        .collect();
    let response = submit(root, &audit, result);
    assert!(
        find_kind(&response, "converge").is_none(),
        "an extension never starts a repair: {response}"
    );
    assert_ne!(response["status"], "needs_input", "{response}");
}

#[test]
fn converge_lists_real_gaps_and_leaves_extension_items_out() {
    let root = fixture(|_| {});
    let root = root.path();
    let (_, wave) = start(root);
    let audited = finish_tasks(root, &wave);
    let audit = find_kind(&audited, "audit").unwrap().clone();
    let input = packet(root, &audit);
    let ids = acceptance_ids(&input);
    let mut result = receipt(&input);
    result.audit = ids
        .iter()
        .enumerate()
        .map(|(i, id)| match i {
            0 => audit_item(id, "FR-001 value is missing", false),
            1 => audit_item(id, "extension: handles plugins/ as well", false),
            _ => audit_item(id, "ok", true),
        })
        .collect();
    let failed = submit(root, &audit, result);
    let converge = find_kind(&failed, "converge")
        .unwrap_or_else(|| panic!("{failed}"))
        .clone();
    let instruction = packet(root, &converge).instruction;
    assert!(
        instruction.contains(&format!(
            "Gaps (1 failed; {} passed audit items omitted)",
            ids.len() - 1
        )),
        "{instruction}"
    );
    assert!(instruction.contains("FR-001 value is missing"));
    assert!(!instruction.contains("handles plugins/"), "{instruction}");
}
