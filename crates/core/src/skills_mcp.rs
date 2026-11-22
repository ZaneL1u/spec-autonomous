//! Opt-in project MCP binding. Existing unrelated settings and owned-region
//! bytes are preserved; modified owned entries are never overwritten.
use crate::paths;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};
const BEGIN: &str = "# BEGIN spec-autonomous MCP\n";
const END: &str = "# END spec-autonomous MCP\n";
fn span(text: &str) -> Result<Option<(usize, usize)>> {
    if let Some(start) = text.find(BEGIN) {
        let end = text[start..]
            .find(END)
            .context("mcp_conflict: incomplete managed region")?
            + start
            + END.len();
        if text[end..].contains(BEGIN) {
            bail!("mcp_conflict: multiple managed regions");
        }
        Ok(Some((start, end)))
    } else {
        Ok(None)
    }
}
fn records(root: &Path) -> Result<BTreeMap<String, String>> {
    let file = paths::inside(root, ".spec-autonomous/mcp-installed.toml")?;
    if !file.exists() {
        return Ok(BTreeMap::new());
    }
    Ok(toml::from_str(&fs::read_to_string(file)?)?)
}
fn record(root: &Path, records: &BTreeMap<String, String>) -> Result<()> {
    paths::atomic_write(
        &paths::inside(root, ".spec-autonomous/mcp-installed.toml")?,
        toml::to_string_pretty(records)?,
    )?;
    Ok(())
}
/// Absolute launcher argv the npm layer passes to the native binary as a JSON array
/// (Node and the CLI entry script), for MCP configurations outside the PATH.
pub const LAUNCHER_ENV: &str = "SPEC_AUTONOMOUS_LAUNCHER";
const BARE: &str = "spec-autonomous";

/// The argv an MCP host starts before `--path <root> mcp`, and a warning when it may not
/// start. A bare `spec-autonomous` is portable but only works when the CLI is on the PATH
/// the host inherits; otherwise the absolute launcher reported by the npm layer is used.
pub fn launch_command(on_path: bool, launcher: Option<&str>) -> (Vec<String>, Option<String>) {
    if on_path {
        return (vec![BARE.into()], None);
    }
    let absolute = launcher
        .and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
        .filter(|argv| {
            !argv.is_empty()
                && argv
                    .iter()
                    .all(|part| Path::new(part).is_absolute() && Path::new(part).is_file())
        });
    match absolute {
        Some(argv) => (argv, None),
        None => (
            vec![BARE.into()],
            Some(format!(
                "mcp_command_not_on_path: {BARE} is not on PATH, so an MCP host starting it fails with ENOENT; put the CLI on PATH (see the install guide) or run init --mcp through the npm launcher so the absolute command is written"
            )),
        ),
    }
}
fn current_launch_command() -> (Vec<String>, Option<String>) {
    launch_command(
        crate::verification_preflight::find_on_path(BARE).is_some(),
        std::env::var(LAUNCHER_ENV).ok().as_deref(),
    )
}
/// Doctor findings for installed MCP entries whose command cannot start.
pub fn diagnostics(root: &Path) -> Vec<String> {
    let mut commands = vec![];
    if let Ok(file) = paths::inside(root, ".mcp.json")
        && let Ok(text) = fs::read_to_string(file)
        && let Ok(value) = serde_json::from_str::<Value>(&text)
        && let Some(command) = value
            .pointer("/mcpServers/spec_autonomous/command")
            .and_then(Value::as_str)
    {
        commands.push((".mcp.json", command.to_owned()));
    }
    if let Ok(file) = paths::inside(root, ".codex/config.toml")
        && let Ok(text) = fs::read_to_string(file)
        && let Ok(value) = toml::from_str::<toml::Value>(&text)
        && let Some(command) = value
            .get("mcp_servers")
            .and_then(|v| v.get("spec_autonomous"))
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
    {
        commands.push((".codex/config.toml", command.to_owned()));
    }
    let mut out = vec![];
    for (file, command) in commands {
        let path = Path::new(&command);
        if path.is_absolute() {
            if !path.is_file() {
                out.push(format!(
                    "mcp_command_missing: {file} starts {command}, which does not exist; run spec-autonomous init --mcp again"
                ));
            }
        } else if crate::verification_preflight::find_on_path(&command).is_none() {
            out.push(format!(
                "mcp_command_not_on_path: {file} starts {command:?}, which is not on this PATH; MCP hosts fail with ENOENT. Put the CLI on PATH or run spec-autonomous init --mcp through the npm launcher to write an absolute command. Hosts started from a desktop launcher may see a shorter PATH than this shell"
            ));
        }
    }
    out
}
pub fn install(root: &Path, agent: &str) -> Result<Value> {
    apply(root, agent, true)
}
pub fn preflight(root: &Path, agent: &str) -> Result<Value> {
    apply(root, agent, false)
}
fn apply(root: &Path, agent: &str, write: bool) -> Result<Value> {
    let mut owned = records(root)?;
    let (argv, warning) = current_launch_command();
    let mut args: Vec<Value> = argv[1..].iter().map(|a| json!(a)).collect();
    args.extend([json!("--path"), json!(root.canonicalize()?), json!("mcp")]);
    let entry = json!({"command":argv[0],"args":args});
    let (relative, before, after, hash) = match agent {
        "codex" => {
            let relative = ".codex/config.toml";
            let file = paths::inside(root, relative)?;
            let before = if file.exists() {
                fs::read_to_string(&file)?
            } else {
                String::new()
            };
            let parsed: toml::Value = toml::from_str(&before)?;
            let block = format!(
                "{BEGIN}[mcp_servers.spec_autonomous]\ncommand = {}\nargs = {}\n{END}",
                toml::Value::String(argv[0].clone()),
                serde_json::to_string(&entry["args"])?
            );
            let after = if let Some((start, end)) = span(&before)? {
                if owned.get(agent) != Some(&paths::hash(&before[start..end])) {
                    bail!("mcp_conflict: preserve modified Codex MCP entry");
                }
                format!("{}{}{}", &before[..start], block, &before[end..])
            } else {
                if parsed
                    .get("mcp_servers")
                    .and_then(|v| v.get("spec_autonomous"))
                    .is_some()
                {
                    bail!("mcp_conflict: an existing Codex server owns this name");
                }
                format!("{before}\n{block}")
            };
            let _:toml::Value=toml::from_str(&after).context("mcp_conflict: cannot append to this TOML structure without rewriting unrelated configuration")?;
            (relative, before, after, paths::hash(block))
        }
        "claude" => {
            let relative = ".mcp.json";
            let file = paths::inside(root, relative)?;
            let before = if file.exists() {
                fs::read_to_string(&file)?
            } else {
                String::new()
            };
            let mut parsed: Value = if before.is_empty() {
                json!({})
            } else {
                serde_json::from_str(&before)?
            };
            if parsed.get("mcpServers").is_none() {
                parsed["mcpServers"] = json!({});
            }
            let servers = parsed["mcpServers"]
                .as_object_mut()
                .context("mcp_conflict: mcpServers must be an object")?;
            if let Some(current) = servers.get("spec_autonomous") {
                if owned.get(agent) != Some(&paths::hash(serde_json::to_vec(current)?)) {
                    bail!("mcp_conflict: preserve existing Claude MCP entry");
                }
            }
            servers.insert("spec_autonomous".into(), entry.clone());
            (
                relative,
                before,
                format!("{}\n", serde_json::to_string_pretty(&parsed)?),
                paths::hash(serde_json::to_vec(&entry)?),
            )
        }
        _ => bail!("host_unsupported"),
    };
    if !write {
        return Ok(json!({"agent":agent,"file":relative,"ready":true}));
    }
    let file = paths::inside(root, relative)?;
    let current = if file.exists() {
        fs::read_to_string(&file)?
    } else {
        String::new()
    };
    if before != current {
        bail!("source_drift: MCP config changed");
    }
    if after != before {
        paths::atomic_write(&file, after)?;
    }
    owned.insert(agent.into(), hash);
    record(root, &owned)?;
    Ok(
        json!({"agent":agent,"file":relative,"server":"spec_autonomous","command":argv[0],"warnings":warning.map(|w| vec![w]).unwrap_or_default(),"starts_agents":false,"next_action":"Reload the host MCP configuration; existing host trust rules still apply."}),
    )
}
pub fn uninstall(root: &Path) -> Result<Vec<String>> {
    if !paths::inside(root, ".spec-autonomous/mcp-installed.toml")?.exists() {
        return Ok(vec![]);
    }
    let mut owned = records(root)?;
    let mut removed = vec![];
    if let Some(expected) = owned.get("codex") {
        let file = paths::inside(root, ".codex/config.toml")?;
        if file.exists() {
            let before = fs::read_to_string(&file)?;
            if let Some((start, end)) = span(&before)? {
                if &paths::hash(&before[start..end]) == expected {
                    paths::atomic_write(&file, format!("{}{}", &before[..start], &before[end..]))?;
                    owned.remove("codex");
                    removed.push(".codex/config.toml:mcp_servers.spec_autonomous".into());
                }
            }
        }
    }
    if let Some(expected) = owned.get("claude") {
        let file = paths::inside(root, ".mcp.json")?;
        if file.exists() {
            let mut value: Value = serde_json::from_slice(&fs::read(&file)?)?;
            if let Some(current) = value.pointer("/mcpServers/spec_autonomous") {
                if &paths::hash(serde_json::to_vec(current)?) == expected {
                    value["mcpServers"]
                        .as_object_mut()
                        .unwrap()
                        .remove("spec_autonomous");
                    paths::atomic_write(&file, serde_json::to_vec_pretty(&value)?)?;
                    owned.remove("claude");
                    removed.push(".mcp.json:mcpServers.spec_autonomous".into());
                }
            }
        }
    }
    record(root, &owned)?;
    Ok(removed)
}
