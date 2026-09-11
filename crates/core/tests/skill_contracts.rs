use spec_autonomous_core::{config::Config, paths, skills};
use std::{collections::BTreeMap, fs, path::Path};
use tempfile::TempDir;

const NAMES: [&str; 5] = ["autonomous", "auto", "milestone", "progress", "resume"];
const MANIFEST: &str = ".spec-autonomous/skills-installed.toml";

fn put(root: &Path, name: &str, body: impl AsRef<[u8]>) {
    let file = root.join(name);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, body).unwrap();
}

fn asset(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/cli/skills")
            .join(name)
            .join("SKILL.md"),
    )
    .unwrap()
}

fn installed_path(agent: &str, name: &str) -> String {
    match agent {
        "codex" => format!(".agents/skills/{name}/SKILL.md"),
        "claude" => format!(".claude/commands/{name}.md"),
        _ => panic!("invalid fixture host"),
    }
}

fn manifest(root: &Path) -> toml::Value {
    toml::from_str(&fs::read_to_string(root.join(MANIFEST)).unwrap()).unwrap()
}

fn save_manifest(root: &Path, value: &toml::Value) {
    put(root, MANIFEST, toml::to_string_pretty(value).unwrap());
}

fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, current: &Path, result: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let key = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                visit(root, &path, result);
            } else if kind.is_symlink() {
                result.insert(
                    key,
                    format!("symlink:{}", fs::read_link(path).unwrap().display()).into_bytes(),
                );
            } else {
                result.insert(key, fs::read(path).unwrap());
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

#[test]
fn install_both_hosts_tracks_every_asset_and_repeated_install_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    for agent in ["codex", "claude"] {
        let installed = skills::install(root, agent, "").unwrap();
        assert_eq!(installed.len(), NAMES.len());
        let state = manifest(root);
        assert_eq!(state["version"].as_str(), Some(env!("CARGO_PKG_VERSION")));
        for name in NAMES {
            let path = installed_path(agent, name);
            assert!(installed.contains(&path));
            let body = fs::read_to_string(root.join(&path)).unwrap();
            assert_eq!(body, asset(name));
            assert_eq!(
                state["files"][&path].as_str(),
                Some(paths::hash(body).as_str())
            );
        }
        let before = files(root);
        assert_eq!(skills::install(root, agent, "").unwrap(), installed);
        assert_eq!(files(root), before);
    }
    assert_eq!(manifest(root)["files"].as_table().unwrap().len(), 10);
    assert_eq!(skills::uninstall(root).unwrap().len(), 10);
    assert!(manifest(root)["files"].as_table().unwrap().is_empty());
}

#[test]
fn namespaced_install_preserves_a_conflicting_user_alias() {
    for agent in ["codex", "claude"] {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let user_alias = installed_path(agent, "auto");
        put(root, &user_alias, "user-owned alias\r\n");
        let before = files(root);
        let error = skills::install(root, agent, "").unwrap_err();
        assert!(error.to_string().contains("skill_conflict"));
        assert_eq!(files(root), before);
        let installed = skills::install(root, agent, "sa").unwrap();
        assert_eq!(installed.len(), 5);
        for name in NAMES {
            let path = installed_path(agent, &format!("sa-{name}"));
            let body = fs::read_to_string(root.join(path)).unwrap();
            assert_eq!(
                body,
                asset(name).replacen(&format!("name: {name}"), &format!("name: sa-{name}"), 1)
            );
        }
        assert_eq!(
            fs::read(root.join(&user_alias)).unwrap(),
            b"user-owned alias\r\n"
        );
        assert_eq!(skills::uninstall(root).unwrap().len(), 5);
        assert_eq!(
            fs::read(root.join(user_alias)).unwrap(),
            b"user-owned alias\r\n"
        );
    }
}

#[test]
fn update_replaces_only_an_unmodified_owned_version() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    skills::install(root, "codex", "").unwrap();
    let path = installed_path("codex", "autonomous");
    let previous_release = "---\nname: autonomous\n---\nPreviously shipped CLI instructions.\n";
    put(root, &path, previous_release);
    let mut state = manifest(root);
    state["version"] = toml::Value::String("0.0.1".into());
    state["files"][&path] = toml::Value::String(paths::hash(previous_release));
    save_manifest(root, &state);

    skills::install(root, "codex", "").unwrap();
    assert_eq!(
        fs::read_to_string(root.join(&path)).unwrap(),
        asset("autonomous")
    );
    assert_eq!(
        manifest(root)["version"].as_str(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        manifest(root)["files"][&path].as_str(),
        Some(paths::hash(asset("autonomous")).as_str())
    );
}

#[test]
fn update_preflights_all_conflicts_without_overwriting_any_asset_or_manifest() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    skills::install(root, "codex", "").unwrap();
    // The last asset conflicts; even the earlier assets and manifest must be preserved.
    put(
        root,
        &installed_path("codex", "resume"),
        "user modified resume\n",
    );
    let before = files(root);
    assert!(
        skills::install(root, "codex", "")
            .unwrap_err()
            .to_string()
            .contains("skill_conflict")
    );
    assert_eq!(files(root), before);
}

#[test]
fn uninstall_preserves_modified_skills_and_native_workflow_files() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    put(
        root,
        ".specify/memory/constitution.md",
        "native constitution\n",
    );
    put(
        root,
        "specs/001-feature/tasks.md",
        "- [ ] T001 Native task\n",
    );
    put(
        root,
        ".agents/skills/speckit-implement/SKILL.md",
        "native implement\n",
    );
    put(root, ".agents/skills/personal/SKILL.md", "personal skill\n");
    put(
        root,
        ".spec-autonomous/config.toml",
        "# user configuration\n",
    );
    let native = files(root);
    skills::install(root, "codex", "").unwrap();
    let modified = installed_path("codex", "auto");
    put(root, &modified, "customized alias\n");

    let removed = skills::uninstall(root).unwrap();
    assert_eq!(removed.len(), 4);
    assert!(!removed.contains(&modified));
    assert_eq!(
        fs::read(root.join(&modified)).unwrap(),
        b"customized alias\n"
    );
    assert_eq!(manifest(root)["files"].as_table().unwrap().len(), 1);
    for (path, bytes) in native {
        assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
    }
    assert!(skills::uninstall(root).unwrap().is_empty());
}

#[test]
fn uninstall_without_manifest_is_a_noop() {
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), ".agents/skills/personal/SKILL.md", "keep\n");
    let before = files(tmp.path());
    assert!(skills::uninstall(tmp.path()).unwrap().is_empty());
    assert_eq!(files(tmp.path()), before);
}

#[test]
fn invalid_host_and_unsafe_prefix_do_not_write_files() {
    let tmp = TempDir::new().unwrap();
    assert!(skills::install(tmp.path(), "unsupported", "").is_err());
    for prefix in ["../escape", "a/b", "a\\b", "a:b", "a\n"] {
        assert!(skills::install(tmp.path(), "codex", prefix).is_err());
    }
    assert!(files(tmp.path()).is_empty());
}

#[test]
fn init_requires_an_existing_unambiguous_provider_before_writing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    assert!(skills::init(root, Some("codex"), "").is_err());
    assert!(files(root).is_empty());
    assert!(!root.join(".spec-autonomous").exists());
    put(root, "openspec/config.yaml", "schema: spec-driven\n");
    put(root, ".specify/feature.json", "{}\n");
    let before = files(root);
    assert!(skills::init(root, Some("codex"), "").is_err());
    assert_eq!(files(root), before);
    assert!(!root.join(".spec-autonomous").exists());
}

#[test]
fn init_requires_host_selection_when_missing_or_ambiguous() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    put(root, ".specify/feature.json", "{}\n");
    let before = files(root);
    assert!(
        skills::init(root, None, "")
            .unwrap_err()
            .to_string()
            .contains("host_selection_required")
    );
    fs::create_dir(root.join(".agents")).unwrap();
    fs::create_dir(root.join(".claude")).unwrap();
    assert!(
        skills::init(root, None, "")
            .unwrap_err()
            .to_string()
            .contains("host_selection_required")
    );
    assert_eq!(files(root), before);
}

#[test]
fn init_detects_native_provider_and_host_and_preserves_existing_configuration() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    put(
        root,
        ".specify/memory/constitution.md",
        "native constitution\r\n",
    );
    put(
        root,
        ".agents/skills/speckit-implement/SKILL.md",
        "native command\n",
    );
    put(root, ".gitignore", "# existing rules\nnode_modules/\n");
    let config =
        "# User configuration must stay byte-for-byte intact.\n[execution]\nmax_workers = 2\n";
    put(root, ".spec-autonomous/config.toml", config);
    let result = skills::init(root, None, "").unwrap();
    assert_eq!(result["framework"], "speckit");
    assert_eq!(result["agent"], "codex");
    assert_eq!(result["entry"], "$autonomous $auto");
    assert_eq!(Config::load(root).unwrap().execution.max_workers, 2);
    assert_eq!(
        fs::read_to_string(root.join(".spec-autonomous/config.toml")).unwrap(),
        config
    );
    assert_eq!(
        fs::read(root.join(".specify/memory/constitution.md")).unwrap(),
        b"native constitution\r\n"
    );
    assert_eq!(
        fs::read(root.join(".agents/skills/speckit-implement/SKILL.md")).unwrap(),
        b"native command\n"
    );
    let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(ignore.starts_with("# existing rules\nnode_modules/\n"));
    assert!(ignore.contains("!.spec-autonomous/milestones/"));
    assert!(ignore.contains("!.spec-autonomous/plans/"));
    let before = files(root);
    skills::init(root, None, "").unwrap();
    assert_eq!(files(root), before);
}

#[test]
fn init_reports_the_actual_namespaced_entry_for_each_host() {
    for (agent, entry) in [
        ("codex", "$sa-autonomous $sa-auto"),
        ("claude", "/sa-autonomous /sa-auto"),
    ] {
        let tmp = TempDir::new().unwrap();
        put(tmp.path(), "openspec/config.yaml", "schema: spec-driven\n");
        let result = skills::init(tmp.path(), Some(agent), "sa").unwrap();
        assert_eq!(result["entry"], entry);
        assert_eq!(result["installed"].as_array().unwrap().len(), 5);
    }
}

#[test]
fn init_rejects_invalid_existing_config_before_creating_any_bindings() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    put(root, ".specify/feature.json", "{}\n");
    put(
        root,
        ".spec-autonomous/config.toml",
        "[execution]\nmax_workers = 0\n",
    );
    put(root, ".gitignore", "# keep existing ignore content\n");
    let before = files(root);
    assert!(
        format!("{:#}", skills::init(root, Some("codex"), "").unwrap_err())
            .contains("invalid_config")
    );
    assert_eq!(files(root), before);
    assert!(!root.join(".agents").exists());
}

#[test]
fn uninstall_preflights_the_entire_manifest_before_deleting_anything() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    skills::install(root, "codex", "").unwrap();
    put(root, "zzz-unrelated.md", "not owned by installer\n");
    let mut state = manifest(root);
    // This sorts after the legitimate skill files and must not cause partial deletion.
    state["files"].as_table_mut().unwrap().insert(
        "zzz-unrelated.md".into(),
        toml::Value::String(paths::hash("not owned by installer\n")),
    );
    save_manifest(root, &state);
    let before = files(root);
    assert!(skills::uninstall(root).is_err());
    assert_eq!(files(root), before);
}

#[cfg(unix)]
#[test]
fn install_rejects_a_symlinked_skill_directory_without_touching_its_target() {
    use std::os::unix::fs::symlink;
    let tmp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join(".agents/skills")).unwrap();
    put(outside.path(), "SKILL.md", "external skill\n");
    symlink(outside.path(), root.join(".agents/skills/resume")).unwrap();
    let before = files(root);
    assert!(skills::install(root, "codex", "").is_err());
    assert_eq!(files(root), before);
    assert_eq!(
        fs::read(outside.path().join("SKILL.md")).unwrap(),
        b"external skill\n"
    );
}

#[cfg(unix)]
#[test]
fn uninstall_rejects_symlinked_owned_files_before_deleting_other_skills() {
    use std::os::unix::fs::symlink;
    let tmp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let root = tmp.path();
    skills::install(root, "codex", "").unwrap();
    let path = root.join(installed_path("codex", "resume"));
    fs::remove_file(&path).unwrap();
    put(outside.path(), "user.md", "external user file\n");
    symlink(outside.path().join("user.md"), &path).unwrap();
    let before = files(root);
    assert!(skills::uninstall(root).is_err());
    assert_eq!(files(root), before);
    assert_eq!(
        fs::read(outside.path().join("user.md")).unwrap(),
        b"external user file\n"
    );
}

#[cfg(unix)]
#[test]
fn init_preflights_gitignore_symlinks_before_writing_skills_or_configuration() {
    use std::os::unix::fs::symlink;
    let tmp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let root = tmp.path();
    put(root, ".specify/feature.json", "{}\n");
    put(outside.path(), "ignore", "outside ignore\n");
    symlink(outside.path().join("ignore"), root.join(".gitignore")).unwrap();
    let before = files(root);
    assert!(
        skills::init(root, Some("codex"), "")
            .unwrap_err()
            .to_string()
            .contains("path_outside_scope")
    );
    assert_eq!(files(root), before);
    assert!(!root.join(".spec-autonomous").exists());
    assert_eq!(
        fs::read(outside.path().join("ignore")).unwrap(),
        b"outside ignore\n"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_ownership_manifest_cannot_redirect_install_or_uninstall() {
    use std::os::unix::fs::symlink;
    let tmp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let root = tmp.path();
    skills::install(root, "codex", "").unwrap();
    let state = fs::read(root.join(MANIFEST)).unwrap();
    put(outside.path(), "manifest.toml", &state);
    fs::remove_file(root.join(MANIFEST)).unwrap();
    symlink(outside.path().join("manifest.toml"), root.join(MANIFEST)).unwrap();
    let before = files(root);

    assert!(
        skills::install(root, "codex", "")
            .unwrap_err()
            .to_string()
            .contains("path_outside_scope")
    );
    assert_eq!(files(root), before);
    assert!(
        skills::uninstall(root)
            .unwrap_err()
            .to_string()
            .contains("path_outside_scope")
    );
    assert_eq!(files(root), before);
    assert_eq!(
        fs::read(outside.path().join("manifest.toml")).unwrap(),
        state
    );
}
