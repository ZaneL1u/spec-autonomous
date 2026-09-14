use super::*;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub description: String,
    pub primary: bool,
    pub mutates: bool,
    pub input_schema: Value,
    pub output_schema: Value,
}
fn entry(
    id: &str,
    description: &str,
    primary: bool,
    mutates: bool,
    required: &[&str],
    mut properties: Value,
) -> Capability {
    let p = properties.as_object_mut().unwrap();
    p.insert(
        "view".into(),
        json!({"type":"string","enum":["agent","full"]}),
    );
    if !mutates {
        p.insert(
            "limit".into(),
            json!({"type":"integer","minimum":1,"maximum":200}),
        );
        p.insert(
            "offset".into(),
            json!({"type":"integer","minimum":0,"maximum":10000000}),
        );
        p.insert(
            "fields".into(),
            json!({"type":"array","items":{"type":"string"}}),
        );
    }
    Capability {
        id: id.into(),
        description: description.into(),
        primary,
        mutates,
        input_schema: json!({"type":"object","additionalProperties":false,"properties":properties,"required":required}),
        output_schema: json!({"type":"object","required":["schema_version","data"],"properties":{"schema_version":{"type":"integer","const":1},"data":{"type":"object"}}}),
    }
}
pub fn all() -> Vec<Capability> {
    let s = json!({"type":"string"});
    let b = json!({"type":"boolean"});
    let o = json!({"type":"object"});
    let f = json!({"type":"string","enum":["auto","openspec","speckit"]});
    let host = json!({"type":"object","additionalProperties":false,"required":["host_id","session_id","fresh_context"],"properties":{"host_id":s,"session_id":s,"fresh_context":b}});
    let source =
        json!({"framework":f,"change":s,"feature":s,"selector":s,"milestone_id":s,"phase_id":s});
    let mut out = vec![
        entry(
            "inspect",
            "Read provider, native artifacts, provenance and repository state.",
            true,
            false,
            &[],
            source.clone(),
        ),
        entry(
            "progress",
            "Read all Git worktrees and verified progress without mutation.",
            true,
            false,
            &[],
            json!({}),
        ),
        entry(
            "prepare",
            "Prepare work packets and advance the selected milestone.",
            true,
            true,
            &[],
            json!({"plan":o,"run_id":s,"framework":f,"change":s,"feature":s,"selector":s,"milestone_id":s,"goal":s,"id":s,"mode":{"type":"string","enum":["native","autonomous","plan"]},"from":s,"to":s,"only":s,"max_workers":{"type":"integer","minimum":1,"maximum":32},"delivery":{"type":"string","enum":["ff-original","branch"]},"reload_config":b,"extend_seconds":{"type":"integer","minimum":0,"maximum":2592000},"max_attempts":{"type":"integer","minimum":1,"maximum":1000}}),
        ),
        entry(
            "discussion.next",
            "Preview phase gray areas with recommended and alternative choices.",
            true,
            false,
            &["run_id"],
            json!({"run_id":s,"phase_id":s}),
        ),
        entry(
            "discussion.apply",
            "Apply clicked or recommended phase discussion choices with source CAS.",
            false,
            true,
            &["run_id", "source_hash", "selections"],
            json!({"run_id":s,"phase_id":s,"source_hash":s,"selections":{"type":"array","items":{"type":"object"}},"auto":b}),
        ),
        entry(
            "next",
            "Preview prepared work, blockers and next action without dispatching.",
            true,
            false,
            &[],
            json!({"run_id":s}),
        ),
        entry(
            "apply-result",
            "Accept an owned host receipt, verify and integrate it, and prepare subsequent work.",
            true,
            true,
            &["token", "host", "result"],
            json!({"token":s,"host":host,"result":o}),
        ),
        entry(
            "archive",
            "Preview a native archive; apply with its plan_hash using an isolated candidate.",
            true,
            true,
            &[],
            json!({"framework":f,"milestone_id":s,"change":s,"feature":s,"selector":s,"apply":b,"plan_hash":s}),
        ),
        entry(
            "doctor",
            "Inspect configuration, ledger, inventory and recoverable operations.",
            true,
            false,
            &[],
            json!({}),
        ),
        entry(
            "capabilities",
            "Discover complete and granular interfaces with versioned schemas.",
            false,
            false,
            &[],
            json!({"all":b}),
        ),
        entry(
            "work.claim",
            "Register the host's unique fresh session for a prepared request.",
            false,
            true,
            &["run_id", "request_id", "token", "host"],
            json!({"run_id":s,"request_id":s,"token":s,"host":host}),
        ),
        entry(
            "work.heartbeat",
            "Report host liveness and receive stop/pause instructions.",
            false,
            true,
            &["run_id", "request_id", "token"],
            json!({"run_id":s,"request_id":s,"token":s}),
        ),
        entry(
            "work.revoke",
            "Revoke a request after its host confirms execution has stopped.",
            false,
            true,
            &["run_id", "request_id", "token", "host_stopped", "reason"],
            json!({"run_id":s,"request_id":s,"token":s,"host_stopped":b,"reason":s}),
        ),
        entry(
            "work.context",
            "Read complete immutable work context with verified hashes.",
            false,
            false,
            &["run_id", "request_id"],
            json!({"run_id":s,"request_id":s}),
        ),
        entry(
            "hook.resolve",
            "Record the explicit outcome of an interrupted native hook.",
            false,
            true,
            &["run_id", "key", "outcome", "evidence"],
            json!({"run_id":s,"key":s,"outcome":{"type":"string","enum":["completed","not-run"]},"evidence":s}),
        ),
        entry(
            "native.instructions",
            "Read the exact installed provider action without generating a private replacement.",
            false,
            false,
            &[],
            source.clone(),
        ),
        entry(
            "native.create",
            "Create a native change or selected feature directory through the provider bridge.",
            false,
            true,
            &[],
            json!({"framework":f,"change":s,"feature":s,"selector":s}),
        ),
        entry(
            "repair",
            "Preview and apply supported repairs with source hashes and retained backups.",
            false,
            true,
            &["kind"],
            json!({"kind":{"type":"string","enum":["legacy-config","registry","abandon-operation"]},"operation_id":s,"apply":b,"expected_hash":s}),
        ),
        entry(
            "document.inspect",
            "Parse Markdown/TOML metadata, headings and task provenance.",
            false,
            false,
            &["file"],
            json!({"file":s,"framework":f,"content":b}),
        ),
        entry(
            "frontmatter.get",
            "Read structured frontmatter or TOML fields with source hash.",
            false,
            false,
            &["file"],
            json!({"file":s,"framework":f}),
        ),
        entry(
            "frontmatter.patch",
            "CAS-merge frontmatter fields while retaining Markdown body bytes.",
            false,
            true,
            &["file", "expected_hash", "patch"],
            json!({"file":s,"framework":f,"expected_hash":s,"patch":o}),
        ),
        entry(
            "toml.patch",
            "CAS-update selected top-level TOML values.",
            false,
            true,
            &["file", "expected_hash", "patch"],
            json!({"file":s,"expected_hash":s,"patch":o}),
        ),
        entry(
            "document.patch",
            "Replace one exact document span after a source-hash check.",
            false,
            true,
            &["file", "expected_hash", "find", "replace"],
            json!({"file":s,"expected_hash":s,"find":s,"replace":s}),
        ),
        entry(
            "document.scaffold",
            "Create operational summary/verification/handoff/decision templates without overwriting.",
            false,
            true,
            &["file", "kind"],
            json!({"file":s,"kind":{"type":"string","enum":["summary","verification","handoff","decision"]},"title":s}),
        ),
        entry(
            "roadmap.import",
            "Validate and save a typed milestone with its generated Markdown view.",
            false,
            true,
            &["milestone"],
            json!({"milestone":o,"expected_hash":s}),
        ),
        entry(
            "roadmap.get",
            "Read the canonical TOML milestone and its digest.",
            false,
            false,
            &["milestone_id"],
            json!({"milestone_id":s}),
        ),
        entry(
            "roadmap.select",
            "Resolve an inclusive range using verified outside prerequisites.",
            false,
            false,
            &["milestone_id"],
            json!({"milestone_id":s,"from":s,"to":s,"only":s}),
        ),
        entry(
            "roadmap.add",
            "Append a phase with dependency and source ownership validation.",
            false,
            true,
            &["milestone_id", "expected_hash", "phase"],
            json!({"milestone_id":s,"expected_hash":s,"phase":o}),
        ),
        entry(
            "roadmap.insert",
            "Insert a phase after a stable ID/label without renumbering identities.",
            false,
            true,
            &["milestone_id", "expected_hash", "phase", "after"],
            json!({"milestone_id":s,"expected_hash":s,"phase":o,"after":s}),
        ),
        entry(
            "roadmap.remove",
            "Remove a phase only if the remaining graph is valid.",
            false,
            true,
            &["milestone_id", "expected_hash", "phase_id"],
            json!({"milestone_id":s,"expected_hash":s,"phase_id":s}),
        ),
        entry(
            "roadmap.render",
            "Regenerate the roadmap view without overwriting human edits.",
            false,
            true,
            &["milestone_id", "expected_hash"],
            json!({"milestone_id":s,"expected_hash":s}),
        ),
        entry(
            "worktree.list",
            "List all worktrees, including unmanaged entries.",
            false,
            false,
            &[],
            json!({}),
        ),
        entry(
            "worktree.diff",
            "Read worktree differences and status without changing files.",
            false,
            false,
            &["worktree"],
            json!({"worktree":s,"base":s}),
        ),
        entry(
            "worktree.create",
            "Create a named isolated workspace from an explicit commit/ref.",
            false,
            true,
            &["name"],
            json!({"name":s,"base":s}),
        ),
        entry(
            "worktree.merge",
            "Verify and fast-forward a tool-created workspace with source/destination CAS.",
            false,
            true,
            &["worktree_id", "expected_head", "expected_target"],
            json!({"worktree_id":s,"expected_head":s,"expected_target":s}),
        ),
        entry(
            "worktree.remove",
            "Remove a clean owned workspace, keeping branch refs.",
            false,
            true,
            &["worktree_id", "expected_head"],
            json!({"worktree_id":s,"expected_head":s}),
        ),
        entry(
            "state.get",
            "Read a run, decisions and blockers without exposing credentials.",
            false,
            false,
            &["run_id"],
            json!({"run_id":s}),
        ),
        entry(
            "state.decision",
            "Append a decision with optional state revision guard.",
            false,
            true,
            &["run_id", "summary"],
            json!({"run_id":s,"summary":s,"rationale":s,"expected_updated_at":s}),
        ),
        entry(
            "state.block",
            "Add an explicit blocker that prevents workflow advancement.",
            false,
            true,
            &["run_id", "blocker_id", "reason"],
            json!({"run_id":s,"blocker_id":s,"reason":s,"expected_updated_at":s}),
        ),
        entry(
            "state.unblock",
            "Resolve one explicit blocker without forcing completion.",
            false,
            true,
            &["run_id", "blocker_id"],
            json!({"run_id":s,"blocker_id":s,"expected_updated_at":s}),
        ),
        entry(
            "state.checkpoint",
            "Record continuity and next-action notes.",
            false,
            true,
            &["run_id", "stopped_at"],
            json!({"run_id":s,"stopped_at":s,"next":s,"expected_updated_at":s}),
        ),
        entry(
            "history.get",
            "Read ordered durable events and decisions.",
            false,
            false,
            &["run_id"],
            json!({"run_id":s}),
        ),
    ];
    out.extend([
        entry(
            "verify.source",
            "Read native readiness and source validation diagnostics.",
            false,
            false,
            &[],
            source.clone(),
        ),
        entry(
            "verify.plan",
            "Validate a supplied execution plan against its native source.",
            false,
            false,
            &["plan"],
            {
                let mut p = source.clone();
                p["plan"] = o.clone();
                p
            },
        ),
        entry(
            "verify.references",
            "Check local Markdown references without reading outside the project.",
            false,
            false,
            &["file"],
            json!({"file":s}),
        ),
        entry(
            "audit.open",
            "Read outstanding native tasks, verification gaps and host work.",
            false,
            false,
            &["run_id"],
            json!({"run_id":s}),
        ),
        entry(
            "history.summaries",
            "Read compact summaries of completed semantic work and failures.",
            false,
            false,
            &["run_id"],
            json!({"run_id":s}),
        ),
        entry(
            "git.inspect",
            "Read repository HEAD, branch and working/index state.",
            false,
            false,
            &[],
            json!({}),
        ),
        entry(
            "git.commit",
            "Commit explicitly selected files without capturing unrelated staged changes.",
            false,
            true,
            &["expected_head", "message", "files"],
            json!({"expected_head":s,"message":s,"files":{"type":"array","items":s}}),
        ),
    ]);
    out.push(entry("run.revise", "Preview/apply pending verification corrections while preserving verified work and native requirements.", false, true, &["run_id", "reason"], json!({"run_id":s,"reason":s,"task_checks":{"type":"array","items":{"type":"object"}},"phase_checks":{"type":"array","items":{"type":"object"}},"milestone_checks":{"type":"array","items":{"type":"object"}},"apply":b,"plan_hash":s})));
    out.push(entry("work.claim-batch", "Atomically claim prepared work for distinct host sessions in one operation.", false, true, &["run_id","requests"], json!({"run_id":s,"requests":{"type":"array","minItems":1,"maxItems":256,"items":{"type":"object","additionalProperties":false,"required":["request_id","token","host"],"properties":{"request_id":s,"token":s,"host":host}}}})));
    for (id, description, write) in [
        (
            "run.pause",
            "Request pause and return host cancellation actions.",
            true,
        ),
        (
            "run.cancel",
            "Cancel a run without pretending external sessions stopped.",
            true,
        ),
        (
            "run.cleanup",
            "Preview/apply terminal worktree cleanup and optional safe merged-branch deletion.",
            true,
        ),
        (
            "task.list",
            "Read execution tasks and source bindings.",
            false,
        ),
        (
            "task.ready",
            "Read eligible tasks under dependency and conflict constraints.",
            false,
        ),
    ] {
        out.push(entry(
            id,
            description,
            false,
            write,
            &["run_id"],
            if id == "run.cleanup" {
                json!({"run_id":s,"apply":b,"delete_branches":b,"delete_integration":b,"plan_hash":s})
            } else {
                json!({"run_id":s,"phase_id":s})
            },
        ));
    }
    let mut complete = out.iter().find(|c| c.id == "apply-result").unwrap().clone();
    complete.id = "task.complete".into();
    complete.primary = false;
    complete.description =
        "Complete owned task work through the same verified receipt gate.".into();
    out.push(complete);
    let mut claim = out.iter().find(|c| c.id == "work.claim").unwrap().clone();
    claim.id = "task.claim".into();
    claim.description = "Claim a prepared task for a fresh host context.".into();
    out.push(claim);
    out
}
pub fn list(all_: bool) -> Value {
    json!({"schema_version":1,"starts_agents":false,"capabilities":all().into_iter().filter(|c|all_||c.primary).collect::<Vec<_>>()})
}
pub fn get(id: &str) -> Result<Capability> {
    all()
        .into_iter()
        .find(|c| c.id == id)
        .context("unknown_capability")
}
pub fn validate(id: &str, args: &Value) -> Result<()> {
    if serde_json::to_vec(args)?.len() > 2 * 1024 * 1024 {
        bail!("arguments_too_large");
    }
    let cap = get(id)?;
    validate_value(args, &cap.input_schema, "arguments")?;
    if id == "prepare" {
        if args.get("run_id").is_some() && args.get("plan").is_some() {
            bail!(
                "revision_required: prepare cannot replace a running plan; use run.revise for pending verification corrections"
            );
        }
        let selectors = [
            "run_id",
            "milestone_id",
            "change",
            "feature",
            "selector",
            "goal",
        ]
        .iter()
        .filter(|k| args.get(**k).is_some())
        .count();
        if selectors > 1 {
            bail!("selection_conflict: choose one run/milestone/native source/goal");
        }
        if args.get("only").is_some() && (args.get("from").is_some() || args.get("to").is_some()) {
            bail!("invalid_range: only conflicts with from/to");
        }
    }
    Ok(())
}
fn validate_value(value: &Value, schema: &Value, path: &str) -> Result<()> {
    let valid = match schema["type"].as_str() {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("boolean") => value.is_boolean(),
        Some("integer") => value.as_u64().is_some(),
        _ => true,
    };
    if !valid {
        bail!("invalid_arguments: wrong type at {path}");
    }
    if let Some(choices) = schema["enum"].as_array() {
        if !choices.contains(value) {
            bail!("invalid_arguments: unsupported value at {path}");
        }
    }
    if let Some(number) = value.as_u64() {
        if schema["minimum"].as_u64().is_some_and(|m| number < m)
            || schema["maximum"].as_u64().is_some_and(|m| number > m)
        {
            bail!("invalid_arguments: out of range at {path}");
        }
    }
    if let Some(object) = value.as_object() {
        for required in schema["required"].as_array().into_iter().flatten() {
            if !object.contains_key(required.as_str().unwrap()) {
                bail!(
                    "invalid_arguments: missing {path}.{}",
                    required.as_str().unwrap()
                );
            }
        }
        if let Some(properties) = schema["properties"].as_object() {
            for (key, v) in object {
                if let Some(s) = properties.get(key) {
                    validate_value(v, s, &format!("{path}.{key}"))?;
                } else if schema["additionalProperties"] == false {
                    bail!("invalid_arguments: unknown field {path}.{key}");
                }
            }
        }
    }
    if let Some(array) = value.as_array() {
        if let Some(items) = schema.get("items") {
            for v in array {
                validate_value(v, items, path)?;
            }
        }
    }
    Ok(())
}
