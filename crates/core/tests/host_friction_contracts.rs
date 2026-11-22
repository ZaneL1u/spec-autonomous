//! Host friction found in real runs: growing a task's write set, preparing worker
//! checkouts and keeping work packets small. Real Git and SQLite, no agents.
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
        verify_only: false,
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

#[test]
fn run_revise_appends_writes_and_the_redispatched_packet_carries_them() {
    let root = fixture(|_| {});
    let root = root.path();
    let (run_id, wave) = start(root);
    let request = work(&wave, "value").clone();
    write_value(&request);
    let input = packet(root, &request);
    let mut blocked = receipt(&input);
    blocked.status = "blocked".into();
    blocked.blockers = vec!["tests/value.mjs must change too".into()];
    let stopped = submit(root, &request, blocked);
    assert_eq!(stopped["status"], "needs_input", "{stopped}");

    let bad = |add: Value| {
        api::invoke(
            root,
            "run.revise",
            &json!({"run_id":run_id,"reason":"value tests change","apply":true,"task_writes":[{"phase_id":"P001","task_id":"value","add":add}]}),
        )
        .unwrap_err()
        .to_string()
    };
    assert!(bad(json!([".spec-autonomous/config.toml"])).contains("write_scope_forbidden"));
    assert!(bad(json!(["specs/feature/spec.md"])).contains("write_scope_forbidden"));
    assert!(bad(json!(["specs/**"])).contains("write_scope_forbidden"));
    assert!(bad(json!(["../outside.txt"])).contains("write_scope_invalid"));
    assert!(bad(json!(["src/value.txt"])).contains("already allow"));

    let applied = api::invoke(
        root,
        "run.revise",
        &json!({"run_id":run_id,"reason":"tests/value.mjs asserts the old format and must change with the value","apply":true,"task_writes":[{"phase_id":"P001","task_id":"value","add":["./tests//value.mjs"]}]}),
    )
    .unwrap();
    assert_eq!(applied["applied"], true, "{applied}");
    let diff = &applied["revision"]["diff"][0];
    assert_eq!(diff["scope"], "task_writes");
    assert_eq!(diff["added"], json!(["tests/value.mjs"]));
    assert_eq!(diff["after"], json!(["src/value.txt", "tests/value.mjs"]));
    let run = saved(root, &run_id);
    assert!(
        run.host
            .as_ref()
            .unwrap()
            .task_retry_epochs
            .contains_key(&Run::key("P001", "value"))
    );

    let resumed = api::invoke(
        root,
        "prepare",
        &json!({"run_id":run_id,"host":host(),"view":"full"}),
    )
    .unwrap();
    let retry = work(&resumed, "value");
    assert_ne!(retry["request_id"], request["request_id"]);
    let next = packet(root, retry);
    assert_eq!(
        next.task.as_ref().unwrap().writes,
        ["src/value.txt", "tests/value.mjs"]
    );
    assert!(
        next.previous_candidate.is_some(),
        "blocked changes carried over"
    );
}

#[test]
fn reconcile_plan_keeps_revised_writes_after_replanning() {
    let mut plan = Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: "h".into(),
        tasks: vec![
            task("value", "T001", "src/value.txt"),
            Task {
                writes: vec![],
                ..task("open", "T002", "")
            },
        ],
        milestone: None,
    };
    let revisions = vec![
        json!({"diff":[{"scope":"task_writes","phase_id":"P001","task_id":"value","before":["src/value.txt"],"after":["src/value.txt","tests/value.mjs"],"added":["tests/value.mjs"]}]}),
        json!({"diff":[{"scope":"task_writes","phase_id":"P001","task_id":"open","before":[],"after":["x"],"added":["x"]}]}),
    ];
    spec_autonomous_core::run_revision::reconcile_plan(&mut plan, &revisions).unwrap();
    assert_eq!(plan.tasks[0].writes, ["src/value.txt", "tests/value.mjs"]);
    assert!(
        plan.tasks[1].writes.is_empty(),
        "an unrestricted task keeps its open scope"
    );
    // Applying twice does not duplicate.
    spec_autonomous_core::run_revision::reconcile_plan(&mut plan, &revisions).unwrap();
    assert_eq!(plan.tasks[0].writes.len(), 2);
}

#[test]
fn blocked_receipt_with_requested_writes_extends_the_scope_and_redispatches() {
    let root = fixture(|_| {});
    let root = root.path();
    let (run_id, wave) = start(root);
    let request = work(&wave, "value").clone();
    write_value(&request);
    let input = packet(root, &request);
    let schema: Value =
        serde_json::from_slice(&fs::read(request["result_schema_path"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(schema["properties"]["requested_writes"]["maxItems"], 10);
    let mut blocked = receipt(&input);
    blocked.status = "blocked".into();
    blocked.blockers = vec!["tests/value.mjs pins the old value".into()];
    blocked.requested_writes = vec!["tests/value.mjs".into()];
    let next = submit(root, &request, blocked);
    assert_ne!(next["status"], "needs_input", "{next}");
    let retry = work(&next, "value");
    assert_ne!(retry["request_id"], request["request_id"]);
    assert!(retry["token"].is_string(), "re-issued work is claimed");
    let input = packet(root, retry);
    assert_eq!(
        input.task.as_ref().unwrap().writes,
        ["src/value.txt", "tests/value.mjs"]
    );
    let previous = input
        .previous_candidate
        .clone()
        .expect("previous candidate");
    assert_eq!(previous.files, ["src/value.txt"]);
    assert!(
        input
            .failure
            .as_deref()
            .unwrap()
            .starts_with("write_scope_extended")
    );
    let run = saved(root, &run_id);
    let record = run
        .host
        .as_ref()
        .unwrap()
        .verification_revisions
        .last()
        .unwrap();
    assert_eq!(record["automatic"], true);
    assert_eq!(record["diff"][0]["added"], json!(["tests/value.mjs"]));

    // The re-issued worker restores the patch, edits the newly allowed test and finishes.
    let worktree = Path::new(retry["worktree"].as_str().unwrap());
    git::command(worktree, &["apply", "--index", &previous.patch_path], None).unwrap();
    let project = Path::new(retry["project"].as_str().unwrap());
    fs::write(
        project.join("tests/value.mjs"),
        "import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';assert.equal(readFileSync('src/value.txt','utf8').trim(),'42');\n",
    )
    .unwrap();
    let accepted = submit(root, retry, receipt(&input));
    assert_ne!(accepted["status"], "needs_input", "{accepted}");
    assert!(
        saved(root, &run_id)
            .completed_tasks
            .contains(&Run::key("P001", "value")),
        "{accepted}"
    );
}

#[test]
fn unsafe_requested_writes_stop_for_review_with_a_revise_example() {
    for (requested, code) in [
        (
            vec![".spec-autonomous/config.toml"],
            "write_scope_forbidden",
        ),
        (vec!["specs/feature/tasks.md"], "write_scope_forbidden"),
        (vec!["src/other.txt"], "write_scope_conflict"),
        (
            (0..11).map(|_| "tests/value.mjs").collect::<Vec<_>>(),
            "write_scope_limit",
        ),
    ] {
        let root = fixture(|_| {});
        let root = root.path();
        let (run_id, wave) = start(root);
        let request = work(&wave, "value").clone();
        let other = work(&wave, "other").clone();
        let input = packet(root, &request);
        let mut blocked = receipt(&input);
        blocked.status = "blocked".into();
        blocked.blockers = vec!["needs more files".into()];
        blocked.requested_writes = requested.iter().map(|s| s.to_string()).collect();
        let stopped = submit(root, &request, blocked);
        assert_eq!(stopped["status"], "needs_input", "{stopped}");
        let blocker = stopped["blocker"].as_str().unwrap();
        assert!(blocker.contains(code), "{code}: {blocker}");
        assert!(blocker.contains("tools call run.revise"), "{blocker}");
        assert!(blocker.contains("\"task_writes\""), "{blocker}");
        let run = saved(root, &run_id);
        assert_eq!(
            run.plans["P001"].tasks[0].writes,
            ["src/value.txt"],
            "{code}: scope unchanged"
        );
        // The manual path enforces the same conflict rule while the sibling is out.
        if code == "write_scope_conflict" {
            assert!(other["token"].is_string());
            let error = api::invoke(
                root,
                "run.revise",
                &json!({"run_id":run_id,"reason":"share other","apply":true,"task_writes":[{"phase_id":"P001","task_id":"value","add":["src/other.txt"]}]}),
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("write_scope_conflict"), "{error}");
        }
    }
}

/// Lets `git submodule` clone a local file:// repository in tests.
fn allow_file_protocol(config: &mut Config) {
    for (key, value) in [
        ("GIT_CONFIG_COUNT", "1"),
        ("GIT_CONFIG_KEY_0", "protocol.file.allow"),
        ("GIT_CONFIG_VALUE_0", "always"),
    ] {
        config.environment.insert(key.into(), value.into());
    }
}
/// A separate repository added as the `vendor/lib` submodule.
fn add_submodule(root: &Path) -> tempfile::TempDir {
    let library = tempfile::tempdir().unwrap();
    put(library.path(), "lib.txt", "shared library\n");
    git::command(library.path(), &["init", "-q", "-b", "main"], None).unwrap();
    git::commit(library.path(), "library").unwrap();
    git::command(
        root,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            library.path().to_str().unwrap(),
            "vendor/lib",
        ],
        None,
    )
    .unwrap();
    library
}

#[test]
fn init_submodules_fills_worker_and_integration_checkouts() {
    for enabled in [true, false] {
        let library = std::cell::RefCell::new(None);
        let root = fixture_with(
            |c| {
                c.execution.init_submodules = enabled;
                allow_file_protocol(c);
            },
            |root| *library.borrow_mut() = Some(add_submodule(root)),
        );
        let root = root.path();
        let (run_id, wave) = start(root);
        let request = work(&wave, "value");
        let worktree = Path::new(request["worktree"].as_str().unwrap());
        let integration = saved(root, &run_id).integration;
        for checkout in [worktree, Path::new(&integration)] {
            assert_eq!(
                checkout.join("vendor/lib/lib.txt").is_file(),
                enabled,
                "{} enabled={enabled}",
                checkout.display()
            );
        }
        if enabled {
            // Initialized submodules leave the checkout clean, so the task still integrates.
            write_value(request);
            let accepted = submit(root, request, receipt(&packet(root, request)));
            assert_ne!(accepted["status"], "needs_input", "{accepted}");
            assert!(
                saved(root, &run_id)
                    .completed_tasks
                    .contains(&Run::key("P001", "value")),
                "{accepted}"
            );
        }
    }
}

#[test]
fn worker_prepare_runs_in_each_writing_checkout_before_issue() {
    let hook = "const fs=require('node:fs');fs.mkdirSync('dist',{recursive:true});fs.writeFileSync('dist/prepared.json',JSON.stringify({cwd:process.cwd(),checkout:process.env.SA_WORKER_CHECKOUT,project:process.env.SA_WORKER_PROJECT,task:process.env.SA_WORKER_TASK_ID,kind:process.env.SA_WORKER_KIND,origin:process.env.SA_ORIGIN_CHECKOUT}))";
    let root = fixture_with(
        |c| {
            c.hooks.insert(
                "worker_prepare".into(),
                spec_autonomous_core::config::Hook {
                    argv: vec!["node".into(), "scripts/worker-prepare.cjs".into()],
                    idempotent: true,
                },
            );
        },
        |root| put(root, "scripts/worker-prepare.cjs", hook),
    );
    let root = root.path();
    let (_, wave) = start(root);
    for task in ["value", "other"] {
        let request = work(&wave, task);
        let worktree = fs::canonicalize(request["worktree"].as_str().unwrap()).unwrap();
        let marker: Value = serde_json::from_slice(
            &fs::read(worktree.join("dist/prepared.json")).expect("hook ran before issue"),
        )
        .unwrap();
        assert_eq!(
            fs::canonicalize(marker["cwd"].as_str().unwrap()).unwrap(),
            worktree
        );
        assert_eq!(
            fs::canonicalize(marker["checkout"].as_str().unwrap()).unwrap(),
            worktree
        );
        assert_eq!(marker["task"], task);
        assert_eq!(marker["kind"], "implement");
        assert!(marker["project"].is_string() && marker["origin"].is_string());
    }
    // Ignored preparation output does not count as a worker change.
    let request = work(&wave, "value");
    write_value(request);
    let accepted = submit(root, request, receipt(&packet(root, request)));
    assert_ne!(accepted["status"], "needs_input", "{accepted}");
}

#[test]
fn failed_worker_preparation_issues_nothing_and_recovers_after_a_fix() {
    let hook = |code: i32| spec_autonomous_core::config::Hook {
        argv: vec![
            "node".into(),
            "-e".into(),
            // Read-only planning checkouts run the hook too; only the writing
            // worker's environment is broken here.
            format!(
                "if(process.env.SA_WORKER_KIND==='implement'){{console.error('uv sync failed');process.exit({code})}}"
            ),
        ],
        idempotent: true,
    };
    let root = fixture(|c| {
        c.hooks.insert("worker_prepare".into(), hook(3));
    });
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    let run_id = first["id"].as_str().unwrap().to_owned();
    let stopped = answer_plan(root, &first["work"][0]);
    assert_eq!(stopped["status"], "needs_input", "{stopped}");
    let blocker = stopped["blocker"].as_str().unwrap();
    assert!(blocker.contains("worker_prepare_failed"), "{blocker}");
    assert!(blocker.contains("exited with 3"), "{blocker}");
    assert!(blocker.contains("uv sync failed"), "{blocker}");
    assert!(blocker.contains("prepare --run-id"), "{blocker}");
    let run = saved(root, &run_id);
    let implement: Vec<_> = run
        .attempts
        .iter()
        .filter(|a| a.kind == "implement")
        .collect();
    assert_eq!(implement.len(), 1);
    assert_eq!(implement[0].status, "superseded");
    assert!(
        !run.host
            .as_ref()
            .unwrap()
            .requests
            .contains_key(&implement[0].id)
    );

    let mut config = Config::load(root).unwrap();
    config.hooks.insert("worker_prepare".into(), hook(0));
    put(
        root,
        ".spec-autonomous/config.toml",
        &toml::to_string(&config).unwrap(),
    );
    let resumed = api::invoke(
        root,
        "prepare",
        &json!({"run_id":run_id,"host":host(),"view":"full","reload_config":true}),
    )
    .unwrap();
    let retry = work(&resumed, "value");
    assert!(retry["token"].is_string(), "{resumed}");

    let mut lax = Config::default();
    lax.hooks.insert(
        "worker_prepare".into(),
        spec_autonomous_core::config::Hook {
            idempotent: false,
            ..hook(0)
        },
    );
    assert!(
        lax.validate()
            .unwrap_err()
            .to_string()
            .contains("worker_prepare must be idempotent")
    );
}

#[test]
fn doctor_warns_about_unbound_prepare_scripts_and_uninitialized_submodules() {
    let library = std::cell::RefCell::new(None);
    let root = fixture_with(
        |_| {},
        |root| {
            *library.borrow_mut() = Some(add_submodule(root));
            put(
                root,
                ".spec-autonomous/verification-prepare.sh",
                "#!/bin/sh\n",
            );
            put(
                root,
                ".spec-autonomous/scripts/worker-prepare.py",
                "print()\n",
            );
        },
    );
    let root = root.path();
    let diagnostics = |root: &Path| -> Vec<String> {
        serde_json::from_value(
            api::invoke(root, "doctor", &json!({})).unwrap()["diagnostics"].clone(),
        )
        .unwrap()
    };
    let report = diagnostics(root);
    let find = |code: &str| report.iter().find(|d| d.starts_with(code)).cloned();
    let verification = find("hook_unbound_warning: .spec-autonomous/verification-prepare.sh")
        .unwrap_or_else(|| panic!("{report:?}"));
    assert!(verification.contains("[hooks.verification_prepare]"));
    let worker = find("hook_unbound_warning: .spec-autonomous/scripts/worker-prepare.py")
        .unwrap_or_else(|| panic!("{report:?}"));
    assert!(worker.contains("[hooks.worker_prepare]"));
    assert!(find("submodules_warning").is_some(), "{report:?}");

    let mut config = Config::load(root).unwrap();
    for (name, script) in [
        (
            "verification_prepare",
            ".spec-autonomous/verification-prepare.sh",
        ),
        (
            "worker_prepare",
            ".spec-autonomous/scripts/worker-prepare.py",
        ),
    ] {
        config.hooks.insert(
            name.into(),
            spec_autonomous_core::config::Hook {
                argv: vec!["sh".into(), script.into()],
                idempotent: true,
            },
        );
    }
    put(
        root,
        ".spec-autonomous/config.toml",
        &toml::to_string(&config).unwrap(),
    );
    let report = diagnostics(root);
    assert!(
        !report
            .iter()
            .any(|d| d.starts_with("hook_unbound_warning") || d.starts_with("submodules_warning")),
        "{report:?}"
    );
}

#[test]
fn implement_prompt_is_a_small_summary_of_a_trimmed_request() {
    let bulk = "Repository rule: keep every handler documented and tested. ".repeat(600);
    let root = fixture_with(|_| {}, |root| put(root, "AGENTS.md", &bulk));
    let root = root.path();
    put(
        root,
        "specs/feature/plan.md",
        &format!("# Plan\nUse a plain UTF-8 file and Node assertions.\n{bulk}\n"),
    );
    git::commit(root, "large plan").unwrap();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    let planning = &first["work"][0];
    let plan_input: Value =
        serde_json::from_slice(&fs::read(planning["input_path"].as_str().unwrap()).unwrap())
            .unwrap();
    let plan_meta = &plan_input["snapshot"]["metadata"];
    assert!(
        plan_meta.get("documents").is_some(),
        "planners keep outlines"
    );
    assert!(
        plan_meta.get("acceptance").is_none(),
        "only audits get acceptance"
    );
    let plan_prompt = fs::read_to_string(planning["prompt_path"].as_str().unwrap()).unwrap();
    assert!(plan_prompt.contains("snapshot.tasks (2; cover every one)"));
    assert!(plan_prompt.contains("T001"));

    let wave = answer_plan(root, planning);
    let request = work(&wave, "value");
    let input_path = request["input_path"].as_str().unwrap();
    let input = fs::read(input_path).unwrap();
    let prompt = fs::read_to_string(request["prompt_path"].as_str().unwrap()).unwrap();
    let value: Value = serde_json::from_slice(&input).unwrap();
    let meta = &value["snapshot"]["metadata"];
    assert!(meta.get("documents").is_none() && meta.get("acceptance").is_none());
    assert!(
        value["snapshot"]["context_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == "specs/feature/plan.md"),
        "native context stays complete in the input file"
    );
    // The old prompt embedded the whole request, so it was larger than input.json.
    assert!(input.len() > 30 * 1024, "{}", input.len());
    assert!(prompt.len() < 8 * 1024, "{}", prompt.len());
    assert!(prompt.len() * 4 < input.len());
    assert!(!prompt.contains("Repository rule: keep every handler"));
    for expected in [
        format!("REQUEST: {input_path}"),
        format!("sha256 {}", spec_autonomous_core::paths::hash(&input)),
        "task.id: value".into(),
        "task.writes: src/value.txt".into(),
        "task.depends_on: none".into(),
        "`node --test tests/value.mjs` (cwd .)".into(),
        "requested_writes".into(),
        "specs/feature/plan.md".into(),
    ] {
        assert!(prompt.contains(&expected), "{expected}\n{prompt}");
    }
    // The trimmed packet still verifies against its recorded input hash.
    let input = packet(root, request);
    assert_eq!(input.task.unwrap().id, "value");
}

#[test]
fn trimming_unwraps_native_apply_json_and_dedupes_planning_failures() {
    let snapshot = |instruction: String, metadata: Value| -> Value {
        json!({"schema_version":1,"framework":"openspec","selector":"demo","source_dir":"openspec/changes/demo","tracking_file":"openspec/changes/demo/tasks.md","planning_ready":true,
            "next_action":{"kind":"implement","artifact":"apply","instruction":instruction,"outputs":[]},
            "tasks":[],"context_files":[],"diagnostics":[],"source_hash":"h","metadata":metadata})
    };
    let apply = json!({"changeName":"demo","contextFiles":{"tasks":["/x/tasks.md"]},"tasks":[{"id":"1","description":"one","done":false}],"progress":{"total":1,"complete":0,"remaining":1},"state":"ready","warnings":["proposal is short"],"context":"Python 3.14 service","instruction":"Implement the pending tasks in order."});
    let base = |kind: &str, snapshot: Value, failure: Option<&str>| -> WorkerInput {
        serde_json::from_value(json!({"schema_version":1,"run_id":"run-trim","task_id":"t","attempt_id":"attempt-trim","kind":kind,"goal":"g","framework":"openspec","base_commit":"abc","instruction":"do it","task":null,"snapshot":snapshot,"milestone":null,"failure":failure}))
            .unwrap()
    };
    let config = Config::default();
    let write = |input: &WorkerInput| -> Value {
        let dir = tempfile::tempdir().unwrap();
        spec_autonomous_core::work_packet::prepare(input, dir.path(), dir.path(), &config).unwrap();
        serde_json::from_slice(&fs::read(dir.path().join("input.json")).unwrap()).unwrap()
    };
    let implement = write(&base(
        "implement",
        snapshot(
            serde_json::to_string_pretty(&apply).unwrap(),
            json!({"documents":[1],"acceptance":[2],"execution":"kept"}),
        ),
        None,
    ));
    assert_eq!(
        implement["snapshot"]["next_action"]["instruction"],
        "Implement the pending tasks in order.\nWarning: proposal is short\n\nProject context:\nPython 3.14 service"
    );
    assert_eq!(
        implement["snapshot"]["metadata"],
        json!({"execution":"kept"})
    );
    let planning = write(&base(
        "plan-tasks",
        snapshot(
            serde_json::to_string_pretty(&apply).unwrap(),
            json!({"documents":[1],"acceptance":[2],"previous_planning_failure":{"request_id":"r","error":"invalid_plan: bad","action":"fix"}}),
        ),
        Some("invalid_plan: bad"),
    ));
    assert_eq!(planning["failure"], "invalid_plan: bad");
    assert_eq!(
        planning["snapshot"]["metadata"],
        json!({"documents":[1],"previous_planning_failure":{"request_id":"r","error_in":"failure","action":"fix"}})
    );
    assert!(
        planning["snapshot"]["next_action"]["instruction"]
            .as_str()
            .unwrap()
            .contains("contextFiles"),
        "non-implement work keeps the native JSON"
    );
    let audit = write(&base(
        "audit",
        snapshot("audit".into(), json!({"documents":[1],"acceptance":[2]})),
        None,
    ));
    assert_eq!(
        audit["snapshot"]["metadata"],
        json!({"documents":[1],"acceptance":[2]})
    );
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
fn find_kind<'a>(response: &'a Value, kind: &str) -> &'a Value {
    response["work"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} work: {response}"))
}

#[test]
fn converge_instruction_lists_only_failed_audit_items() {
    let root = fixture(|_| {});
    let root = root.path();
    let (_, wave) = start(root);
    let audited = finish_tasks(root, &wave);
    let audit = find_kind(&audited, "audit").clone();
    let input = packet(root, &audit);
    let ids: Vec<String> = input.snapshot.as_ref().unwrap().metadata["acceptance"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_owned())
        .collect();
    assert!(ids.len() >= 2, "{ids:?}");
    let long = "passed evidence ".repeat(200);
    let mut result = receipt(&input);
    result.audit = ids
        .iter()
        .enumerate()
        .map(|(i, id)| AuditItem {
            requirement: id.clone(),
            evidence: if i == 0 {
                "FR-001 value is missing".into()
            } else {
                long.clone()
            },
            passed: i != 0,
            files: vec![],
            carried_over: None,
        })
        .collect();
    let failed = submit(root, &audit, result);
    let converge = find_kind(&failed, "converge");
    let instruction = packet(root, converge).instruction;
    assert!(
        instruction.contains(&format!(
            "Gaps (1 failed; {} passed audit items omitted)",
            ids.len() - 1
        )),
        "{instruction}"
    );
    assert!(instruction.contains(&ids[0]), "{instruction}");
    assert!(instruction.contains("FR-001 value is missing"));
    assert!(!instruction.contains("passed evidence"), "{instruction}");
    assert!(!instruction.contains(&format!("\"{}\"", ids[1])));
}

#[test]
fn a_cancelled_integration_check_keeps_the_candidate_and_counts_no_failure() {
    let scratch = tempfile::tempdir().unwrap();
    let slow = scratch.path().join("slow");
    let started = scratch.path().join("started");
    fs::write(&slow, "").unwrap();
    let script = format!(
        "import assert from 'node:assert/strict';import {{readFileSync,existsSync,writeFileSync}} from 'node:fs';\
         if(existsSync({slow:?})){{writeFileSync({started:?},'1');await new Promise(r=>setTimeout(r,60000));}}\
         assert.equal(readFileSync('src/value.txt','utf8'),'42');\n",
        slow = slow.to_str().unwrap(),
        started = started.to_str().unwrap(),
    );
    let root = fixture_with(|_| {}, |root| put(root, "tests/slow-value.mjs", &script));
    let root = root.path();
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
    let mut value = task("value", &snapshot.tasks[0].id, "src/value.txt");
    value.verification = vec![Check {
        argv: vec!["node".into(), "tests/slow-value.mjs".into()],
        cwd: ".".into(),
        env: Default::default(),
    }];
    let mut result = receipt(&input);
    result.plan = Some(Plan {
        schema_version: 1,
        phase_id: "P001".into(),
        source_hash: snapshot.source_hash,
        tasks: vec![value, task("other", &snapshot.tasks[1].id, "src/other.txt")],
        milestone: input.milestone.clone(),
    });
    let wave = submit(root, planning, result);
    let request = work(&wave, "value").clone();
    write_value(&request);
    let receipt = receipt(&packet(root, &request));

    // The host process is interrupted while apply-result runs the integration check.
    let stopped = std::thread::scope(|scope| {
        let apply = scope.spawn(|| submit(root, &request, receipt.clone()));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while !started.exists() {
            assert!(std::time::Instant::now() < deadline, "check never started");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        api::invoke(root, "run.pause", &json!({"run_id":run_id})).unwrap();
        apply.join().unwrap()
    });
    assert_eq!(stopped["status"], "paused", "{stopped}");
    let run = saved(root, &run_id);
    let cancelled = run.evidence.last().unwrap().details.clone().unwrap();
    assert_eq!(cancelled["failure_kind"], "cancelled", "{cancelled}");
    let implement: Vec<_> = run
        .attempts
        .iter()
        .filter(|a| a.kind == "implement" && a.task_id == "value")
        .collect();
    assert_eq!(implement.len(), 1);
    assert_eq!(implement[0].status, "submitted", "{:?}", implement[0]);
    assert!(implement[0].error.is_none());

    // Resuming verifies the same receipt again instead of issuing new implement work.
    fs::remove_file(&slow).unwrap();
    let resumed = api::invoke(
        root,
        "prepare",
        &json!({"run_id":run_id,"host":host(),"view":"full"}),
    )
    .unwrap();
    let run = saved(root, &run_id);
    assert!(
        run.completed_tasks.contains(&Run::key("P001", "value")),
        "{resumed}"
    );
    let implement: Vec<_> = run
        .attempts
        .iter()
        .filter(|a| a.kind == "implement" && a.task_id == "value")
        .collect();
    assert_eq!(implement.len(), 1, "no re-implementation");
    assert_eq!(implement[0].status, "integrated");
    assert!(!run.attempts.iter().any(|a| a.status == "failed"));
}

#[test]
fn reload_config_keeps_earlier_extensions_and_budget_stops_say_how_much_to_add() {
    let root = fixture(|c| c.execution.run_timeout_seconds = 28800);
    let root = root.path();
    let (run_id, _) = start(root);
    let timeout = |root: &Path| saved(root, &run_id).config.execution.run_timeout_seconds;
    let resume = |extra: Value| {
        let mut args = json!({"run_id":run_id,"host":host(),"view":"full"});
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        api::invoke(root, "prepare", &args).unwrap()
    };
    resume(json!({"extend_seconds":43200}));
    assert_eq!(timeout(root), 72000);
    // Reloading an unchanged file keeps the extension and adds the new one.
    resume(json!({"reload_config":true,"extend_seconds":100}));
    assert_eq!(timeout(root), 72100);
    // A larger configured budget wins over the granted one.
    let mut config = Config::load(root).unwrap();
    config.execution.run_timeout_seconds = 90000;
    put(
        root,
        ".spec-autonomous/config.toml",
        &toml::to_string(&config).unwrap(),
    );
    resume(json!({"reload_config":true}));
    assert_eq!(timeout(root), 90000);

    // The deadline counts active time; the host holding issued packets is active.
    let root = fixture(|c| c.execution.run_timeout_seconds = 1);
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    let run_id = first["id"].as_str().unwrap().to_owned();
    std::thread::sleep(std::time::Duration::from_millis(2100));
    let stopped = api::invoke(
        root,
        "prepare",
        &json!({"run_id":run_id,"host":host(),"view":"full"}),
    )
    .unwrap();
    let blocker = stopped["blocker"].as_str().unwrap();
    assert!(
        blocker.starts_with("budget_exhausted: run deadline reached: "),
        "{blocker}"
    );
    assert!(blocker.contains("of run_timeout_seconds=1;"), "{blocker}");
    assert!(blocker.contains("s elapsed"), "{blocker}");
    assert!(
        blocker.contains(&format!("prepare --run-id {run_id} --extend-seconds")),
        "{blocker}"
    );
    let elapsed: u64 = blocker["budget_exhausted: run deadline reached: ".len()..]
        .split('s')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(elapsed >= 2, "{blocker}");
    assert!(
        blocker.contains(&format!("is at least {} plus", elapsed)),
        "{blocker}"
    );
}

#[test]
fn source_config_drift_is_reported_and_full_views_show_hooks() {
    let root = fixture(|_| {});
    let root = root.path();
    let (run_id, wave) = start(root);
    assert!(wave.get("warnings").is_none(), "{wave}");
    assert_eq!(wave["config"]["hooks"], json!({}), "{wave}");

    let mut config = Config::load(root).unwrap();
    config.hooks.insert(
        "verification_prepare".into(),
        spec_autonomous_core::config::Hook {
            argv: vec!["node".into(), "--version".into()],
            idempotent: true,
        },
    );
    put(
        root,
        ".spec-autonomous/config.toml",
        &toml::to_string(&config).unwrap(),
    );
    let drift = |value: &Value| {
        value["warnings"].as_array().is_some_and(|w| {
            w.iter()
                .any(|w| w.as_str().unwrap().starts_with("config_drift: "))
        })
    };
    let frozen = api::invoke(
        root,
        "prepare",
        &json!({"run_id":run_id,"host":host(),"view":"full"}),
    )
    .unwrap();
    assert!(drift(&frozen), "{frozen}");
    assert!(
        frozen["warnings"][0]
            .as_str()
            .unwrap()
            .contains("pass reload_config")
    );
    assert_eq!(frozen["config"]["hooks"], json!({}), "not reloaded");
    let compact = api::invoke(root, "prepare", &json!({"run_id":run_id,"host":host()})).unwrap();
    assert!(drift(&compact), "{compact}");
    let next = api::invoke(root, "next", &json!({"run_id":run_id})).unwrap();
    assert!(drift(&next), "{next}");

    let reloaded = api::invoke(
        root,
        "prepare",
        &json!({"run_id":run_id,"host":host(),"view":"full","reload_config":true,"extend_seconds":60}),
    )
    .unwrap();
    assert!(!drift(&reloaded), "{reloaded}");
    assert_eq!(
        reloaded["config"]["hooks"]["verification_prepare"],
        json!({"argv":["node","--version"],"idempotent":true}),
        "{reloaded}"
    );
    assert_eq!(reloaded["config"]["environment"], "redacted");
    // Run-local overrides (extend_seconds) are not drift.
    let again = api::invoke(root, "prepare", &json!({"run_id":run_id,"host":host()})).unwrap();
    assert!(!drift(&again), "{again}");
}

#[test]
fn no_progress_blocker_names_only_the_latest_failure_per_task() {
    let root = fixture(|c| c.execution.max_attempts = 2);
    let root = root.path();
    let (run_id, wave) = start(root);
    let other = work(&wave, "other").clone();
    put(
        Path::new(other["project"].as_str().unwrap()),
        "src/other.txt",
        "other",
    );
    submit(root, &other, receipt(&packet(root, &other)));
    let mut request = work(&wave, "value").clone();
    let mut response = Value::Null;
    for wrong in ["41", "43"] {
        put(
            Path::new(request["project"].as_str().unwrap()),
            "src/value.txt",
            wrong,
        );
        response = submit(root, &request, receipt(&packet(root, &request)));
        if wrong == "41" {
            request = work(&response, "value").clone();
        }
    }
    let blocker = response["blocker"].as_str().unwrap();
    assert!(
        blocker.starts_with("no_progress: no eligible task remains: value: verification_failed"),
        "{blocker}"
    );
    assert_eq!(
        blocker.matches("verification_failed").count(),
        1,
        "{blocker}"
    );
    assert!(
        blocker.ends_with("(1 earlier failures are listed in attempts)"),
        "{blocker}"
    );
    let run = saved(root, &run_id);
    let failed = run
        .attempts
        .iter()
        .filter(|a| a.task_id == "value" && a.status == "failed")
        .count();
    assert_eq!(failed, 2);
}
