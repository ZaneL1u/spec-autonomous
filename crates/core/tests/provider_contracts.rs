//! Native-document contracts: examples use real source bytes, not parser internals.
use spec_autonomous_core::{Framework, config::Config, markdown, paths, provider};
use std::{fs, path::Path};
use tempfile::TempDir;

fn put(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn feature() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    put(
        dir.path(),
        ".specify/memory/constitution.md",
        "# Constitution\nPreserve source requirements.\n",
    );
    put(
        dir.path(),
        "specs/001-space feature/spec.md",
        "# Feature\nProvide greeting.\n",
    );
    put(
        dir.path(),
        "specs/001-space feature/plan.md",
        "# Plan\nA local greeting command.\n",
    );
    put(
        dir.path(),
        "specs/001-space feature/tasks.md",
        "## Phase 1: Setup\n- [ ] T00001 Create greeting command\n",
    );
    dir
}

#[test]
fn speckit_extracts_long_ids_phase_story_and_byte_provenance() {
    let text = "# 开发任务\r\n\r\n## Phase 2: User Story 1\r\n  - [ ] T123456 [P] [US1] 编写欢迎命令\r\n* [X] T123457 [US1] Add a focused check\r\n";
    let doc = markdown::parse("specs/greeting/tasks.md", text, Framework::Speckit).unwrap();
    assert_eq!(doc.parser_profile, "speckit-v1");
    assert_eq!(doc.source_path, "specs/greeting/tasks.md");
    assert_eq!(doc.headings.len(), 2);
    assert_eq!(doc.headings[1].line, 3);
    assert_eq!(doc.tasks.len(), 2);
    let task = &doc.tasks[0];
    assert_eq!(task.id, "T123456");
    assert_eq!(task.phase, "Phase 2: User Story 1");
    assert_eq!(task.story.as_deref(), Some("US1"));
    assert!(task.parallel);
    assert!(!task.done);
    assert_eq!(task.line, 4);
    assert_eq!(task.source_hash, doc.source_hash);
    assert_eq!(&text[task.checkbox_byte - 1..task.checkbox_byte + 2], "[ ]");
    assert!(doc.tasks[1].done);
    assert!(doc.diagnostics.is_empty());
}

#[test]
fn openspec_id_survives_reordering_but_document_revision_changes() {
    let before = markdown::parse(
        "tasks.md",
        "- [ ] 1.1 Create command\n- [ ] 1.2 Test command\n",
        Framework::Openspec,
    )
    .unwrap();
    let after = markdown::parse(
        "tasks.md",
        "## Moved tasks\n- [ ] 1.2 Test command\n- [ ] 1.1 Create command\n",
        Framework::Openspec,
    )
    .unwrap();
    assert_eq!(before.tasks[0].id, after.tasks[1].id);
    assert_eq!(before.tasks[0].text_hash, after.tasks[1].text_hash);
    assert_ne!(before.source_hash, after.source_hash);
    assert_ne!(before.tasks[0].line, after.tasks[1].line);
}

#[test]
fn provider_profiles_distinguish_hidden_checkbox_semantics() {
    let text = "```md\n- [ ] T900 Example\n```\n<!--\n- [ ] T901 Comment\n-->\n## Phase 1\n- [ ] T001 Real work\n";
    let kit = markdown::parse("tasks.md", text, Framework::Speckit).unwrap();
    assert_eq!(
        kit.tasks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        ["T001"]
    );
    assert!(kit.diagnostics.is_empty());
    let os = markdown::parse("tasks.md", text, Framework::Openspec).unwrap();
    assert_eq!(os.tasks.len(), 3);
    assert_eq!(
        os.diagnostics
            .iter()
            .filter(|d| d.starts_with("upstream_counts_hidden_checkbox:"))
            .count(),
        2
    );
}

#[test]
fn shorter_fence_does_not_expose_an_example_task() {
    let text = "````markdown\n```rust\n- [ ] T998 Example inside nested triple fence\n```\n- [ ] T999 Example still inside four-backtick fence\n````\n- [ ] T001 Real work\n";
    let doc = markdown::parse("tasks.md", text, Framework::Speckit).unwrap();
    assert_eq!(
        doc.tasks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        ["T001"]
    );
}

#[test]
fn inline_comment_does_not_hide_the_task_before_it() {
    let doc = markdown::parse(
        "tasks.md",
        "- [ ] T001 Add command <!-- implementation note -->\n",
        Framework::Speckit,
    )
    .unwrap();
    assert_eq!(doc.tasks.len(), 1);
    assert_eq!(doc.tasks[0].id, "T001");
}

#[test]
fn yaml_and_toml_frontmatter_preserve_unknown_nested_fields() {
    for (text, title) in [
        (
            "---\ntitle: YAML roadmap\ncustom:\n  priority: 7\n---\n# Work\n- [ ] T001 Do work\n",
            "YAML roadmap",
        ),
        (
            "+++\ntitle = 'TOML roadmap'\n[custom]\npriority = 7\n+++\n# Work\n- [ ] T001 Do work\n",
            "TOML roadmap",
        ),
    ] {
        let doc = markdown::parse("tasks.md", text, Framework::Speckit).unwrap();
        assert_eq!(doc.frontmatter["title"], title);
        assert_eq!(doc.frontmatter["custom"]["priority"], 7);
        assert_eq!(doc.tasks.len(), 1);
        let json = serde_json::to_value(&doc).unwrap();
        let restored: markdown::Document = serde_json::from_value(json).unwrap();
        assert_eq!(restored.source_hash, doc.source_hash);
        assert_eq!(restored.frontmatter, doc.frontmatter);
    }
}

#[test]
fn malformed_or_unclosed_frontmatter_is_not_silently_discarded() {
    for text in [
        "---\ntitle: [broken\n---\n",
        "---\ntitle: unfinished\n",
        "+++\ntitle = [broken\n+++\n",
    ] {
        assert!(
            markdown::parse("tasks.md", text, Framework::Speckit).is_err(),
            "accepted {text:?}"
        );
    }
}

#[test]
fn duplicate_source_id_or_description_is_ambiguous() {
    for (framework, text) in [
        (
            Framework::Speckit,
            "- [ ] T001 First\n- [ ] T001 Different task\n",
        ),
        (
            Framework::Openspec,
            "- [ ] 1.1 Same task\n- [x] 1.1 Same task\n",
        ),
    ] {
        let error = markdown::parse("tasks.md", text, framework).unwrap_err();
        assert!(error.to_string().contains("ambiguous_source_task"));
    }
}

#[test]
fn empty_and_unidentified_tasks_produce_actionable_diagnostics() {
    let doc = markdown::parse(
        "tasks.md",
        "- [ ]\n- [ ] Missing native ID\n",
        Framework::Speckit,
    )
    .unwrap();
    assert_eq!(doc.tasks.len(), 1);
    assert!(doc.diagnostics.iter().any(|d| d.starts_with("empty_task:")));
    assert!(
        doc.diagnostics
            .iter()
            .any(|d| d.starts_with("task_id_missing:"))
    );
}

#[test]
fn openspec_whitespace_checkboxes_match_native_progress_contract() {
    let text = "- [\t] 1.1 Tab checkbox\n- [\u{00a0}] 1.2 Unicode space checkbox\n- [X] 1.3 Finished checkbox\n";
    let doc = markdown::parse("tasks.md", text, Framework::Openspec).unwrap();
    assert_eq!(
        doc.tasks.len(),
        3,
        "native OpenSpec counts whitespace inside an empty checkbox"
    );
    assert!(!doc.tasks[0].done);
    assert!(!doc.tasks[1].done);
    assert!(doc.tasks[2].done);
}

#[test]
fn completion_preserves_crlf_unicode_frontmatter_and_unrelated_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = "specs/demo/tasks.md";
    let text = "---\ntitle: 保留标题\ncustom: value\n---\n\n## Phase 1\n  * [ ] T001 [US1] 编写功能\n- [X] T002 已有成果\n".replace('\n', "\r\n");
    put(dir.path(), path, &text);
    let doc = markdown::parse(path, &text, Framework::Speckit).unwrap();
    markdown::complete(
        dir.path(),
        path,
        Framework::Speckit,
        &doc.source_hash,
        &["T001".into()],
    )
    .unwrap();
    assert_eq!(
        fs::read(dir.path().join(path)).unwrap(),
        text.replacen("* [ ]", "* [x]", 1).as_bytes()
    );
}

#[test]
fn completion_of_unicode_empty_checkbox_preserves_utf8() {
    let dir = tempfile::tempdir().unwrap();
    let text = "- [\u{00a0}] 1.1 编写功能\r\n";
    put(dir.path(), "tasks.md", text);
    let doc = markdown::parse("tasks.md", text, Framework::Openspec).unwrap();
    assert_eq!(doc.tasks.len(), 1);
    markdown::complete(
        dir.path(),
        "tasks.md",
        Framework::Openspec,
        &doc.source_hash,
        &[doc.tasks[0].id.clone()],
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("tasks.md")).unwrap(),
        "- [x] 1.1 编写功能\r\n"
    );
}

#[test]
fn stale_or_missing_source_identity_never_partially_writes() {
    let dir = tempfile::tempdir().unwrap();
    let original = "- [ ] T001 First\n- [ ] T002 Second\n";
    put(dir.path(), "tasks.md", original);
    let doc = markdown::parse("tasks.md", original, Framework::Speckit).unwrap();
    let missing = markdown::complete(
        dir.path(),
        "tasks.md",
        Framework::Speckit,
        &doc.source_hash,
        &["T001".into(), "T404".into()],
    )
    .unwrap_err();
    assert!(missing.to_string().contains("source_drift"));
    assert_eq!(
        fs::read_to_string(dir.path().join("tasks.md")).unwrap(),
        original
    );
    let edited = format!("{original}\nA human added this note.\n");
    put(dir.path(), "tasks.md", &edited);
    let stale = markdown::complete(
        dir.path(),
        "tasks.md",
        Framework::Speckit,
        &doc.source_hash,
        &["T001".into()],
    )
    .unwrap_err();
    assert!(stale.to_string().contains("source_drift"));
    assert_eq!(
        fs::read_to_string(dir.path().join("tasks.md")).unwrap(),
        edited
    );
}

#[test]
fn paths_reject_traversal_platform_aliases_and_control_characters() {
    let dir = tempfile::tempdir().unwrap();
    for path in [
        "../tasks.md",
        "/tmp/tasks.md",
        "C:/tasks.md",
        "C:\\tasks.md",
        "tasks\n.md",
        "tasks\0.md",
        "",
    ] {
        assert!(
            paths::inside(dir.path(), path).is_err(),
            "accepted {path:?}"
        );
    }
    assert!(paths::inside(dir.path(), "specs/with spaces/tasks.md").is_ok());
    assert!(
        provider::inspect(
            dir.path(),
            Framework::Openspec,
            "../another-change",
            &Config::default()
        )
        .is_err()
    );
    assert!(provider::select_feature(dir.path(), Some("../another-feature")).is_err());
}

#[cfg(unix)]
#[test]
fn source_symlinks_cannot_escape_snapshot_or_writeback() {
    use std::os::unix::fs::symlink;
    let dir = feature();
    let outside = tempfile::tempdir().unwrap();
    put(outside.path(), "secret.md", "- [ ] T001 Outside data\n");
    symlink(
        outside.path().join("secret.md"),
        dir.path().join("specs/001-space feature/linked.md"),
    )
    .unwrap();
    assert!(
        provider::inspect(
            dir.path(),
            Framework::Speckit,
            "specs/001-space feature",
            &Config::default()
        )
        .unwrap_err()
        .to_string()
        .contains("path_outside_scope")
    );
    let text = fs::read_to_string(outside.path().join("secret.md")).unwrap();
    let doc = markdown::parse(
        "specs/001-space feature/linked.md",
        &text,
        Framework::Speckit,
    )
    .unwrap();
    assert!(
        markdown::complete(
            dir.path(),
            &doc.source_path,
            Framework::Speckit,
            &doc.source_hash,
            &["T001".into()]
        )
        .is_err()
    );
    assert_eq!(
        fs::read_to_string(outside.path().join("secret.md")).unwrap(),
        text
    );
}

#[test]
fn speckit_snapshot_is_read_only_and_revision_covers_constraints() {
    let dir = feature();
    let selector = "specs/001-space feature";
    let before = fs::read(dir.path().join(selector).join("tasks.md")).unwrap();
    let snapshot =
        provider::inspect(dir.path(), Framework::Speckit, selector, &Config::default()).unwrap();
    assert!(snapshot.planning_ready);
    assert_eq!(snapshot.next_action.kind, "implement");
    assert_eq!(
        snapshot.tracking_file.as_deref(),
        Some("specs/001-space feature/tasks.md")
    );
    assert_eq!(snapshot.tasks[0].id, "T00001");
    assert!(
        snapshot
            .context_files
            .iter()
            .any(|c| c.path == ".specify/memory/constitution.md")
    );
    assert!(
        snapshot
            .context_files
            .iter()
            .all(|c| !Path::new(&c.path).is_absolute())
    );
    assert_eq!(
        fs::read(dir.path().join(selector).join("tasks.md")).unwrap(),
        before
    );
    assert!(!dir.path().join(".specify/feature.json").exists());
    put(
        dir.path(),
        ".specify/memory/constitution.md",
        "# Constitution\nA changed mandatory constraint.\n",
    );
    let changed =
        provider::inspect(dir.path(), Framework::Speckit, selector, &Config::default()).unwrap();
    assert_ne!(snapshot.source_hash, changed.source_hash);
    assert_eq!(snapshot.tasks[0].id, changed.tasks[0].id);
}

#[test]
fn incomplete_checklist_blocks_execution_without_marking_it_complete() {
    let dir = feature();
    let checklist = "specs/001-space feature/checklists/requirements.md";
    put(
        dir.path(),
        checklist,
        "# Quality gate\n- [ ] Review scope\n",
    );
    let snapshot = provider::inspect(
        dir.path(),
        Framework::Speckit,
        "specs/001-space feature",
        &Config::default(),
    )
    .unwrap();
    assert_eq!(snapshot.next_action.kind, "blocked");
    assert!(
        snapshot
            .diagnostics
            .iter()
            .any(|d| d.contains("checklist_incomplete"))
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(checklist)).unwrap(),
        "# Quality gate\n- [ ] Review scope\n"
    );
}

#[test]
fn missing_artifact_uses_installed_native_skill_without_creating_it() {
    let dir = feature();
    let selector = "specs/001-space feature";
    fs::remove_file(dir.path().join(selector).join("plan.md")).unwrap();
    let blocked =
        provider::inspect(dir.path(), Framework::Speckit, selector, &Config::default()).unwrap();
    assert!(!blocked.planning_ready);
    assert_eq!(blocked.next_action.kind, "blocked");
    assert!(
        blocked
            .next_action
            .instruction
            .contains("native_bridge_unavailable")
    );
    put(
        dir.path(),
        ".agents/skills/speckit-plan/SKILL.md",
        "# Native planning\nFollow the existing project architecture.\n",
    );
    let ready =
        provider::inspect(dir.path(), Framework::Speckit, selector, &Config::default()).unwrap();
    assert_eq!(ready.next_action.kind, "planning");
    assert_eq!(ready.next_action.artifact, "plan");
    assert!(
        ready
            .next_action
            .instruction
            .contains("Follow the existing project architecture.")
    );
    assert!(!dir.path().join(selector).join("plan.md").exists());
}

#[test]
fn context_budget_is_enforced_before_worker_dispatch() {
    let dir = feature();
    let mut config = Config::default();
    config.execution.max_context_bytes = 40;
    let error = provider::inspect(
        dir.path(),
        Framework::Speckit,
        "specs/001-space feature",
        &config,
    )
    .unwrap_err();
    assert!(error.to_string().contains("context_too_large"));
}

#[cfg(unix)]
fn fixed_cli(dir: &Path, output: &str, code: i32) -> Config {
    let script = dir.join("provider fixture.sh");
    fs::write(&script, format!("cat \"$0.json\"\nexit {code}\n")).unwrap();
    fs::write(dir.join("provider fixture.sh.json"), output).unwrap();
    let mut config = Config::default();
    config.provider.openspec_command =
        vec!["/bin/sh".into(), script.to_string_lossy().into_owned()];
    config
}

#[cfg(unix)]
#[test]
fn openspec_protocol_rejects_non_json_and_nonzero_structured_failures() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixed_cli(dir.path(), "not a JSON document", 0);
    let error = provider::openspec(dir.path(), &config, &["list", "--json"]).unwrap_err();
    assert!(error.to_string().contains("provider_protocol_error"));
    let config = fixed_cli(dir.path(), r#"{"status":[{"code":"no_openspec_root"}]}"#, 1);
    let error = provider::openspec(dir.path(), &config, &["list", "--json"]).unwrap_err();
    assert!(error.to_string().contains("provider_failed"));
    assert!(error.to_string().contains("no_openspec_root"));
}

#[cfg(unix)]
#[test]
fn openspec_external_root_is_not_reinterpreted_as_local() {
    let dir = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let output = serde_json::json!({"changes": [], "root": {"path": external.path().canonicalize().unwrap(), "source": "store"}}).to_string();
    let config = fixed_cli(dir.path(), &output, 0);
    let error = provider::openspec(dir.path(), &config, &["list", "--json"]).unwrap_err();
    assert!(error.to_string().contains("external_spec_root_unsupported"));
}

#[cfg(unix)]
#[test]
fn openspec_unknown_apply_state_is_a_protocol_failure_not_permission_to_execute() {
    let dir = tempfile::tempdir().unwrap();
    put(
        dir.path(),
        "openspec/changes/contract/tasks.md",
        "- [ ] 1.1 Implement command\n",
    );
    let root = dir.path().canonicalize().unwrap();
    // A fixture CLI emits the same response for each call. The payload contains
    // the valid fields those commands need but an unrecognized apply state.
    let payload = serde_json::json!({
        "root": {"path": root, "source": "nearest"},
        "artifacts": [{"id": "tasks", "status": "done"}],
        "contextFiles": {"tasks": [root.join("openspec/changes/contract/tasks.md")]},
        "tasks": [{"id": "1", "description": "1.1 Implement command", "done": false}],
        "progress": {"total": 1, "complete": 0, "remaining": 1},
        "state": "new-upstream-state"
    });
    let config = fixed_cli(dir.path(), &payload.to_string(), 0);
    assert!(
        provider::inspect(dir.path(), Framework::Openspec, "contract", &config).is_err(),
        "unknown upstream state must not authorize implementation"
    );
}

#[test]
#[ignore = "requires the installed pinned OpenSpec package and Node; run explicitly after bun install"]
fn real_openspec_plans_then_accepts_skipped_specs_and_custom_tracking_artifact() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    let cli = project.join("node_modules/@fission-ai/openspec/bin/openspec.js");
    assert!(
        cli.is_file(),
        "run bun install to obtain the pinned upstream CLI"
    );
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.provider.openspec_command = vec!["node".into(), cli.to_string_lossy().into_owned()];
    put(
        dir.path(),
        "openspec/config.yaml",
        "schema: local-contract\n",
    );
    put(
        dir.path(),
        "openspec/schemas/local-contract/schema.yaml",
        "name: local-contract\nversion: 1\nartifacts:\n  - id: proposal\n    generates: proposal.md\n    description: Why this change exists\n    template: proposal.md\n    requires: []\n  - id: specs\n    generates: specs/**/*.md\n    description: Specification deltas\n    template: spec.md\n    requires: [proposal]\n  - id: implementation-checklist\n    generates: checklist.md\n    description: Implementation tasks\n    template: tasks.md\n    requires: [proposal, specs]\napply:\n  requires: [implementation-checklist]\n  tracks: checklist.md\n",
    );
    for name in ["proposal", "spec", "tasks"] {
        put(
            dir.path(),
            &format!("openspec/schemas/local-contract/templates/{name}.md"),
            "# Native contract template\n",
        );
    }
    fs::create_dir_all(dir.path().join("openspec/changes")).unwrap();
    let initial = provider::inspect(dir.path(), Framework::Openspec, "contract", &config).unwrap();
    assert_eq!(initial.next_action.kind, "create");
    provider::create_source(dir.path(), Framework::Openspec, "contract", &config).unwrap();
    let planning = provider::inspect(dir.path(), Framework::Openspec, "contract", &config).unwrap();
    assert_eq!(planning.next_action.kind, "planning");
    assert_eq!(planning.next_action.artifact, "proposal");
    put(
        dir.path(),
        "openspec/changes/contract/.openspec.yaml",
        "schema: local-contract\ncreated: 2026-09-11\nskip_specs: true\n",
    );
    put(
        dir.path(),
        "openspec/changes/contract/proposal.md",
        "## Why\nMake the existing developer instructions clearer.\n\n## What Changes\n- Clarify developer documentation.\n\n## Capabilities\n\n### New Capabilities\nNone.\n\n### Modified Capabilities\nNone.\n\n## Impact\nDocumentation only.\n",
    );
    put(
        dir.path(),
        "openspec/changes/contract/checklist.md",
        "## 1. Documentation\n- [ ] 1.1 Clarify developer instructions and verify the document exists\n",
    );
    let snapshot = provider::inspect(dir.path(), Framework::Openspec, "contract", &config).unwrap();
    assert!(snapshot.planning_ready);
    assert_eq!(snapshot.next_action.kind, "implement");
    assert_eq!(
        snapshot.tracking_file.as_deref(),
        Some("openspec/changes/contract/checklist.md")
    );
    assert_eq!(snapshot.tasks.len(), 1);
    assert!(!dir.path().join("openspec/changes/contract/specs").exists());
    let tracked = snapshot.tracking_file.as_deref().unwrap();
    markdown::complete(
        dir.path(),
        tracked,
        Framework::Openspec,
        &snapshot.tasks[0].source_hash,
        &[snapshot.tasks[0].id.clone()],
    )
    .unwrap();
    let done = provider::inspect(dir.path(), Framework::Openspec, "contract", &config).unwrap();
    assert!(done.tasks.iter().all(|task| task.done));
    assert_ne!(snapshot.source_hash, done.source_hash);
}

#[cfg(unix)]
#[test]
fn openspec_task_context_must_match_native_descriptions_and_completion() {
    let dir = tempfile::tempdir().unwrap();
    put(
        dir.path(),
        "openspec/changes/contract/tasks.md",
        "- [ ] 1.1 Implement command\n",
    );
    let root = dir.path().canonicalize().unwrap();
    for (description, done) in [
        ("1.1 Different command", false),
        ("1.1 Implement command", true),
    ] {
        let payload = serde_json::json!({
            "root": {"path": root, "source": "nearest"},
            "artifacts": [{"id": "tasks", "status": "done"}],
            "contextFiles": {"tasks": [root.join("openspec/changes/contract/tasks.md")]},
            "tasks": [{"id":"1", "description":description, "done":done}],
            "progress":{"total":1,"complete":0,"remaining":1}, "state":"ready"
        });
        let config = fixed_cli(dir.path(), &payload.to_string(), 0);
        let error =
            provider::inspect(dir.path(), Framework::Openspec, "contract", &config).unwrap_err();
        assert!(
            error.to_string().contains("tracking_unsupported"),
            "{error}"
        );
    }
}

#[test]
fn native_feature_pointer_is_worker_local_and_preserves_original_unknown_fields() {
    let origin = tempfile::tempdir().unwrap();
    let worker = tempfile::tempdir().unwrap();
    let original = "{\n  \"feature_directory\": \"specs/user-active\", \"custom\": true\n}\n";
    put(origin.path(), ".specify/feature.json", original);
    put(
        worker.path(),
        ".specify/feature.json",
        "{\"feature_directory\":\"/worker/specs/new\"}",
    );
    provider::restore_feature_pointer(origin.path(), worker.path()).unwrap();
    assert_eq!(
        fs::read_to_string(worker.path().join(".specify/feature.json")).unwrap(),
        original
    );
    fs::remove_file(origin.path().join(".specify/feature.json")).unwrap();
    provider::restore_feature_pointer(origin.path(), worker.path()).unwrap();
    assert!(!worker.path().join(".specify/feature.json").exists());
}

#[test]
fn ignored_native_rules_and_scripts_are_copied_without_conventional_secret_stores() {
    let origin = tempfile::tempdir().unwrap();
    let worker = tempfile::tempdir().unwrap();
    for path in [
        ".specify/memory/constitution.md",
        ".specify/scripts/check.py",
        ".specify/init-options.json",
    ] {
        put(origin.path(), path, "native context");
    }
    for path in [
        ".specify/credentials.json",
        ".specify/secrets.toml",
        ".specify/auth.json",
        ".specify/.env.local",
        ".agents/skills/example/tokens.yml",
    ] {
        put(origin.path(), path, "private-secret-value");
    }
    provider::copy_ignored_context(origin.path(), worker.path()).unwrap();
    for path in [
        ".specify/memory/constitution.md",
        ".specify/scripts/check.py",
        ".specify/init-options.json",
    ] {
        assert_eq!(
            fs::read_to_string(worker.path().join(path)).unwrap(),
            "native context"
        );
    }
    for path in [
        ".specify/credentials.json",
        ".specify/secrets.toml",
        ".specify/auth.json",
        ".specify/.env.local",
        ".agents/skills/example/tokens.yml",
    ] {
        assert!(!worker.path().join(path).exists(), "{path}");
    }
}
