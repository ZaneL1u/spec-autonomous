//! Immutable work packets for an external host. This module never starts a process.
use crate::{config::Config, model::*, paths};
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::{fs, path::Path};

pub fn doctor(config: &Config) -> Result<serde_json::Value> {
    config.validate()?;
    Ok(
        json!({"execution":"host-driven","starts_agents":false,"protocol_version":1,"max_concurrency":config.host.max_concurrency,"legacy_runner_ignored":config.runner.profile!="disabled"}),
    )
}
pub fn prepare(input: &WorkerInput, root: &Path, dir: &Path, config: &Config) -> Result<()> {
    let mapped = map_input(input, root);
    let input_file = dir.join("input.json");
    let result_file = dir.join("result.json");
    if result_file.exists()
        || input_file.exists()
        || dir.join("prompt.md").exists()
        || dir.join("context-full.json").exists()
    {
        bail!("attempt_reused: immutable attempt files already exist");
    }
    let full = serde_json::to_vec_pretty(&mapped)?;
    let mut packet = mapped.clone();
    let budget = config.execution.max_context_bytes;
    if full.len() + 1600 > budget {
        if packet.snapshot.is_none() {
            bail!("context_too_large: required goal or envelope exceeds packet budget");
        }
        let reference = dir.join("context-full.json");
        let snapshot = packet.snapshot.as_mut().unwrap();
        snapshot.context_files.clear();
        snapshot.tasks.clear();
        snapshot.metadata =
            json!({"context_reference":{"path":reference,"sha256":paths::hash(&full)}});
        snapshot.next_action.instruction =
            "Read the complete native action from the immutable context reference before acting."
                .into();
        packet.milestone = None;
        packet.instruction = format!(
            "MANDATORY: Read the complete WorkerInput JSON at {} before acting. Its sha256 is {}. It contains the original unit instruction, required native rules, source files, task list and acceptance references. Nothing has been discarded: this packet only references that immutable full context. Respect its exact scope and result identity.",
            reference.display(),
            paths::hash(&full)
        );
        if serde_json::to_vec_pretty(&packet)?.len() + 1600 > budget {
            bail!("context_too_large: minimal reference packet does not fit");
        }
        fs::create_dir_all(dir)?;
        paths::atomic_write(&reference, &full)?;
    }
    let input = &packet;
    let data = serde_json::to_vec_pretty(input)?;
    fs::create_dir_all(dir)?;
    paths::atomic_write(&input_file, &data)?;
    let prompt = format!(
        "You are a fresh Spec Autonomous worker. Your task is the JSON request below. Follow its native SDD instructions and repository rules. Work only inside the assigned checkout. If context_reference is present, read the immutable complete request before acting. The host owns your session and must report it with the receipt. Do not start other autonomous coordinators, write shared task checkboxes, commit, push or publish. The coordinator owns native extension hooks: do not invoke hooks yourself, including hooks mentioned by native skills. Native planning may create only snapshot.next_action.outputs; Spec Kit feature.json may be updated locally by the native command but will be restored before integration. Return a single JSON object matching WorkerResult: schema_version=1, run_id/task_id/attempt_id copied exactly, status=candidate|blocked|failed, summary<=8192 bytes, blockers array, milestone or null, plan or null, audit array. Roadmap work returns milestone; task planning returns plan; verification returns requirement/evidence/passed audit items. Never claim integrated/verified. Only implementation/native-planning/converge work may edit allowed files.\n\nREQUEST:\n{}",
        String::from_utf8_lossy(&data)
    );
    if prompt.len() > budget {
        bail!("context_too_large: prompt exceeds configured byte budget");
    }
    paths::atomic_write(&dir.join("prompt.md"), &prompt)?;
    paths::atomic_write(
        &dir.join("result.schema.json"),
        serde_json::to_vec_pretty(&result_schema(input))?,
    )?;
    Ok(())
}
pub fn read_input(dir: &Path) -> Result<WorkerInput> {
    let input: WorkerInput = serde_json::from_slice(&fs::read(dir.join("input.json"))?)?;
    if let Some(reference) = input
        .snapshot
        .as_ref()
        .and_then(|s| s.metadata.get("context_reference"))
    {
        let bytes = fs::read(dir.join("context-full.json"))?;
        if reference["sha256"].as_str() != Some(paths::hash(&bytes).as_str()) {
            bail!("context_corrupt: full packet hash changed");
        }
        return Ok(serde_json::from_slice(&bytes)?);
    }
    Ok(input)
}
pub fn read_result(dir: &Path) -> Result<WorkerResult> {
    if fs::metadata(dir.join("result.json"))?.len() > 64 * 1024 {
        bail!("worker_protocol_error: result exceeds 64 KiB");
    }
    let result: WorkerResult = serde_json::from_slice(&fs::read(dir.join("result.json"))?)
        .context("worker_protocol_error: invalid result JSON")?;
    validate_result(&read_input(dir)?, &result)?;
    Ok(result)
}
pub fn result_schema(input: &WorkerInput) -> serde_json::Value {
    fn object(properties: serde_json::Value) -> serde_json::Value {
        let required: Vec<_> = properties.as_object().unwrap().keys().cloned().collect();
        json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
    }
    let string = json!({"type":"string"});
    let strings = json!({"type":"array","items":string});
    let check = object(json!({"argv":strings,"cwd":string}));
    let checks = json!({"type":"array","items":{"$ref":"#/$defs/check"}});
    let source = object(
        json!({"kind":{"type":"string","enum":["openspec-change","speckit-feature"]},"selector":string}),
    );
    let phase = object(
        json!({"id":string,"label":string,"title":string,"depends_on":strings,"source":{"$ref":"#/$defs/source"},"verification":checks}),
    );
    let milestone = object(
        json!({"schema_version":{"type":"integer","enum":[1]},"id":string,"goal":string,"framework":{"type":"string","enum":["openspec","speckit"]},"revision":{"type":"integer"},"phases":{"type":"array","items":{"$ref":"#/$defs/phase"}},"verification":checks}),
    );
    let task = object(
        json!({"id":string,"description":string,"source_ids":strings,"depends_on":strings,"reads":strings,"writes":strings,"verification":checks}),
    );
    let plan = object(
        json!({"schema_version":{"type":"integer","enum":[1]},"phase_id":string,"source_hash":string,"tasks":{"type":"array","items":{"$ref":"#/$defs/task"}},"milestone":{"anyOf":[{"$ref":"#/$defs/milestone"},{"type":"null"}]}}),
    );
    let audit = object(json!({"requirement":string,"evidence":string,"passed":{"type":"boolean"}}));
    let mut schema = object(
        json!({"schema_version":{"type":"integer","enum":[1]},"run_id":{"type":"string","enum":[input.run_id]},"task_id":{"type":"string","enum":[input.task_id]},"attempt_id":{"type":"string","enum":[input.attempt_id]},"status":{"type":"string","enum":["candidate","blocked","failed"]},"summary":string,"blockers":strings,"milestone":{"anyOf":[{"$ref":"#/$defs/milestone"},{"type":"null"}]},"plan":{"anyOf":[{"$ref":"#/$defs/plan"},{"type":"null"}]},"audit":{"type":"array","items":{"$ref":"#/$defs/audit"}}}),
    );
    schema["$defs"] = json!({"check":check,"source":source,"phase":phase,"milestone":milestone,"task":task,"plan":plan,"audit":audit});
    schema
}
fn map_input(input: &WorkerInput, root: &Path) -> WorkerInput {
    let mut input = input.clone();
    if let Some(snapshot) = &mut input.snapshot {
        let source = snapshot
            .metadata
            .pointer("/root/path")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        if let Some(source) = source {
            let target = root.to_string_lossy();
            input.instruction = input.instruction.replace(&source, &target);
            snapshot.next_action.instruction =
                snapshot.next_action.instruction.replace(&source, &target);
            fn map(value: &mut serde_json::Value, from: &str, to: &str) {
                match value {
                    serde_json::Value::String(s) => *s = s.replace(from, to),
                    serde_json::Value::Array(a) => a.iter_mut().for_each(|v| map(v, from, to)),
                    serde_json::Value::Object(o) => o.values_mut().for_each(|v| map(v, from, to)),
                    _ => (),
                }
            }
            map(&mut snapshot.metadata, &source, &target);
        }
    }
    input
}
pub fn validate_result(input: &WorkerInput, result: &WorkerResult) -> Result<()> {
    if result.schema_version != SCHEMA
        || result.run_id != input.run_id
        || result.task_id != input.task_id
        || result.attempt_id != input.attempt_id
    {
        bail!("worker_protocol_error: mismatched identity/schema");
    }
    if result.summary.len() > 8192
        || !["candidate", "blocked", "failed"].contains(&result.status.as_str())
    {
        bail!("worker_protocol_error: invalid status or oversized summary");
    }
    if result.status != "candidate" || !result.blockers.is_empty() {
        bail!(
            "needs_input: {} {}",
            result.summary,
            result.blockers.join("; ")
        );
    }
    Ok(())
}
