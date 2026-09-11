//! Read-only execution evidence for semantic audits. Raw logs remain on disk;
//! public metadata contains identities, hashes and explicit availability.
use crate::{model::Run, paths};
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
pub fn record(run: &Run, runtime: &Path, directory: &Path) -> Result<Value> {
    paths::valid_id(&run.id)?;
    let mut attempts = vec![];
    let mut sessions = BTreeSet::new();
    let mut session_count = 0;
    for a in run
        .attempts
        .iter()
        .filter(|a| a.kind != "audit" && a.finished_at.is_some())
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
        attempts.push(json!({"attempt_id":a.id,"task_id":a.task_id,"phase_id":a.phase_id,"kind":a.kind,"status":a.status,"worktree":a.worktree,"base_commit":a.base_commit,"started_at":a.started_at,"finished_at":a.finished_at,"native_session_id":id,"input":reference(&paths::inside(&dir,"input.json")?)?,"result":reference(&paths::inside(&dir,"result.json")?)?,"execution_log":reference(&stdout)?}));
    }
    let body = json!({"schema_version":1,"run_id":run.id,"framework":run.milestone.framework,"origin_head":run.origin_head,"accepted_head":run.accepted_head,"runner_profile":run.config.runner.profile,"fresh_session_contract":run.config.runner.profile=="codex"||run.config.runner.fresh_session,"native_session_ids_observed":session_count,"native_sessions_unique":session_count==attempts.len()&&sessions.len()==session_count,"attempts":attempts,"verification":run.evidence.iter().map(|e|json!({"revision":e.revision,"exit_code":e.exit_code,"tree_unchanged":e.tree_unchanged,"log":e.log,"log_hash":e.log_hash})).collect::<Vec<_>>()});
    let path = directory.join("execution-evidence.json");
    let bytes = serde_json::to_vec_pretty(&body)?;
    paths::atomic_write(&path, &bytes)?;
    Ok(
        json!({"schema_version":1,"run_id":run.id,"path":path,"sha256":paths::hash(&bytes),"attempt_count":attempts.len(),"verification_count":run.evidence.len(),"runner_profile":run.config.runner.profile,"accepted_head":run.accepted_head,"native_session_ids_observed":session_count,"native_sessions_unique":body["native_sessions_unique"]}),
    )
}
