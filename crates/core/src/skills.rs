use crate::{config::Config, paths, provider};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

const ASSETS: [(&str, &str); 5] = [
    (
        "autonomous",
        include_str!("../../../packages/cli/skills/autonomous/SKILL.md"),
    ),
    (
        "auto",
        include_str!("../../../packages/cli/skills/auto/SKILL.md"),
    ),
    (
        "milestone",
        include_str!("../../../packages/cli/skills/milestone/SKILL.md"),
    ),
    (
        "progress",
        include_str!("../../../packages/cli/skills/progress/SKILL.md"),
    ),
    (
        "resume",
        include_str!("../../../packages/cli/skills/resume/SKILL.md"),
    ),
];
#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    #[serde(default)]
    version: String,
    #[serde(default)]
    files: BTreeMap<String, String>,
}
fn installation_plan(
    root: &Path,
    agent: &str,
    prefix: &str,
) -> Result<(Manifest, Vec<(String, String)>)> {
    if !prefix.is_empty() {
        paths::valid_id(prefix)?;
    }
    if !["codex", "claude"].contains(&agent) {
        bail!("host_unsupported: supported installation profiles: codex, claude");
    }
    let manifest_path = paths::inside(root, ".spec-autonomous/skills-installed.toml")?;
    let manifest: Manifest = if manifest_path.exists() {
        toml::from_str(&fs::read_to_string(&manifest_path)?)?
    } else {
        Manifest::default()
    };
    let mut changes = vec![];
    for (name, body) in ASSETS {
        let name = if prefix.is_empty() {
            name.into()
        } else {
            format!("{prefix}-{name}")
        };
        let path = if agent == "codex" {
            format!(".agents/skills/{name}/SKILL.md")
        } else {
            format!(".claude/commands/{name}.md")
        };
        let target = paths::inside(root, &path)?;
        if target.exists() {
            let current = paths::hash(fs::read(&target)?);
            if manifest.files.get(&path) != Some(&current) {
                bail!("skill_conflict: preserve existing {path}; choose --prefix");
            }
        }
        let original = body.lines().find(|l| l.starts_with("name: ")).unwrap();
        changes.push((path, body.replacen(original, &format!("name: {name}"), 1)));
    }
    Ok((manifest, changes))
}

pub fn install(root: &Path, agent: &str, prefix: &str) -> Result<Vec<String>> {
    let (mut manifest, changes) = installation_plan(root, agent, prefix)?;
    let manifest_path = paths::inside(root, ".spec-autonomous/skills-installed.toml")?;
    // All conflicts are checked before any file is changed.
    for (path, body) in &changes {
        paths::atomic_write(&paths::inside(root, path)?, body)?;
        manifest.files.insert(path.clone(), paths::hash(body));
    }
    manifest.version = env!("CARGO_PKG_VERSION").into();
    paths::atomic_write(&manifest_path, toml::to_string_pretty(&manifest)?)?;
    Ok(changes.into_iter().map(|(path, _)| path).collect())
}

pub fn preflight_init(root: &Path, agent: &str, prefix: &str, mcp: bool) -> Result<()> {
    let _ = Config::load(root).context("invalid_config: existing project configuration")?;
    let _ = installation_plan(root, agent, prefix)?;
    for name in [
        ".spec-autonomous",
        ".spec-autonomous/milestones",
        ".spec-autonomous/plans",
        ".spec-autonomous/archives",
    ] {
        let path = paths::inside(root, name)?;
        if path.exists() && !path.is_dir() {
            bail!("init_path_conflict: {name}");
        }
    }
    let ignore = paths::inside(root, ".gitignore")?;
    if ignore.exists() {
        let _ = fs::read_to_string(ignore).context("init_path_conflict: .gitignore")?;
    }
    if mcp {
        crate::skills_mcp::preflight(root, agent)?;
    }
    Ok(())
}
pub fn uninstall(root: &Path) -> Result<Vec<String>> {
    let path = paths::inside(root, ".spec-autonomous/skills-installed.toml")?;
    if !path.exists() {
        return crate::skills_mcp::uninstall(root);
    }
    let mut manifest: Manifest = toml::from_str(&fs::read_to_string(&path)?)?;
    let mut eligible = vec![];
    for (name, hash) in &manifest.files {
        let owned = name.starts_with(".agents/skills/") && name.ends_with("/SKILL.md")
            || name.starts_with(".claude/commands/") && name.ends_with(".md");
        if !owned {
            bail!("invalid_manifest: path outside owned skill locations");
        }
        let target = paths::inside(root, name)?;
        if target.is_file() && paths::hash(fs::read(&target)?) == *hash {
            eligible.push((name.clone(), target));
        }
    }
    let mut deleted = vec![];
    for (name, target) in eligible {
        fs::remove_file(target)?;
        deleted.push(name);
    }
    for name in &deleted {
        manifest.files.remove(name);
    }
    paths::atomic_write(&path, toml::to_string_pretty(&manifest)?)?;
    deleted.extend(crate::skills_mcp::uninstall(root)?);
    Ok(deleted)
}
pub fn init(root: &Path, agent: Option<&str>, prefix: &str) -> Result<serde_json::Value> {
    init_selected(root, agent, prefix, None)
}
fn init_selected(
    root: &Path,
    agent: Option<&str>,
    prefix: &str,
    framework: Option<crate::Framework>,
) -> Result<serde_json::Value> {
    let detected = provider::framework(root, framework)?;
    let agent = if let Some(a) = agent {
        a
    } else {
        match (root.join(".agents").exists(), root.join(".claude").exists()) {
            (true, false) => "codex",
            (false, true) => "claude",
            _ => bail!("host_selection_required: choose --agent codex|claude"),
        }
    };
    preflight_init(root, agent, prefix, false)?;
    let path = paths::inside(root, ".spec-autonomous/config.toml")?;
    let _ = Config::load(root).context("invalid existing project configuration")?;
    let ignore = paths::inside(root, ".gitignore")?;
    let mut text = if ignore.exists() {
        fs::read_to_string(&ignore)?
    } else {
        String::new()
    };
    let installed = install(root, agent, prefix)?;
    if !path.exists() {
        paths::atomic_write(&path, toml::to_string_pretty(&Config::default())?)?;
    }
    for name in ["milestones", "plans", "archives"] {
        fs::create_dir_all(paths::inside(root, &format!(".spec-autonomous/{name}"))?)?;
    }
    if !text.contains("# Spec Autonomous declarations") {
        text.push_str("\n# Spec Autonomous declarations (runtime lives in the Git common directory)\n!.spec-autonomous/\n.spec-autonomous/*\n!.spec-autonomous/config.toml\n!.spec-autonomous/skills-installed.toml\n!.spec-autonomous/mcp-installed.toml\n!.spec-autonomous/archives/\n!.spec-autonomous/milestones/\n!.spec-autonomous/plans/\n");
        paths::atomic_write(&ignore, &text)?;
    } else if !text
        .lines()
        .any(|line| line == "!.spec-autonomous/skills-installed.toml")
    {
        text.push_str("\n!.spec-autonomous/skills-installed.toml\n!.spec-autonomous/mcp-installed.toml\n!.spec-autonomous/archives/\n");
        paths::atomic_write(&ignore, &text)?;
    }
    let sigil = if agent == "claude" { "/" } else { "$" };
    let namespace = if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}-")
    };
    Ok(
        serde_json::json!({"root":root,"framework":detected,"agent":agent,"installed":installed,"config":".spec-autonomous/config.toml","directories":[".spec-autonomous/milestones", ".spec-autonomous/plans", ".spec-autonomous/archives"],"entry":format!("{sigil}{namespace}autonomous {sigil}{namespace}auto"),"next_action":"Configure host capacity and verification, then commit the project before prepare."}),
    )
}

pub use crate::skills_mcp::install as install_mcp;

pub fn init_with_mcp(
    root: &Path,
    agent: Option<&str>,
    prefix: &str,
    mcp: bool,
) -> Result<serde_json::Value> {
    init_with_provider(root, agent, prefix, mcp, None)
}
pub fn init_with_provider(
    root: &Path,
    agent: Option<&str>,
    prefix: &str,
    mcp: bool,
    framework: Option<crate::Framework>,
) -> Result<serde_json::Value> {
    let selected = if let Some(agent) = agent {
        agent
    } else {
        match (root.join(".agents").exists(), root.join(".claude").exists()) {
            (true, false) => "codex",
            (false, true) => "claude",
            _ => bail!("host_selection_required: choose --agent codex|claude"),
        }
    };
    if mcp {
        crate::skills_mcp::preflight(root, selected)?;
    }
    let mut result = init_selected(root, Some(selected), prefix, framework)?;
    if mcp {
        result["mcp"] = crate::skills_mcp::install(root, selected)?;
    }
    Ok(result)
}
