//! Read-only execution evidence for semantic audits. Raw logs remain on disk;
//! public metadata contains identities, hashes and explicit availability.
use crate::{
    model::{Evidence, Run},
    paths,
};
use anyhow::Result;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

fn reference(path: &Path) -> Result<Value> {
    if !path.is_file() {
        return Ok(json!({"availability":"unavailable"}));
    }
    let bytes = fs::read(path)?;
    Ok(
        json!({"availability":"recorded","path":path,"sha256":paths::hash(bytes),"bytes":fs::metadata(path)?.len()}),
    )
}
fn session(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    // Codex announces its session before command output. Never load full logs
    // into the coordinator prompt or parse unbounded lines for this header.
    let reader = BufReader::new(std::io::Read::take(file, 64 * 1024));
    for line in reader.lines().take(8).flatten() {
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            if value["type"] == "thread.started" {
                return value["thread_id"].as_str().map(str::to_owned);
            }
        }
    }
    None
}
/// Distinct checks listed in the packet's `execution_evidence.checks` digest.
pub const CHECK_DIGEST_LIMIT: usize = 40;

/// One verification record as an auditor needs it: what ran (declared `argv` and the
/// `executed_argv` actually started), where (`cwd`, and the plan's `declared_cwd`), at which `revision` and `stage`,
/// how it ended and how long it took, and the earlier pass it stands on when it was
/// reused rather than executed. The leading fields are the pre-existing ones.
fn check_record(e: &Evidence) -> Value {
    let detail = |key: &str| {
        e.details
            .as_ref()
            .and_then(|d| d.get(key))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let mut record = json!({
        "revision": e.revision,
        "exit_code": e.exit_code,
        "tree_unchanged": e.tree_unchanged,
        "log": e.log,
        "log_hash": e.log_hash,
        "argv": e.argv,
        "cwd": e.cwd,
        "stage": detail("stage"),
        "step": detail("step"),
        "duration_ms": detail("duration_ms"),
        "failure_kind": detail("failure_kind"),
        "wave_id": detail("wave_id"),
        "manifest": detail("manifest"),
        "finished_at": e.finished_at,
    });
    for key in ["declared_cwd", "executed_argv", "reused_from", "env_keys"] {
        let value = detail(key);
        if !value.is_null() {
            record[key] = value;
        }
    }
    record
}

/// The latest record of each distinct check (stage, step, argv, declared cwd), newest first,
/// with how many times it was recorded. Bounded by [`CHECK_DIGEST_LIMIT`].
fn check_digest(run: &Run) -> (Vec<Value>, usize) {
    let identity = |e: &Evidence| {
        let d = e.details.as_ref();
        (
            d.and_then(|d| d.get("stage")).map(Value::to_string),
            d.and_then(|d| d.get("step")).map(Value::to_string),
            e.argv.clone(),
            // The checkout differs per wave; the declared cwd names the same check.
            d.and_then(|d| d["declared_cwd"].as_str())
                .map_or_else(|| e.cwd.clone(), str::to_owned),
        )
    };
    let mut runs = std::collections::BTreeMap::new();
    for e in &run.evidence {
        *runs.entry(identity(e)).or_insert(0usize) += 1;
    }
    let mut digest = vec![];
    for e in run.evidence.iter().rev() {
        if let Some(count) = runs.remove(&identity(e)) {
            // The digest stays small enough to inline: logs and hashes are one hop away
            // in `manifest` and in the full list at `path`.
            let mut record = check_record(e);
            if let Some(map) = record.as_object_mut() {
                for key in ["log", "log_hash", "wave_id"] {
                    map.remove(key);
                }
            }
            if let Some(reused) = record.get_mut("reused_from").and_then(Value::as_object_mut) {
                reused.remove("changed_sample");
            }
            record["runs"] = json!(count);
            digest.push(record);
        }
    }
    let distinct = digest.len();
    digest.truncate(CHECK_DIGEST_LIMIT);
    (digest, distinct)
}

pub fn record(run: &Run, runtime: &Path, directory: &Path) -> Result<Value> {
    paths::valid_id(&run.id)?;
    let mut attempts = vec![];
    let mut sessions = BTreeSet::new();
    let mut session_count = 0;
    for a in run
        .attempts
        .iter()
        // Hooks and coordinator-run verify-only checks have no packet; their checks are
        // in `verification`.
        .filter(|a| {
            !matches!(a.kind.as_str(), "audit" | "hook" | "verify") && a.finished_at.is_some()
        })
    {
        paths::valid_id(&a.id)?;
        let dir = paths::inside(runtime, &format!("runs/{}/attempts/{}", run.id, a.id))?;
        let stdout = paths::inside(&dir, "stdout.log")?;
        let id = if run.config.runner.profile == "codex" {
            session(&stdout)
        } else {
            None
        };
        if let Some(id) = &id {
            session_count += 1;
            sessions.insert(id.clone());
        }
        attempts.push(json!({"attempt_id":a.id,"task_id":a.task_id,"phase_id":a.phase_id,"kind":a.kind,"status":a.status,"worktree":a.worktree,"base_commit":a.base_commit,"started_at":a.started_at,"finished_at":a.finished_at,"native_session_id":id,"host":run.host.as_ref().and_then(|h|h.requests.get(&a.id)).and_then(|r|r.owner.as_ref()),"input":reference(&paths::inside(&dir,"input.json")?)?,"result":reference(&paths::inside(&dir,"result.json")?)?,"execution_log":reference(&stdout)?}));
    }
    let host_sessions: Vec<_> = run
        .host
        .as_ref()
        .into_iter()
        .flat_map(|h| h.requests.values())
        .filter_map(|r| r.owner.as_ref())
        .map(|o| format!("{}/{}", o.host_id, o.session_id))
        .collect();
    let mut body = json!({"schema_version":1,"run_id":run.id,"framework":run.milestone.framework,"origin_head":run.origin_head,"accepted_head":run.accepted_head,"execution_model":if run.host.is_some(){"host-driven"}else{"legacy"},"host_sessions_declared":host_sessions.len(),"host_sessions_unique":host_sessions.iter().collect::<BTreeSet<_>>().len()==host_sessions.len(),"session_evidence_basis":if run.host.is_some(){"host-declaration"}else{"process-log"},"runner_profile":run.config.runner.profile,"fresh_session_contract":run.config.runner.profile=="codex"||run.config.runner.fresh_session,"native_session_ids_observed":session_count,"native_sessions_unique":session_count==attempts.len()&&sessions.len()==session_count,"attempts":attempts,"verification":run.evidence.iter().map(check_record).collect::<Vec<_>>()});
    body["verification_revisions"] = json!(run.host.as_ref().map(|h| &h.verification_revisions));
    let path = directory.join("execution-evidence.json");
    let bytes = serde_json::to_vec_pretty(&body)?;
    paths::atomic_write(&path, &bytes)?;
    let mut summary = json!({"schema_version":1,"run_id":run.id,"path":path,"sha256":paths::hash(&bytes),"attempt_count":attempts.len(),"verification_count":run.evidence.len(),"execution_model":if run.host.is_some(){"host-driven"}else{"legacy"},"host_sessions_declared":host_sessions.len(),"host_sessions_unique":host_sessions.iter().collect::<BTreeSet<_>>().len()==host_sessions.len(),"session_evidence_basis":if run.host.is_some(){"host-declaration"}else{"process-log"},"runner_profile":run.config.runner.profile,"accepted_head":run.accepted_head,"native_session_ids_observed":session_count,"native_sessions_unique":body["native_sessions_unique"]});
    // Added after the original fields; the packet's request key never includes them.
    let (checks, distinct) = check_digest(run);
    summary["checks"] = json!(checks);
    summary["checks_distinct"] = json!(distinct);
    summary["checks_truncated"] = json!(distinct > CHECK_DIGEST_LIMIT);
    Ok(summary)
}
