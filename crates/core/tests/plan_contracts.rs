//! Planner and scheduler contracts grounded in native source-task snapshots.
use serde_json::json;
use spec_autonomous_core::{Framework, markdown, model::*, plan, provider};
use std::fs;

fn snapshot(framework: Framework, text: &str) -> Snapshot {
    let doc = markdown::parse("specs/sample/tasks.md", text, framework).unwrap();
    Snapshot {
        schema_version: SCHEMA,
        framework,
        selector: "sample".into(),
        source_dir: "specs/sample".into(),
        tracking_file: Some(doc.source_path),
        planning_ready: true,
        next_action: NativeAction {
            kind: "implement".into(),
            artifact: "tasks".into(),
            instruction: "Follow native scope".into(),
            outputs: vec![],
        },
        tasks: doc.tasks,
        context_files: vec![],
        diagnostics: doc.diagnostics,
        source_hash: doc.source_hash,
        metadata: json!({}),
    }
}

fn task(id: &str, source_id: &str, writes: &[&str], depends_on: &[&str]) -> Task {
    Task {
        id: id.into(),
        description: format!("Implement {id} and verify its native requirement"),
        source_ids: vec![source_id.into()],
        depends_on: depends_on.iter().map(|s| (*s).into()).collect(),
        reads: vec![],
        writes: writes.iter().map(|s| (*s).into()).collect(),
        verification: vec![],
    }
}

fn execution(snapshot: &Snapshot) -> Plan {
    let tasks = snapshot
        .tasks
        .iter()
        .filter(|t| !t.done)
        .enumerate()
        .map(|(i, t)| {
            task(
                &format!("work-{i}"),
                &t.id,
                &[&format!("src/task-{i}.rs")],
                &[],
            )
        })
        .collect();
    Plan {
        schema_version: SCHEMA,
        phase_id: "implementation".into(),
        source_hash: snapshot.source_hash.clone(),
        tasks,
        milestone: None,
    }
}

fn roadmap() -> Milestone {
    Milestone {
        schema_version: SCHEMA,
        id: "greeting".into(),
        goal: "Deliver a verified greeting command".into(),
        framework: Framework::Openspec,
        revision: 1,
        phases: vec![
            Phase {
                id: "foundation".into(),
                label: "1".into(),
                title: "Foundation".into(),
                depends_on: vec![],
                workstream: None,
                owner: None,
                external_ids: vec![],
                source: Source {
                    kind: "openspec-change".into(),
                    selector: "foundation-change".into(),
                },
                verification: vec![],
            },
            Phase {
                id: "greeting".into(),
                label: "2".into(),
                title: "Greeting".into(),
                depends_on: vec!["foundation".into()],
                workstream: None,
                owner: None,
                external_ids: vec![],
                source: Source {
                    kind: "openspec-change".into(),
                    selector: "greeting-change".into(),
                },
                verification: vec![],
            },
            Phase {
                id: "delivery".into(),
                label: "3".into(),
                title: "Delivery".into(),
                depends_on: vec!["greeting".into()],
                workstream: None,
                owner: None,
                external_ids: vec![],
                source: Source {
                    kind: "openspec-change".into(),
                    selector: "delivery-change".into(),
                },
                verification: vec![],
            },
        ],
        verification: vec![],
    }
}

#[test]
fn decomposition_covers_every_pending_source_without_reopening_completed_work() {
    let source = snapshot(
        Framework::Openspec,
        "- [x] 0.1 Existing baseline\n- [ ] 1.1 Add command\n- [ ] 1.2 Add tests\n",
    );
    let mut plan = execution(&source);
    plan.tasks.push(task(
        "command-docs",
        &source.tasks[1].id,
        &["docs/command.md"],
        &["work-0"],
    ));
    assert!(plan::validate_plan(&plan, &source).is_ok());
    plan.tasks
        .retain(|task| !task.source_ids.contains(&source.tasks[2].id));
    assert!(
        plan::validate_plan(&plan, &source)
            .unwrap_err()
            .to_string()
            .contains("coverage")
    );
}

#[test]
fn plans_reject_stale_sources_unknown_tasks_and_already_done_sources() {
    let source = snapshot(
        Framework::Openspec,
        "- [x] 0.1 Existing\n- [ ] 1.1 New work\n",
    );
    let base = execution(&source);
    let mut changed = base.clone();
    changed.source_hash = "stale-revision".into();
    assert!(plan::validate_plan(&changed, &source).is_err());
    for id in ["invented-task".to_string(), source.tasks[0].id.clone()] {
        let mut changed = base.clone();
        changed.tasks[0].source_ids = vec![id];
        assert!(
            plan::validate_plan(&changed, &source)
                .unwrap_err()
                .to_string()
                .contains("unknown source")
        );
    }
}

#[test]
fn plans_reject_cycles_missing_dependencies_duplicate_ids_and_empty_work() {
    let source = snapshot(Framework::Openspec, "- [ ] 1.1 First\n- [ ] 1.2 Second\n");
    let base = execution(&source);
    let mut cycle = base.clone();
    cycle.tasks[0].depends_on = vec!["work-1".into()];
    cycle.tasks[1].depends_on = vec!["work-0".into()];
    assert!(
        plan::validate_plan(&cycle, &source)
            .unwrap_err()
            .to_string()
            .contains("cycle")
    );
    let mut missing = base.clone();
    missing.tasks[0].depends_on = vec!["missing".into()];
    assert!(
        plan::validate_plan(&missing, &source)
            .unwrap_err()
            .to_string()
            .contains("missing dependency")
    );
    let mut duplicate = base.clone();
    duplicate.tasks[1].id = "work-0".into();
    assert!(plan::validate_plan(&duplicate, &source).is_err());
    let mut empty = base.clone();
    empty.tasks.clear();
    assert!(plan::validate_plan(&empty, &source).is_err());
    let mut ungrounded = base;
    ungrounded.tasks[0].source_ids.clear();
    assert!(plan::validate_plan(&ungrounded, &source).is_err());
}

#[test]
fn plans_reject_unsafe_scopes_and_verification_directories() {
    let source = snapshot(Framework::Openspec, "- [ ] 1.1 Work\n");
    for path in [
        "../outside",
        "/absolute",
        "C:/drive",
        ".git/config",
        "./.git/config",
        ".spec-autonomous/state.db",
        "./.spec-autonomous/state.db",
        ".spec-autonomous",
    ] {
        let mut plan = execution(&source);
        plan.tasks[0].writes = vec![path.into()];
        assert!(
            plan::validate_plan(&plan, &source).is_err(),
            "accepted protected scope {path:?}"
        );
    }
    let mut plan = execution(&source);
    plan.tasks[0].verification.push(Check {
        argv: vec!["cargo".into(), "test".into()],
        cwd: "../outside".into(),
    });
    assert!(plan::validate_plan(&plan, &source).is_err());
}

#[test]
fn malformed_glob_and_command_are_rejected_before_dispatch() {
    let source = snapshot(Framework::Openspec, "- [ ] 1.1 Work\n");
    let mut plan = execution(&source);
    plan.tasks[0].writes = vec!["src/[".into()];
    assert!(plan::validate_plan(&plan, &source).is_err());
    let mut plan = execution(&source);
    plan.tasks[0].verification.push(Check {
        argv: vec![],
        cwd: ".".into(),
    });
    assert!(plan::validate_plan(&plan, &source).is_err());
}

#[test]
fn speckit_native_order_and_stage_barriers_are_binding() {
    let source = snapshot(
        Framework::Speckit,
        "## Phase 1: Setup\n- [ ] T001 Setup\n## Phase 2: User Story\n- [ ] T002 [P] Implement first part\n- [ ] T003 [P] Implement second part\n## Phase 3: Polish\n- [ ] T004 Polish\n",
    );
    let mut plan = execution(&source);
    assert!(
        plan::validate_plan(&plan, &source)
            .unwrap_err()
            .to_string()
            .contains("native order")
    );
    plan.tasks[1].depends_on = vec!["work-0".into()];
    plan.tasks[2].depends_on = vec!["work-0".into()];
    plan.tasks[3].depends_on = vec!["work-1".into(), "work-2".into()];
    assert!(plan::validate_plan(&plan, &source).is_ok());
    plan.tasks[3].depends_on = vec!["work-1".into()];
    assert!(
        plan::validate_plan(&plan, &source).is_err(),
        "polish must wait for both story tasks"
    );
}

#[test]
fn parallel_markers_do_not_remove_dependencies_across_phases() {
    let source = snapshot(
        Framework::Speckit,
        "## Phase 1\n- [ ] T001 [P] Foundation\n## Phase 2\n- [ ] T002 [P] Feature\n",
    );
    let mut plan = execution(&source);
    assert!(plan::validate_plan(&plan, &source).is_err());
    plan.tasks[1].depends_on = vec!["work-0".into()];
    assert!(plan::validate_plan(&plan, &source).is_ok());
}

#[test]
fn dependencies_cover_all_decomposed_native_predecessors() {
    let source = snapshot(
        Framework::Speckit,
        "## Phase 1\n- [ ] T001 Foundation\n- [ ] T002 Feature\n",
    );
    let mut plan = execution(&source);
    plan.tasks
        .insert(1, task("foundation-extra", "T001", &["src/extra.rs"], &[]));
    plan.tasks[2].depends_on = vec!["work-0".into()];
    assert!(plan::validate_plan(&plan, &source).is_err());
    plan.tasks[2].depends_on.push("foundation-extra".into());
    assert!(plan::validate_plan(&plan, &source).is_ok());
}

#[test]
fn disjoint_writers_can_share_read_only_inputs() {
    let mut a = task("a", "source-a", &["src/a.rs"], &[]);
    let mut b = task("b", "source-b", &["src/b.rs"], &[]);
    a.reads = vec!["Cargo.toml".into()];
    b.reads = vec!["Cargo.toml".into()];
    assert!(!plan::conflicts(&a, &b));
    b.reads.push("src/a.rs".into());
    assert!(plan::conflicts(&a, &b));
    assert!(plan::conflicts(&b, &a));
}

#[test]
fn unknown_writes_and_ancestor_scopes_serialize() {
    let known = task("a", "source-a", &["src/a.rs"], &[]);
    assert!(plan::conflicts(
        &known,
        &task("unknown", "source-b", &[], &[])
    ));
    assert!(plan::conflicts(
        &known,
        &task("parent", "source-c", &["src"], &[])
    ));
    assert!(plan::conflicts(
        &known,
        &task("glob", "source-d", &["src/**"], &[])
    ));
    assert!(!plan::conflicts(
        &known,
        &task("sibling", "source-e", &["src-other/a.rs"], &[])
    ));
}

#[test]
fn overlapping_filename_globs_conflict_with_concrete_files() {
    let concrete = task("a", "source-a", &["src/foo.rs"], &[]);
    for scope in [
        "src/f*.rs",
        "src/f??.rs",
        "src/{foo,bar}.rs",
        "src/[a-z]*.rs",
    ] {
        let glob = task("glob", "source-b", &[scope], &[]);
        assert!(
            plan::conflicts(&concrete, &glob),
            "missed overlap with {scope}"
        );
        assert!(
            plan::conflicts(&glob, &concrete),
            "asymmetric overlap with {scope}"
        );
    }
}

#[test]
fn equivalent_relative_scopes_cannot_run_as_independent_writers() {
    let canonical = task("a", "source-a", &["src/shared.rs"], &[]);
    for scope in ["./src/shared.rs", "src/./shared.rs", "src//shared.rs"] {
        assert!(
            plan::conflicts(&canonical, &task("b", "source-b", &[scope], &[])),
            "missed alias {scope}"
        );
    }
}

#[test]
fn write_allowlist_distinguishes_directory_boundaries_and_patterns() {
    let scopes = vec!["src/domain".into(), "tests/*.rs".into()];
    assert!(plan::allowed("src/domain/user.rs", &scopes));
    assert!(plan::allowed("tests/user.rs", &scopes));
    assert!(!plan::allowed("src/domain-escape/user.rs", &scopes));
    assert!(!plan::allowed("Cargo.toml", &scopes));
}

#[test]
fn ready_queue_respects_integrated_dependencies_capacity_and_running_writes() {
    let source = snapshot(
        Framework::Openspec,
        "- [ ] 1.1 First\n- [ ] 1.2 Second\n- [ ] 1.3 Dependent\n",
    );
    let mut plan = execution(&source);
    plan.tasks[2].depends_on = vec!["work-0".into(), "work-1".into()];
    let ids = |tasks: Vec<&Task>| tasks.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(plan::ready(&plan, &[], &[], 2)), ["work-0", "work-1"]);
    assert_eq!(
        ids(plan::ready(&plan, &["work-0".into()], &[&plan.tasks[1]], 3)),
        Vec::<String>::new()
    );
    assert_eq!(
        ids(plan::ready(
            &plan,
            &["work-0".into(), "work-1".into()],
            &[],
            3
        )),
        ["work-2"]
    );
    assert!(plan::ready(&plan, &[], &[], 0).is_empty());
    assert!(plan::ready(&plan, &[], &[&plan.tasks[0]], 1).is_empty());
    plan.tasks[1].writes = plan.tasks[0].writes.clone();
    assert!(plan::ready(&plan, &[], &[&plan.tasks[0]], 3).is_empty());
}

#[test]
fn range_is_inclusive_and_outside_prerequisites_require_evidence() {
    let m = roadmap();
    let range = Range {
        from: Some("2".into()),
        to: Some("delivery".into()),
        only: None,
    };
    assert!(range.bounded());
    assert!(
        plan::select(&m, &range, &[])
            .unwrap_err()
            .to_string()
            .contains("prerequisite_outside_range")
    );
    assert_eq!(
        plan::select(&m, &range, &["foundation".into()]).unwrap(),
        ["greeting", "delivery"]
    );
    assert_eq!(
        plan::select(&m, &Range::default(), &[]).unwrap(),
        ["foundation", "greeting", "delivery"]
    );
}

#[test]
fn only_selects_exactly_one_stable_phase_id_or_label() {
    let m = roadmap();
    for key in ["greeting", "2"] {
        let range = Range {
            only: Some(key.into()),
            ..Range::default()
        };
        assert_eq!(
            plan::select(&m, &range, &["foundation".into()]).unwrap(),
            ["greeting"]
        );
    }
}

#[test]
fn ranges_reject_conflicting_unknown_and_reverse_endpoints() {
    let m = roadmap();
    for range in [
        Range {
            only: Some("1".into()),
            from: Some("1".into()),
            to: None,
        },
        Range {
            only: None,
            from: Some("3".into()),
            to: Some("1".into()),
        },
        Range {
            only: Some("missing".into()),
            ..Range::default()
        },
    ] {
        assert!(plan::select(&m, &range, &[]).is_err());
    }
}

#[test]
fn stable_labels_survive_insertion_without_becoming_array_indexes() {
    let mut m = roadmap();
    let mut inserted = m.phases[0].clone();
    inserted.id = "interlude".into();
    inserted.label = "1.5".into();
    inserted.source.selector = "interlude-change".into();
    m.phases.insert(1, inserted);
    m.revision += 1;
    let range = Range {
        from: Some("2".into()),
        to: Some("3".into()),
        only: None,
    };
    assert_eq!(
        plan::select(&m, &range, &["foundation".into()]).unwrap(),
        ["greeting", "delivery"]
    );
}

#[test]
fn milestone_rejects_cycles_duplicates_unsafe_sources_and_provider_mismatch() {
    let base = roadmap();
    let mut cyclic = base.clone();
    cyclic.phases[0].depends_on = vec!["delivery".into()];
    assert!(plan::validate_milestone(&cyclic).is_err());
    let mut missing = base.clone();
    missing.phases[0].depends_on = vec!["missing".into()];
    assert!(plan::validate_milestone(&missing).is_err());
    let mut duplicate = base.clone();
    duplicate.phases[1].label = "1".into();
    assert!(plan::validate_milestone(&duplicate).is_err());
    let mut duplicate = base.clone();
    duplicate.phases[1].source = duplicate.phases[0].source.clone();
    assert!(plan::validate_milestone(&duplicate).is_err());
    let mut unsafe_source = base.clone();
    unsafe_source.phases[0].source.selector = "../outside".into();
    assert!(plan::validate_milestone(&unsafe_source).is_err());
    let mut mismatch = base;
    mismatch.phases[0].source.kind = "speckit-feature".into();
    assert!(plan::validate_milestone(&mismatch).is_err());
}

#[test]
fn ambiguous_id_label_aliases_are_never_guessed() {
    let mut m = roadmap();
    m.phases[1].label = "foundation".into();
    let range = Range {
        only: Some("foundation".into()),
        ..Range::default()
    };
    assert!(plan::select(&m, &range, &[]).is_err());
}

#[test]
fn milestone_json_and_toml_preserve_semantics_and_allow_unknown_metadata() {
    let original = roadmap();
    let mut value = serde_json::to_value(&original).unwrap();
    value["future_metadata"] = json!({"review": "recorded elsewhere"});
    let decoded: Milestone = serde_json::from_value(value).unwrap();
    assert!(plan::validate_milestone(&decoded).is_ok());
    let toml = toml::to_string_pretty(&decoded).unwrap();
    let restored: Milestone = toml::from_str(&toml).unwrap();
    assert_eq!(
        serde_json::to_value(original).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
    let mut missing = serde_json::to_value(decoded).unwrap();
    missing.as_object_mut().unwrap().remove("goal");
    assert!(serde_json::from_value::<Milestone>(missing).is_err());
}

#[test]
fn roadmap_regeneration_preserves_human_modified_view_and_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = roadmap();
    provider::save_milestone(dir.path(), &m).unwrap();
    let path = provider::milestone_path(dir.path(), &m.id).unwrap();
    let manifest_before = fs::read(&path).unwrap();
    let view = path.with_file_name("ROADMAP.md");
    let edited = format!("{}\nA human note.\n", fs::read_to_string(&view).unwrap());
    fs::write(&view, &edited).unwrap();
    m.goal = "A newer proposed goal".into();
    assert!(
        provider::save_milestone(dir.path(), &m)
            .unwrap_err()
            .to_string()
            .contains("roadmap_view_modified")
    );
    assert_eq!(fs::read(&path).unwrap(), manifest_before);
    assert_eq!(fs::read_to_string(view).unwrap(), edited);
    assert_eq!(
        provider::load_milestone(dir.path(), &m.id).unwrap().goal,
        "Deliver a verified greeting command"
    );
}
