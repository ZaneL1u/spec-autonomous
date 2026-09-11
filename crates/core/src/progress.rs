use crate::{
    git::{self, Repository},
    model::Run,
    paths, process,
    state::Store,
};
use anyhow::Result;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};

pub fn snapshot(path: &Path) -> Result<Value> {
    let repo = Repository::discover(path)?;
    let mut diagnostics = vec![];
    let first = repo.worktrees()?;
    let runs = match Store::open(&repo, false) {
        Ok(Some(db)) => match db.list() {
            Ok(r) => r,
            Err(e) => {
                diagnostics.push(e.to_string());
                vec![]
            }
        },
        Ok(None) => vec![],
        Err(e) => {
            diagnostics.push(e.to_string());
            vec![]
        }
    };
    let runs: Vec<_> = runs
        .into_iter()
        .filter(|run| {
            let valid = paths::valid_id(&run.id).is_ok()
                && run.attempts.iter().all(|a| paths::valid_id(&a.id).is_ok());
            if !valid {
                diagnostics.push(
                    "state_corrupt: invalid run or attempt identifier; record ignored".into(),
                );
            }
            valid
        })
        .collect();
    let runtime = repo.runtime()?;
    let registry = runtime.join("registry.toml");
    if registry.exists() {
        let parsed = paths::read(&repo.common, "spec-autonomous/registry.toml", 64 * 1024)
            .and_then(|s| Ok(toml::from_str::<toml::Value>(&s)?));
        match parsed {
            Ok(value) if value.get("ledger").and_then(toml::Value::as_str) == Some("state.db") => {}
            _ => diagnostics
                .push("registry_invalid: index ignored; fixed common-directory ledger used".into()),
        }
    }
    let mut active = 0usize;
    let mut worktrees = vec![];
    let mut done = BTreeSet::new();
    for run in &runs {
        for task in &run.completed_tasks {
            done.insert(format!("{}/{}", run.milestone.id, task));
        }
    }
    for wt in &first {
        let mut row = json!({"id":paths::hash(&wt.path),"path":wt.path,"head":wt.head,"branch":wt.branch,"locked":wt.locked,"prunable":wt.prunable,"kind":"external","status":"unknown"});
        if !Path::new(&wt.path).exists() {
            row["status"] = json!("prunable");
        }
        for run in &runs {
            if run.integration == wt.path {
                row["kind"] = json!("managed-integration");
                row["run_id"] = json!(run.id);
                row["milestone_id"] = json!(run.milestone.id);
                row["status"] = json!(run.status);
                row["accepted_head"] = json!(run.accepted_head);
                row["blocker"] = json!(run.blocker);
                break;
            }
            if let Some(intent) = run.intents.iter().find(|i| i.worktree == wt.path) {
                row["kind"] = json!("managed-candidate");
                row["run_id"] = json!(run.id);
                row["milestone_id"] = json!(run.milestone.id);
                row["status"] = json!(intent.state);
                row["accepted_head"] = json!(run.accepted_head);
                row["candidate_head"] = json!(intent.candidate_head);
                break;
            }
            if let Some(a) = run.attempts.iter().find(|a| a.worktree == wt.path) {
                row["kind"] = json!("managed-worker");
                row["run_id"] = json!(run.id);
                row["milestone_id"] = json!(run.milestone.id);
                row["phase_id"] = json!(a.phase_id);
                row["task_id"] = json!(a.task_id);
                row["attempt_id"] = json!(a.id);
                row["stage"] = json!(a.kind);
                row["status"] = json!(a.status);
                row["blocker"] = json!(a.error);
                row["accepted_head"] = json!(run.accepted_head);
                if let Some(lease) = run.host.as_ref().and_then(|h| h.requests.get(&a.id)) {
                    if matches!(a.status.as_str(), "issued" | "claimed" | "submitted")
                        && !lease.revoked
                    {
                        let age = (chrono::Utc::now().timestamp_millis().max(0) as u64)
                            .saturating_sub(lease.heartbeat_at_ms);
                        let stale = age > run.config.host.lease_seconds * 1000;
                        row["lease_state"] = json!(if stale {
                            "stale"
                        } else if a.status == "claimed" {
                            "host_reported"
                        } else {
                            "prepared"
                        });
                        row["host"] = json!(lease.owner);
                        row["heartbeat_age_ms"] = json!(age);
                        if stale {
                            row["status"] = json!("stale");
                        } else if a.status == "claimed" {
                            active += 1;
                        }
                        if run.terminal() {
                            row["host_action"] = json!("stop_and_acknowledge");
                        }
                    }
                } else if a.status == "running" {
                    let file = paths::inside(
                        &runtime,
                        &format!("runs/{}/attempts/{}/process.json", run.id, a.id),
                    );
                    let live = file
                        .ok()
                        .and_then(|file| fs::read(file).ok())
                        .and_then(|b| serde_json::from_slice::<process::Identity>(&b).ok())
                        .is_some_and(|p| {
                            !p.identity.is_empty() && process::identity(p.pid) == p.identity
                        });
                    row["lease_state"] = json!(if live { "live" } else { "stale" });
                    if live {
                        active += 1;
                    } else {
                        row["status"] = json!("stale");
                    }
                }
                break;
            }
        }
        worktrees.push(row);
    }
    let second = repo.worktrees()?;
    if first.iter().map(|w| (&w.path, &w.head)).collect::<Vec<_>>()
        != second
            .iter()
            .map(|w| (&w.path, &w.head))
            .collect::<Vec<_>>()
    {
        diagnostics.push("inventory_changed_during_snapshot".into());
    }
    let summaries: Vec<_> = runs
        .iter()
        .map(|run| {
            let mut value = summary(run);
            value["native_progress"] = native_progress(&runtime, run, &mut diagnostics);
            value
        })
        .collect();
    Ok(
        json!({"schema_version":1,"snapshot_id":paths::id("snapshot"),"generated_at":paths::now(),"repository_id":paths::hash(repo.common.to_string_lossy().as_bytes()),"consistency":if diagnostics.is_empty(){"consistent"}else{"partial"},"active_workers":active,"verified_tasks":done.len(),"usage":"unavailable","worktrees":worktrees,"runs":summaries,"diagnostics":diagnostics}),
    )
}
fn native_progress(runtime: &Path, run: &Run, diagnostics: &mut Vec<String>) -> Value {
    let mut views = vec![];
    for phase in &run.milestone.phases {
        let mut view = json!({"phase_id":phase.id,"availability":"unavailable","verified":run.completed_phases.contains(&phase.id)});
        for attempt in run.attempts.iter().rev().filter(|a| a.phase_id == phase.id) {
            let relative = format!("runs/{}/attempts/{}/input.json", run.id, attempt.id);
            let data = paths::read(runtime, &relative, 512 * 1024);
            let Ok(text) = data else {
                continue;
            };
            match serde_json::from_str::<crate::model::WorkerInput>(&text) {
                Ok(mut input) => {
                    if let Some(reference) = input
                        .snapshot
                        .as_ref()
                        .and_then(|s| s.metadata.get("context_reference"))
                    {
                        let full =
                            format!("runs/{}/attempts/{}/context-full.json", run.id, attempt.id);
                        match paths::read(runtime, &full, 16 * 1024 * 1024) {
                            Ok(text)
                                if reference["sha256"].as_str() == Some(&paths::hash(&text)) =>
                            {
                                match serde_json::from_str(&text) {
                                    Ok(full) => input = full,
                                    Err(_) => {
                                        diagnostics.push("invalid_context_reference".into());
                                        break;
                                    }
                                }
                            }
                            _ => {
                                diagnostics.push("invalid_context_reference".into());
                                break;
                            }
                        }
                    }
                    if let Some(snapshot) = input.snapshot {
                        if snapshot.metadata["scope"] == "milestone" {
                            continue;
                        }
                        view["availability"] = json!("recorded");
                        view["observed_at"] = json!(attempt.started_at);
                        view["source_revision"] = json!(snapshot.source_hash);
                        view["native_total"] = snapshot.metadata["native_counts"]["total"]
                            .as_u64()
                            .map_or_else(|| json!(snapshot.tasks.len()), |v| json!(v));
                        view["native_checked"] = snapshot.metadata["native_counts"]["checked"]
                            .as_u64()
                            .map_or_else(
                                || json!(snapshot.tasks.iter().filter(|t| t.done).count()),
                                |v| json!(v),
                            );
                        break;
                    }
                }
                Err(_) => {
                    diagnostics.push("invalid_context_packet: native progress unavailable".into());
                    break;
                }
            }
        }
        views.push(view);
    }
    json!(views)
}
pub fn summary(run: &Run) -> Value {
    json!({"run_id":run.id,"milestone_id":run.milestone.id,"status":run.status,"stage":run.stage,"selected_phases":run.selected_phases,"completed_phases":run.completed_phases,"verified_tasks":run.completed_tasks.len(),"attempts":run.attempts.len(),"blocker":run.blocker,"checkout":run.integration,"accepted_head":run.accepted_head,"next_action":if run.terminal(){String::new()}else{format!("spec-autonomous resume {}",run.id)}})
}
pub fn public_run(run: &Run) -> Value {
    let mut value = serde_json::to_value(run).unwrap();
    value.as_object_mut().unwrap().remove("host");
    value["execution_model"] = json!(if run.host.is_some() {
        "host-driven"
    } else {
        "legacy"
    });
    value["config"] = json!({"schema_version":run.config.schema_version,"execution":run.config.execution,"host":run.config.host,"environment":"redacted","runner":{"profile":run.config.runner.profile,"sandbox":run.config.runner.sandbox,"environment":"redacted"}});
    value
}
/// Remove null fields for the common JSON/TOML public representation.
pub fn portable(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            for v in map.values_mut() {
                portable(v);
            }
        }
        Value::Array(array) => {
            array.retain(|v| !v.is_null());
            for v in array {
                portable(v);
            }
        }
        _ => (),
    }
}
pub fn human(value: &Value) -> String {
    if value
        .get("data")
        .is_some_and(|v| v.get("worktrees").is_some())
    {
        return human(&value["data"]);
    }
    if let Some(rows) = value.get("worktrees").and_then(Value::as_array) {
        let mut out = format!(
            "Worktrees: {} | active workers: {} | verified tasks: {} | consistency: {}\n",
            rows.len(),
            value["active_workers"],
            value["verified_tasks"],
            value["consistency"].as_str().unwrap_or("unknown")
        );
        for row in rows {
            out.push_str(&format!(
                "{}  {}  {}  {}\n",
                row["kind"].as_str().unwrap_or(""),
                row["status"].as_str().unwrap_or(""),
                row["phase_id"].as_str().unwrap_or(""),
                row["path"].as_str().unwrap_or("")
            ));
        }
        if let Some(ds) = value.get("diagnostics").and_then(Value::as_array) {
            for diagnostic in ds {
                out.push_str(&format!(
                    "Diagnostic: {}\n",
                    diagnostic.as_str().unwrap_or("unknown")
                ));
            }
        }
        out
    } else {
        serde_json::to_string_pretty(value).unwrap_or_default()
    }
}
pub fn readonly_git_status(root: &Path) -> Result<String> {
    git::command(root, &["status", "--porcelain"], None)
}
