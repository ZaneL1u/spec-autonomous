//! Preflight classifies configuration readiness without running unbuilt tests.
use spec_autonomous_core::{
    model::*,
    verification_preflight::{self as preflight, Report},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn node(root: &Path) -> PathBuf {
    let path = root.join(if cfg!(windows) { "node.exe" } else { "node" });
    // A non-runnable payload with executable permissions proves no process probe
    // or verification command is launched by filesystem readiness inspection.
    fs::write(&path, "PREFLIGHT MUST NOT EXECUTE THIS FILE").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}
fn check(program: &Path, args: &[&str], cwd: &str) -> Check {
    Check {
        argv: std::iter::once(program.to_string_lossy().into_owned())
            .chain(args.iter().map(|a| (*a).into()))
            .collect(),
        cwd: cwd.into(),
    }
}
fn has(report: &Report, code: &str) -> bool {
    report.diagnostics.iter().any(|d| d.code == code)
}

#[test]
fn node_directory_is_portability_warning_not_a_claim_all_versions_fail() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("test")).unwrap();
    let report = preflight::inspect_checks(
        root.path(),
        &[check(&node(root.path()), &["--test", "test/"], ".")],
        &[],
    );
    assert!(report.ready, "directory support depends on the runtime");
    assert!(!report.executed);
    assert!(has(&report, "node_test_directory_operand"));
    assert_eq!(report.diagnostics[0].severity, "warning");
    assert!(report.diagnostics[0].message.contains("run.revise"));
}

#[test]
fn missing_future_test_files_are_deferred_and_not_passed() {
    let root = tempfile::tempdir().unwrap();
    let report = preflight::inspect_checks(
        root.path(),
        &[check(
            &node(root.path()),
            &["--test", "tests/count.test.mjs"],
            ".",
        )],
        &["tests/**".into()],
    );
    assert!(report.ready);
    assert_eq!(report.status, "deferred");
    assert!(!report.executed);
    assert!(has(&report, "check_target_deferred"));
    assert!(!root.path().join("tests").exists());
}

#[test]
fn undeclared_missing_test_target_needs_input() {
    let root = tempfile::tempdir().unwrap();
    let report = preflight::inspect_checks(
        root.path(),
        &[check(
            &node(root.path()),
            &["--test", "missing.test.mjs"],
            ".",
        )],
        &["src/**".into()],
    );
    assert!(!report.ready);
    assert_eq!(report.status, "needs_input");
    assert!(has(&report, "check_target_missing"));
}

#[test]
fn valid_discovery_and_explicit_files_do_not_trigger_directory_warning() {
    let root = tempfile::tempdir().unwrap();
    let node = node(root.path());
    fs::write(root.path().join("count.test.mjs"), "not evaluated").unwrap();
    for args in [
        vec!["--test"],
        vec!["--test", "count.test.mjs"],
        vec!["--test", "--test-name-pattern", "test/", "count.test.mjs"],
        vec!["--test", "--test-name-pattern=test/", "count.test.mjs"],
        vec!["--test", "tests/*.test.mjs"],
    ] {
        let report = preflight::inspect_checks(root.path(), &[check(&node, &args, ".")], &[]);
        assert!(report.ready, "{args:?}: {report:?}");
        assert!(!has(&report, "node_test_directory_operand"), "{args:?}");
    }
}

#[test]
fn non_node_and_inline_javascript_are_not_treated_as_node_test_paths() {
    let root = tempfile::tempdir().unwrap();
    let node = node(root.path());
    let other = root.path().join("another-runner");
    fs::copy(&node, &other).unwrap();
    for command in [
        check(&other, &["--test", "missing/"], "."),
        check(&node, &["--test", "-e", "test/"], "."),
        check(&node, &["--test", "--unknown-runtime-option", "test/"], "."),
    ] {
        let report = preflight::inspect_checks(root.path(), &[command], &[]);
        assert!(!has(&report, "node_test_directory_operand"));
        assert!(!has(&report, "check_target_missing"));
    }
}

#[test]
fn cwd_and_executable_readiness_distinguishes_future_project_outputs() {
    let root = tempfile::tempdir().unwrap();
    let node = node(root.path());
    let future = preflight::inspect_checks(
        root.path(),
        &[check(
            &node,
            &["--test", "count.test.mjs"],
            "packages/count",
        )],
        &["packages/count/**".into()],
    );
    assert!(future.ready);
    assert!(has(&future, "check_cwd_deferred"));
    assert!(has(&future, "check_target_deferred"));
    let unknown = preflight::inspect_checks(root.path(), &[check(&node, &["--test"], "typo")], &[]);
    assert!(!unknown.ready);
    assert!(has(&unknown, "check_cwd_missing"));
    let executable = preflight::inspect_checks(
        root.path(),
        &[check(Path::new("./build/verify"), &[], ".")],
        &["build/**".into()],
    );
    assert!(executable.ready);
    assert!(has(&executable, "check_executable_deferred"));
    let missing = preflight::inspect_checks(
        root.path(),
        &[check(
            Path::new("sa-nonexistent-verifier-1178973"),
            &[],
            ".",
        )],
        &[],
    );
    assert!(!missing.ready);
    assert!(has(&missing, "check_executable_missing"));
}

#[test]
fn plans_defer_dependency_outputs_but_not_unrelated_task_outputs() {
    let root = tempfile::tempdir().unwrap();
    let node = node(root.path());
    let make = Task {
        id: "make".into(),
        description: "implementation and tests".into(),
        source_ids: vec!["source-1".into()],
        depends_on: vec![],
        reads: vec![],
        writes: vec!["count.test.mjs".into()],
        verification: vec![],
    };
    let verify = Task {
        id: "verify".into(),
        description: "independent native acceptance".into(),
        source_ids: vec!["source-2".into()],
        depends_on: vec!["make".into()],
        reads: vec!["count.test.mjs".into()],
        writes: vec![],
        verification: vec![check(&node, &["--test", "count.test.mjs"], ".")],
    };
    let mut plan = Plan {
        schema_version: 1,
        phase_id: "phase".into(),
        source_hash: "source".into(),
        tasks: vec![make, verify],
        milestone: None,
    };
    let report = preflight::inspect_plan(root.path(), &plan, &[]);
    assert!(report.ready);
    assert!(has(&report, "empty_write_scope"));
    assert!(has(&report, "check_target_deferred"));
    plan.tasks[1].depends_on.clear();
    assert!(!preflight::inspect_plan(root.path(), &plan, &[]).ready);
}

#[test]
fn roadmaps_defer_unbuilt_paths_before_task_write_sets_exist() {
    let root = tempfile::tempdir().unwrap();
    let milestone = Milestone {
        schema_version: 1,
        id: "m".into(),
        goal: "build".into(),
        framework: spec_autonomous_core::Framework::Openspec,
        revision: 1,
        phases: vec![],
        verification: vec![check(
            &node(root.path()),
            &["--test", "future.test.mjs"],
            "future",
        )],
    };
    let report = preflight::inspect_milestone(root.path(), &milestone);
    assert!(report.ready);
    assert_eq!(report.status, "deferred");
    assert!(!report.executed);
}

#[test]
fn planning_guidance_preserves_native_coverage_and_discourages_fragments() {
    let guidance = preflight::planning_guidance();
    assert!(guidance.contains("multiple source_ids"));
    assert!(guidance.contains("phase barriers intact"));
    assert!(guidance.contains("small function"));
    assert!(guidance.contains("check cwd"));
    assert!(guidance.contains("not a portable test command"));
}

#[test]
fn configured_path_resolves_verifiers_without_mutating_process_environment() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("private-bin");
    fs::create_dir(&bin).unwrap();
    let fake_node = node(&bin);
    let verifier = bin.join(if cfg!(windows) {
        "sa-private-verifier-773649.exe"
    } else {
        "sa-private-verifier-773649"
    });
    fs::copy(&fake_node, &verifier).unwrap();
    let command = check(Path::new("sa-private-verifier-773649"), &[], ".");
    assert!(!preflight::inspect_checks(root.path(), std::slice::from_ref(&command), &[]).ready);
    let before = std::env::var_os("PATH");
    let overrides = std::collections::BTreeMap::from([
        (
            if cfg!(windows) {
                "Path".into()
            } else {
                "PATH".into()
            },
            bin.to_string_lossy().into_owned(),
        ),
        ("PATHEXT".into(), ".EXE;.CMD".into()),
    ]);
    let report = preflight::inspect_checks_with_environment(
        root.path(),
        std::slice::from_ref(&command),
        &[],
        &overrides,
    );
    assert!(report.ready, "{report:?}");
    assert_eq!(report.checks[0].executable.as_ref(), Some(&verifier));
    assert!(!report.executed);
    assert_eq!(std::env::var_os("PATH"), before);
    assert_eq!(
        preflight::environment_with_overrides(root.path(), &overrides)["executables"]["node"],
        fake_node.to_string_lossy().as_ref()
    );
    let milestone = Milestone {
        schema_version: 1,
        id: "m".into(),
        goal: "build".into(),
        framework: spec_autonomous_core::Framework::Openspec,
        revision: 1,
        phases: vec![],
        verification: vec![command.clone()],
    };
    assert!(
        preflight::inspect_milestone_with_environment(root.path(), &milestone, &overrides).ready
    );
    let execution = Plan {
        schema_version: 1,
        phase_id: "p".into(),
        source_hash: "hash".into(),
        tasks: vec![],
        milestone: None,
    };
    assert!(
        preflight::inspect_plan_with_environment(root.path(), &execution, &[command], &overrides)
            .ready
    );
}

#[cfg(unix)]
#[test]
fn unix_path_overrides_remain_case_sensitive() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("private-bin");
    fs::create_dir(&bin).unwrap();
    fs::rename(node(&bin), bin.join("sa-private-verifier-103979")).unwrap();
    let overrides =
        std::collections::BTreeMap::from([("Path".into(), bin.to_string_lossy().into_owned())]);
    let report = preflight::inspect_checks_with_environment(
        root.path(),
        &[check(Path::new("sa-private-verifier-103979"), &[], ".")],
        &[],
        &overrides,
    );
    assert!(!report.ready);
}
