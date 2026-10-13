use spec_autonomous_core::{config::Config, git, model::Check, verification::Wave};
use std::{
    fs,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
fn fixture() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    git::command(t.path(), &["init", "-q", "-b", "main"], None).unwrap();
    fs::write(t.path().join("source"), "unchanged").unwrap();
    git::commit(t.path(), "fixture").unwrap();
    t
}
fn check(script: &str) -> Check {
    Check {
        argv: vec!["node".into(), "-e".into(), script.into()],
        cwd: ".".into(),
    }
}
#[test]
fn independent_waves_preserve_reports_and_record_both_streams() {
    let root = fixture();
    let output = tempfile::tempdir().unwrap();
    let config = Config::default();
    for stage in ["worker", "combined"] {
        let wave = Wave::new(output.path().join(stage), root.path(), stage, &config).unwrap();
        let r=wave.execute(&check("const fs=require('fs'),p=require('path');fs.writeFileSync(p.join(process.env.SA_VERIFY_OUTPUT_DIR,'report.json'),JSON.stringify(process.env.SA_VERIFY_STAGE));console.log('out');console.error('err');"),false,&config,Arc::new(AtomicBool::new(false))).unwrap();
        assert!(r.failure.is_none());
        let d = r.evidence.details.unwrap();
        assert_eq!(d["stage"], stage);
        assert_eq!(d["tree_unchanged"], true);
        for k in ["stdout", "stderr", "manifest"] {
            assert!(Path::new(d[k].as_str().unwrap()).is_file());
        }
        assert!(d["stderr_sha256"].as_str().unwrap().len() == 64);
    }
    assert_ne!(
        fs::read(output.path().join("worker/report.json")).unwrap(),
        fs::read(output.path().join("combined/report.json")).unwrap()
    );
    assert!(git::clean(root.path()).unwrap());
}
#[test]
fn failure_timeout_and_missing_executable_have_durable_diagnostics() {
    let root = fixture();
    let output = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.execution.attempt_timeout_seconds = 1;
    for (name, c, kind) in [
        (
            "failure",
            check("console.error('private-output');process.exit(7)"),
            "check_failed",
        ),
        ("timeout", check("setTimeout(()=>{},10000)"), "timeout"),
        (
            "launch",
            Check {
                argv: vec!["sa-nonexistent-test-executable".into()],
                cwd: ".".into(),
            },
            "execution_error",
        ),
    ] {
        let wave = Wave::new(output.path().join(name), root.path(), "worker", &config).unwrap();
        let r = wave
            .execute(&c, false, &config, Arc::new(AtomicBool::new(false)))
            .unwrap();
        assert_eq!(r.failure.as_deref(), Some(kind));
        let details = r.evidence.details.unwrap();
        let manifest = fs::read_to_string(details["manifest"].as_str().unwrap()).unwrap();
        assert!(manifest.contains(kind));
        assert!(!manifest.contains("private-output\\n"));
    }
}
#[test]
fn source_mutation_is_not_a_pass_and_old_evidence_remains_readable() {
    let root = fixture();
    let output = tempfile::tempdir().unwrap();
    let config = Config::default();
    let wave = Wave::new(
        output.path().join("mutation"),
        root.path(),
        "worker",
        &config,
    )
    .unwrap();
    let result = wave
        .execute(
            &check("require('fs').writeFileSync('source','changed')"),
            false,
            &config,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
    assert_eq!(result.failure.as_deref(), Some("source_changed"));
    let mut json = serde_json::to_value(result.evidence).unwrap();
    json.as_object_mut().unwrap().remove("details");
    let old: spec_autonomous_core::model::Evidence = serde_json::from_value(json).unwrap();
    assert!(old.details.is_none());
}
