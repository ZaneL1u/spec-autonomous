use spec_autonomous_core::config::Config;
use std::fs;
use tempfile::tempdir;

#[test]
fn configuration_layers_project_over_user_over_defaults_without_merging_arrays() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(project.join(".spec-autonomous")).unwrap();
    let user = dir.path().join("user.toml");
    fs::write(&user,"[execution]\nmax_workers=4\nmax_attempts=5\n[runner]\ncommand=['old','argument']\nfresh_session=true\n").unwrap();
    fs::write(
        project.join(".spec-autonomous/config.toml"),
        "[execution]\nmode='native'\nmax_workers=2\n[runner]\ncommand=['new']\n",
    )
    .unwrap();
    let config = Config::load_with_user(&project, Some(&user)).unwrap();
    assert_eq!(config.execution.max_workers, 2);
    assert_eq!(config.execution.max_attempts, 5);
    assert_eq!(config.execution.mode, "native");
    assert_eq!(config.runner.command, ["new"]);
    assert!(config.runner.fresh_session);
}
#[test]
fn unknown_budget_fields_do_not_silently_become_unlimited_execution() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join(".spec-autonomous")).unwrap();
    fs::write(
        dir.path().join(".spec-autonomous/config.toml"),
        "[execution]\nmax_tokens=10\n",
    )
    .unwrap();
    assert!(Config::load_with_user(dir.path(), None).is_err());
}
#[test]
fn invalid_config_does_not_echo_secret_values() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join(".spec-autonomous")).unwrap();
    fs::write(
        dir.path().join(".spec-autonomous/config.toml"),
        "[runner]\ncommand='private-secret-value'\n",
    )
    .unwrap();
    let error = Config::load_with_user(dir.path(), None)
        .unwrap_err()
        .to_string();
    assert!(!error.contains("private-secret-value"));
}
#[test]
fn unsupported_lifecycle_and_overflowing_deadlines_are_rejected() {
    let mut config = Config::default();
    config.policy.publish = true;
    assert!(config.validate().is_err());
    config.policy.publish = false;
    config.execution.run_timeout_seconds = u64::MAX;
    assert!(config.validate().is_err());
}

#[test]
fn misspelled_verification_and_hook_fields_cannot_silently_change_execution() {
    for body in [
        "[[verification]]\nargv=['node','test.mjs']\ncwdd='subproject'\n",
        "[hooks.test]\nargv=['node','test.mjs']\nidempotant=true\n",
        "[hooks.test]\nargv=[]\n",
    ] {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join(".spec-autonomous")).unwrap();
        fs::write(dir.path().join(".spec-autonomous/config.toml"), body).unwrap();
        assert!(Config::load_with_user(dir.path(), None).is_err(), "{body}");
    }
}
