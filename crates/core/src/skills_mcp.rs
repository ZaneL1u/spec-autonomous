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
pub fn install(root: &Path, agent: &str) -> Result<Value> {
    apply(root, agent, true)
}
pub fn preflight(root: &Path, agent: &str) -> Result<Value> {
    apply(root, agent, false)
}
fn apply(root: &Path, agent: &str, write: bool) -> Result<Value> {
    let mut owned = records(root)?;
    let entry = json!({"command":"spec-autonomous","args":["--path",root.canonicalize()?,"mcp"]});
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
                "{BEGIN}[mcp_servers.spec_autonomous]\ncommand = \"spec-autonomous\"\nargs = {}\n{END}",
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
        json!({"agent":agent,"file":relative,"server":"spec_autonomous","command":"spec-autonomous","starts_agents":false,"next_action":"Reload the host MCP configuration; existing host trust rules still apply."}),
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
