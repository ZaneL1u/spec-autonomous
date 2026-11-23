//! Token economy: task sizing guidance for planners and per-kind host model hints.
//! Real Git and SQLite, no agents.
#![allow(dead_code)]
use serde_json::{Value, json};
use spec_autonomous_core::{capabilities as api, config::Config, engine, git, model::*};
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

// ---------------------------------------------------------------------------------
// Fresh contexts are the dominant cost of a run: planners are told to size tasks as one
// worker session, and per-kind model hints reach the host without changing unconfigured
// packets.

#[test]
fn planners_are_told_to_size_tasks_as_one_worker_session_outside_the_keyed_instruction() {
    let guidance = spec_autonomous_core::verification_preflight::task_sizing_guidance();
    for phrase in [
        "fresh worker context",
        "2-5 source_ids",
        "Never create one task per checkbox",
    ] {
        assert!(guidance.contains(phrase), "{phrase}: {guidance}");
    }
    let root = fixture(|_| {});
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    assert_eq!(first["work"][0]["kind"], "plan-tasks");
    let input = packet(root, &first["work"][0]);
    assert!(
        input.instruction.contains(guidance),
        "{}",
        input.instruction
    );
    let keyed = input
        .instruction
        .split(" Declare reads for every task:")
        .next()
        .unwrap();
    assert!(!keyed.contains("Task size:"), "{keyed}");
}

#[test]
fn unconfigured_packets_carry_no_model_hint() {
    let root = fixture(|_| {});
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    let requirements = &first["work"][0]["requirements"];
    assert_eq!(
        requirements,
        &json!({"fresh_context":true,"host_owned_execution":true})
    );
}

#[test]
fn host_models_reach_matching_packets_as_requirements_model() {
    let root = fixture(|config| {
        config
            .host
            .models
            .insert("plan-tasks".into(), "sonnet".into());
        config.host.models.insert("implement".into(), "opus".into());
    });
    let root = root.path();
    let first = api::invoke(
        root,
        "prepare",
        &json!({"feature":"specs/feature","host":host(),"view":"full","discussion_auto":true}),
    )
    .unwrap();
    assert_eq!(first["work"][0]["kind"], "plan-tasks");
    assert_eq!(first["work"][0]["requirements"]["model"], "sonnet");
    assert_eq!(first["work"][0]["requirements"]["fresh_context"], true);
}

#[test]
fn host_models_reject_unknown_kinds_and_malformed_names() {
    let mut config = Config::default();
    config
        .host
        .models
        .insert("audit".into(), "claude-sonnet-5-5".into());
    config.validate().unwrap();
    config.host.models.insert("review".into(), "sonnet".into());
    let error = config.validate().unwrap_err().to_string();
    assert!(error.contains("[host.models] key review"), "{error}");
    let mut config = Config::default();
    config
        .host
        .models
        .insert("audit".into(), "sonnet; rm -rf /".into());
    let error = config.validate().unwrap_err().to_string();
    assert!(error.contains("short model name"), "{error}");
    let mut config = Config::default();
    config.host.models.insert("audit".into(), String::new());
    assert!(config.validate().is_err());
}

#[test]
fn host_models_round_trip_through_toml() {
    let config: Config = toml::from_str(
        "schema_version = 2\n[host]\nmax_concurrency = 2\n[host.models]\naudit = \"haiku\"\nplan-tasks = \"sonnet\"\n",
    )
    .unwrap();
    assert_eq!(config.host.model_for("audit"), Some("haiku"));
    assert_eq!(config.host.model_for("implement"), None);
    let text = toml::to_string(&config).unwrap();
    let again: Config = toml::from_str(&text).unwrap();
    assert_eq!(again.host.models, config.host.models);
    // An empty table is not written back, so existing configs keep their exact shape.
    assert!(
        !toml::to_string(&Config::default())
            .unwrap()
            .contains("models")
    );
}
