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
    let mut mapped = map_input(input, root);
    trim(&mut mapped);
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
    let reference = input
        .snapshot
        .as_ref()
        .and_then(|s| s.metadata.get("context_reference"))
        .map(|r| {
            format!(
                " input.json is a bounded envelope: its snapshot.metadata.context_reference names the immutable complete request at {} (sha256 {}); read that file before acting.",
                r["path"].as_str().unwrap_or_default(),
                r["sha256"].as_str().unwrap_or_default()
            )
        })
        .unwrap_or_default();
    let prompt = format!(
        "You are a fresh Spec Autonomous worker. Your task is the JSON request in the input file named below. Follow its native SDD instructions and repository rules. Work only inside the assigned checkout. The host owns your session and must report it with the receipt. Do not start other autonomous coordinators, write shared task checkboxes, commit, push or publish. The coordinator owns native extension hooks: do not invoke hooks yourself, including hooks mentioned by native skills. Native planning may create only snapshot.next_action.outputs; Spec Kit feature.json may be updated locally by the native command but will be restored before integration. Return a single JSON object matching WorkerResult: schema_version=1, run_id/task_id/attempt_id copied exactly, status=candidate|blocked|failed, summary<=8192 bytes, blockers array, milestone or null, plan or null, audit array. Roadmap work returns milestone; task planning returns plan; verification returns requirement/evidence/passed audit items. Never claim integrated/verified. Only implementation/native-planning/converge work may edit allowed files.\n\nREQUEST: {} (sha256 {}). Read this JSON file completely before acting; it is the authority for identity, scope, the unit instruction, native instructions (snapshot.next_action), native context (snapshot.context_files) and tasks. The summary below only orients you and never replaces it.{reference}\n\nSUMMARY:\n{}",
        input_file.display(),
        paths::hash(&data),
        summary(input)
    );
    let prompt = if input.kind == "implement" {
        format!(
            "{prompt}\n\nWRITE SCOPE: change only files allowed by task.writes. If the task cannot be finished without changing other files (for example existing tests the change breaks), stop and return status=blocked with requested_writes listing those project-relative paths (at most {}). The coordinator adds safe paths to the write set and re-issues the task with your changes; otherwise it stops for operator review.",
            crate::run_revision::MAX_REQUESTED_WRITES
        )
    } else {
        prompt
    };
    let prompt = match &input.previous_candidate {
        Some(previous) => format!(
            "{prompt}\n\nPREVIOUS CANDIDATE: attempt {} of this task failed or was blocked (see failure) after changing {}. Its uncommitted changes are saved at {}. {} Review them against the failure before reusing anything; reused code is not evidence, so run this task's verification checks again before returning a candidate.",
            previous.attempt_id,
            previous.files.join(", "),
            previous.patch_path,
            if previous.same_base {
                "This checkout starts from the same commit, so `git apply --index <patch>` run from the worktree root restores them."
            } else {
                "This checkout starts from a newer commit; try `git apply --3way --index <patch>` from the worktree root, or use the patch as a reference."
            }
        ),
        None => prompt,
    };
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
/// Drop what a worker of this kind never reads: acceptance references outside audits,
/// parsed document outlines and the raw native JSON envelope for implementation, and a
/// planning failure repeated verbatim from `failure`.
fn trim(input: &mut WorkerInput) {
    let kind = input.kind.clone();
    let failure = input.failure.clone();
    let Some(snapshot) = input.snapshot.as_mut() else {
        return;
    };
    if let Some(meta) = snapshot.metadata.as_object_mut() {
        if kind != "audit" {
            meta.remove("acceptance");
        }
        if kind == "implement" {
            meta.remove("documents");
        }
        if let (Some(failure), Some(previous)) = (
            failure.as_deref(),
            meta.get_mut("previous_planning_failure")
                .and_then(serde_json::Value::as_object_mut),
        ) && previous.get("error").and_then(serde_json::Value::as_str) == Some(failure)
        {
            previous.remove("error");
            previous.insert("error_in".into(), json!("failure"));
        }
    }
    // OpenSpec apply instructions arrive as their whole JSON document; its task list and
    // context file map are already snapshot.tasks and snapshot.context_files.
    if kind == "implement"
        && let Ok(native) =
            serde_json::from_str::<serde_json::Value>(&snapshot.next_action.instruction)
        && let Some(text) = native
            .get("instruction")
            .and_then(serde_json::Value::as_str)
    {
        let mut instruction = text.to_owned();
        for warning in native["warnings"].as_array().into_iter().flatten() {
            if let Some(warning) = warning.as_str() {
                instruction.push_str(&format!("\nWarning: {warning}"));
            }
        }
        if let Some(context) = native["context"].as_str().filter(|c| !c.trim().is_empty()) {
            instruction.push_str(&format!("\n\nProject context:\n{context}"));
        }
        snapshot.next_action.instruction = instruction;
    }
}
pub(crate) fn clip(text: &str, limit: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    format!("{}…", text.chars().take(limit).collect::<String>())
}
fn listed(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        empty.to_owned()
    } else {
        values.join(", ")
    }
}
/// A short orientation per work kind; the input file remains the authority.
fn summary(input: &WorkerInput) -> String {
    let mut lines = vec![
        format!(
            "kind: {} | run_id: {} | task_id: {} | attempt_id: {} | base_commit: {}",
            input.kind, input.run_id, input.task_id, input.attempt_id, input.base_commit
        ),
        format!("instruction: {}", clip(&input.instruction, 6000)),
    ];
    let snapshot = input.snapshot.as_ref();
    match input.kind.as_str() {
        "implement" => {
            if let Some(task) = &input.task {
                lines.push(format!("task.id: {}", task.id));
                lines.push(format!(
                    "task.description: {}",
                    clip(&task.description, 4000)
                ));
                lines.push(format!(
                    "task.source_ids: {}",
                    listed(&task.source_ids, "none")
                ));
                lines.push(format!(
                    "task.depends_on: {}",
                    listed(&task.depends_on, "none")
                ));
                lines.push(format!(
                    "task.writes: {}",
                    listed(
                        &task.writes,
                        "unrestricted (empty: any file except native context)"
                    )
                ));
                lines.push(format!(
                    "task.reads: {}",
                    listed(&task.reads, "none declared")
                ));
                let checks: Vec<String> = task
                    .verification
                    .iter()
                    .map(|c| format!("`{}` (cwd {})", c.argv.join(" "), c.cwd))
                    .collect();
                lines.push(format!(
                    "task.verification: {}",
                    listed(&checks, "project-level checks only")
                ));
            }
            if let Some(snapshot) = snapshot {
                lines.push(format!(
                    "native instruction (snapshot.next_action.instruction): {}",
                    clip(&snapshot.next_action.instruction, 1500)
                ));
            }
            if let Some(previous) = &input.previous_candidate {
                lines.push(format!(
                    "previous_candidate: attempt {} changed {}; patch at {} (same_base: {}) — see PREVIOUS CANDIDATE below",
                    previous.attempt_id,
                    previous.files.join(", "),
                    previous.patch_path,
                    previous.same_base
                ));
            }
        }
        "plan-tasks" => {
            if let Some(snapshot) = snapshot {
                lines.push(format!(
                    "source: {} | source_hash: {}",
                    snapshot.selector, snapshot.source_hash
                ));
                lines.push(format!(
                    "snapshot.tasks ({}; cover every one):",
                    snapshot.tasks.len()
                ));
                for task in snapshot.tasks.iter().take(60) {
                    lines.push(format!(
                        "- {}{}: {}",
                        task.id,
                        if task.done { " (done)" } else { "" },
                        clip(&task.description, 160)
                    ));
                }
                if snapshot.tasks.len() > 60 {
                    lines.push(format!(
                        "- … {} more in input.json",
                        snapshot.tasks.len() - 60
                    ));
                }
            }
        }
        "audit" => {
            let acceptance = snapshot
                .and_then(|s| s.metadata["acceptance"].as_array())
                .cloned()
                .unwrap_or_default();
            lines.push(format!(
                "snapshot.metadata.acceptance ({}; one audit item per id):",
                acceptance.len()
            ));
            for item in acceptance.iter().take(60) {
                lines.push(format!(
                    "- {}: {}",
                    item["id"].as_str().unwrap_or_default(),
                    clip(item["description"].as_str().unwrap_or_default(), 160)
                ));
            }
            if acceptance.len() > 60 {
                lines.push(format!("- … {} more in input.json", acceptance.len() - 60));
            }
        }
        _ => {}
    }
    if let Some(snapshot) = snapshot {
        lines.push(format!(
            "native action: {} {} | outputs: {}",
            snapshot.next_action.kind,
            snapshot.next_action.artifact,
            listed(&snapshot.next_action.outputs, "none")
        ));
        let files: Vec<String> = snapshot
            .context_files
            .iter()
            .map(|f| f.path.clone())
            .collect();
        lines.push(format!(
            "native context files (full text in snapshot.context_files): {}",
            listed(&files, "none inline")
        ));
    }
    if let Some(failure) = &input.failure {
        lines.push(format!(
            "previous failure (full text in failure): {}",
            clip(failure, 1500)
        ));
    }
    lines.join("\n")
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
    let result = read_receipt(dir)?;
    validate_result(&read_input(dir)?, &result)?;
    Ok(result)
}
/// The stored receipt as submitted, before a blocked or failed status becomes an error.
pub fn read_receipt(dir: &Path) -> Result<WorkerResult> {
    if fs::metadata(dir.join("result.json"))?.len() > 64 * 1024 {
        bail!("worker_protocol_error: result exceeds 64 KiB");
    }
    serde_json::from_slice(&fs::read(dir.join("result.json"))?)
        .context("worker_protocol_error: invalid result JSON")
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
    let mut audit =
        object(json!({"requirement":string,"evidence":string,"passed":{"type":"boolean"}}));
    // Optional, so receipts written before incremental audits stay valid.
    audit["properties"]["files"] = json!({"type":"array","maxItems":50,"items":string,"description":"Project-relative implementation and test files this conclusion rests on. A passed item is carried into later audits only while none of them changed; omit it and the item is audited again every time."});
    let mut schema = object(
        json!({"schema_version":{"type":"integer","enum":[1]},"run_id":{"type":"string","enum":[input.run_id]},"task_id":{"type":"string","enum":[input.task_id]},"attempt_id":{"type":"string","enum":[input.attempt_id]},"status":{"type":"string","enum":["candidate","blocked","failed"]},"summary":string,"blockers":strings,"milestone":{"anyOf":[{"$ref":"#/$defs/milestone"},{"type":"null"}]},"plan":{"anyOf":[{"$ref":"#/$defs/plan"},{"type":"null"}]},"audit":{"type":"array","items":{"$ref":"#/$defs/audit"}}}),
    );
    schema["properties"]["requested_writes"] = json!({"type":"array","maxItems":crate::run_revision::MAX_REQUESTED_WRITES,"items":string,"description":"Only with status=blocked on implement work: project-relative files outside task.writes that the task needs (for example existing tests it breaks). The coordinator adds safe paths and re-issues the task; otherwise the run stops for review."});
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
