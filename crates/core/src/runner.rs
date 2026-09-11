use crate::{config::Config, model::*, paths, process};
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

pub fn doctor(config: &Config) -> Result<serde_json::Value> {
    config.validate()?;
    if config.runner.profile == "command" {
        if config.runner.command.is_empty() || !config.runner.fresh_session {
            bail!(
                "runner_not_configured: configure command argv and fresh_session=true in config.toml"
            );
        }
        if config
            .runner
            .command
            .iter()
            .any(|a| ["resume", "--resume", "--last"].contains(&a.as_str()))
        {
            bail!("runner_not_fresh: persistent resume arguments are forbidden");
        }
    }
    if config.runner.profile == "codex" {
        let executable = config
            .runner
            .command
            .first()
            .map(String::as_str)
            .unwrap_or("codex");
        let result = std::process::Command::new(executable)
            .args(["exec", "--help"])
            .output()
            .context("runner_unavailable: codex not found")?;
        let help = String::from_utf8_lossy(&result.stdout);
        if !result.status.success()
            || !["--ephemeral", "--output-last-message", "--sandbox"]
                .iter()
                .all(|s| help.contains(s))
        {
            bail!("runner_unsupported: codex CLI lacks required exec capabilities");
        }
    }
    Ok(
        json!({"profile":config.runner.profile,"fresh_session":true,"transport":"input-json/result-json","sandbox":config.runner.sandbox,"usage":"unavailable","platform":std::env::consts::OS}),
    )
}
pub fn run(
    input: &WorkerInput,
    root: &Path,
    dir: &Path,
    config: &Config,
    cancel: Arc<AtomicBool>,
) -> Result<WorkerResult> {
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
        "You are a fresh Spec Autonomous worker. Your task is the JSON request below. Follow its native SDD instructions and repository rules. Work only inside the assigned checkout. If context_reference is present, read the immutable complete request before acting. Do not start other autonomous coordinators, write shared task checkboxes, commit, push or publish. The coordinator owns native extension hooks: do not invoke hooks yourself, including hooks mentioned by native skills. Native planning may create only snapshot.next_action.outputs; Spec Kit feature.json may be updated locally by the native command but will be restored before integration. Return a single JSON object matching WorkerResult: schema_version=1, run_id/task_id/attempt_id copied exactly, status=candidate|blocked|failed, summary<=8192 bytes, blockers array, milestone or null, plan or null, audit array. Roadmap work returns milestone; task planning returns plan; verification returns requirement/evidence/passed audit items. Never claim integrated/verified. Only implementation/native-planning/converge work may edit allowed files.\n\nREQUEST:\n{}",
        String::from_utf8_lossy(&data)
    );
    if prompt.len() > budget {
        bail!("context_too_large: prompt exceeds configured byte budget");
    }
    paths::atomic_write(&dir.join("prompt.md"), &prompt)?;
    let mut env = config.runner.environment.clone();
    env.insert(
        "SPEC_AUTONOMOUS_INPUT".into(),
        input_file.to_string_lossy().into(),
    );
    env.insert(
        "SPEC_AUTONOMOUS_RESULT".into(),
        result_file.to_string_lossy().into(),
    );
    env.insert("SPEC_AUTONOMOUS_ATTEMPT".into(), input.attempt_id.clone());
    if input.framework == crate::Framework::Speckit {
        env.insert("SPECIFY_INIT_DIR".into(), root.to_string_lossy().into());
        if let Some(snapshot) = &input.snapshot {
            env.insert(
                "SPECIFY_FEATURE_DIRECTORY".into(),
                paths::inside(root, &snapshot.selector)?
                    .to_string_lossy()
                    .into(),
            );
            if let Some(label) = Path::new(&snapshot.selector).file_name() {
                env.insert("SPECIFY_FEATURE".into(), label.to_string_lossy().into());
            }
        }
    }
    let argv = if config.runner.profile == "codex" {
        let schema = dir.join("result.schema.json");
        paths::atomic_write(&schema, serde_json::to_vec_pretty(&result_schema(input))?)?;
        let mut args = vec![
            config
                .runner
                .command
                .first()
                .cloned()
                .unwrap_or_else(|| "codex".into()),
            "exec".into(),
            "--ephemeral".into(),
            "--json".into(),
            "--sandbox".into(),
            if matches!(input.kind.as_str(), "roadmap" | "plan-tasks" | "audit") {
                "read-only".into()
            } else {
                "workspace-write".into()
            },
            "--output-schema".into(),
            schema.to_string_lossy().into(),
            "--output-last-message".into(),
            result_file.to_string_lossy().into(),
            "-".into(),
        ];
        if config.runner.command.len() > 1 {
            bail!(
                "invalid_config: codex command contains only the executable; configure model in native Codex settings"
            );
        }
        args.shrink_to_fit();
        args
    } else {
        config
            .runner
            .command
            .iter()
            .map(|x| {
                x.replace("{input}", &input_file.to_string_lossy())
                    .replace("{result}", &result_file.to_string_lossy())
            })
            .collect()
    };
    let output = process::execute(process::Request {
        argv: &argv,
        cwd: root,
        env: &env,
        stdin: Some(prompt.as_bytes()),
        directory: dir,
        timeout: Duration::from_secs(config.execution.attempt_timeout_seconds),
        max_log_bytes: config.execution.max_log_bytes,
        cancel,
    })?;
    if output.cancelled {
        bail!("cancelled: worker cancelled");
    }
    if output.timed_out {
        bail!("attempt_timeout: worker timed out");
    }
    if output.code != 0 {
        let stderr = fs::read_to_string(dir.join("stderr.log")).unwrap_or_default();
        bail!(
            "worker_failed: exit {}: {}",
            output.code,
            stderr.chars().take(2048).collect::<String>()
        );
    }
    if !result_file.is_file() {
        bail!("worker_protocol_error: missing result.json");
    }
    if fs::metadata(&result_file)?.len() > 64 * 1024 {
        bail!("worker_protocol_error: result exceeds 64 KiB");
    }
    let result: WorkerResult = serde_json::from_slice(&fs::read(&result_file)?)
        .context("worker_protocol_error: invalid result JSON")?;
    validate_result(input, &result)?;
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
