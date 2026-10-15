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
    let native_milestones = declared_milestones(path, &runs, &mut diagnostics);
    let mut kinds = std::collections::BTreeMap::<String, usize>::new();
    for row in &worktrees {
        *kinds
            .entry(row["status"].as_str().unwrap_or("unknown").into())
            .or_default() += 1;
    }
    let capacity: usize = runs
        .iter()
        .filter(|r| !r.terminal())
        .map(|r| {
            r.config
                .execution
                .max_workers
                .min(r.config.host.max_concurrency)
        })
        .sum::<usize>()
        .max(1);
    // Primary + integration + review + workers + transient merge candidates,
    // with recovery allowance. This is a warning budget, never forced deletion.
    let budget = 1 + runs.iter().filter(|r| !r.terminal()).count() * 2 + capacity * 2 + 2;
    let backlog = worktrees.len().saturating_sub(budget);
    let resource_summary = json!({"worktrees":worktrees.len(),"by_status":kinds,"host_capacity":capacity,"expected_worktree_budget":budget,"retained_backlog":backlog,"pressure":if backlog > 0 {"high"} else {"normal"},"cleanup_hint":if backlog > 0 {Some("Review maintenance.json and run.cleanup with active:true at a coordinator-free boundary; ignored artifacts and active references are retained.")} else {None}});
    Ok(
        json!({"native_milestones":native_milestones,"resource_summary":resource_summary,"schema_version":1,"snapshot_id":paths::id("snapshot"),"generated_at":paths::now(),"repository_id":paths::hash(repo.common.to_string_lossy().as_bytes()),"consistency":if diagnostics.is_empty(){"consistent"}else{"partial"},"active_workers":active,"verified_tasks":done.len(),"usage":usage_summary(&runs),"worktrees":worktrees,"runs":summaries,"diagnostics":diagnostics}),
    )
}

fn usage_summary(runs: &[Run]) -> Value {
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut cached_input_tokens = 0u64;
    let mut any = false;
    for run in runs {
        for attempt in &run.attempts {
            let Some(usage) = &attempt.usage else {
                continue;
            };
            any = true;
            input_tokens = input_tokens.saturating_add(usage["input_tokens"].as_u64().unwrap_or(0));
            output_tokens =
                output_tokens.saturating_add(usage["output_tokens"].as_u64().unwrap_or(0));
            cached_input_tokens = cached_input_tokens
                .saturating_add(usage["cached_input_tokens"].as_u64().unwrap_or(0));
        }
    }
    if !any {
        return json!("unavailable");
    }
    json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "cached_input_tokens": cached_input_tokens
    })
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
    let mut attempt_states = std::collections::BTreeMap::<String, usize>::new();
    for attempt in &run.attempts {
        *attempt_states
            .entry(format!("{}:{}", attempt.kind, attempt.status))
            .or_default() += 1;
    }
    let measured: Vec<_> = run
        .evidence
        .iter()
        .filter_map(|e| e.details.as_ref()?.get("duration_ms")?.as_u64())
        .collect();
    let duration_ms = measured
        .iter()
        .fold(0u64, |total, next| total.saturating_add(*next));
    let unique_checkouts: std::collections::BTreeSet<_> =
        run.attempts.iter().map(|a| &a.worktree).collect();
    let overhead = json!({"verification_reuses":run.verification_reuses,"attempt_checkouts":unique_checkouts.len(),"read_only_attempts":run.attempts.iter().filter(|a|matches!(a.kind.as_str(),"plan-tasks"|"audit")).count(),"attempts_by_kind_status":attempt_states,"verification_checks":run.evidence.len(),"historical_failed_checks":run.evidence.iter().filter(|e|e.exit_code != 0).count(),"measured_checks":measured.len(),"measured_duration_ms":duration_ms,"duration_coverage":if measured.len()==run.evidence.len(){"complete"}else{"partial"},"verification_revisions":run.host.as_ref().map_or(0,|h|h.verification_revisions.len())});
    json!({"overhead":overhead,"run_id":run.id,"milestone_id":run.milestone.id,"status":run.status,"stage":run.stage,"selected_phases":run.selected_phases,"completed_phases":run.completed_phases,"verified_tasks":run.completed_tasks.len(),"attempts":run.attempts.len(),"blocker":run.blocker,"checkout":run.integration,"accepted_head":run.accepted_head,"next_action":if run.terminal(){String::new()}else{format!("spec-autonomous resume {}",run.id)}})
}
pub fn public_run(run: &Run) -> Value {
    let mut value = serde_json::to_value(run).unwrap();
    value.as_object_mut().unwrap().remove("host");
    value["verification_revisions"] = json!(run.host.as_ref().map(|h| &h.verification_revisions));
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

/// Read only declared native files. Never invoke a provider process for progress.
fn declared_milestones(root: &Path, runs: &[Run], diagnostics: &mut Vec<String>) -> Vec<Value> {
    let Ok(base) = paths::inside(root, ".spec-autonomous/milestones") else {
        return vec![];
    };
    let Ok(entries) = fs::read_dir(base) else {
        return vec![];
    };
    let mut names: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    if names.len() > 200 {
        diagnostics.push("native_milestones_truncated: first 200 declarations inspected".into());
        names.truncate(200);
    }
    names.into_iter().map(|id| {
        let result = (|| -> Result<Value> {
            paths::valid_id(&id)?;
            let content = paths::read(root, &format!(".spec-autonomous/milestones/{id}/milestone.toml"), 2*1024*1024)?;
            let m: crate::model::Milestone = toml::from_str(&content)?;
            let associated: Vec<_> = runs.iter().filter(|r|r.milestone.id==m.id).map(|r|r.id.clone()).collect();
            let phases: Vec<_> = m.phases.iter().take(128).map(|phase| {
                let file = if m.framework == crate::Framework::Openspec { format!("openspec/changes/{}/tasks.md", phase.source.selector) } else { format!("{}/tasks.md", phase.source.selector) };
                match paths::read(root, &file, 2*1024*1024).and_then(|s|crate::markdown::parse(&file, &s, m.framework)) {
                    Ok(doc) => json!({"phase_id":phase.id,"source_path":file,"availability":"native_file","native_checked":doc.tasks.iter().filter(|t|t.done).count(),"native_total":doc.tasks.len(),"verification":"not_inferred","diagnostics":doc.diagnostics}),
                    Err(_) => json!({"phase_id":phase.id,"source_path":file,"availability":"unavailable","verification":"not_inferred"}),
                }
            }).collect();
            Ok(json!({"milestone_id":m.id,"execution":if associated.is_empty(){"not_started"}else{"recorded"},"run_ids":associated,"phases":phases,"phase_count":m.phases.len(),"truncated":m.phases.len()>128}))
        })();
        result.unwrap_or_else(|e|json!({"milestone_id":id,"availability":"unavailable","error":e.to_string()}))
    }).collect()
}

pub fn with_related(root: &Path, args: &Value) -> Result<Value> {
    let mut primary = snapshot(root)?;
    let mut seen = BTreeSet::from([primary["repository_id"].as_str().unwrap().to_owned()]);
    let requested = args["repository_paths"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    anyhow::ensure!(
        requested.len() <= 16,
        "invalid_arguments: at most 16 related repositories"
    );
    let mut related = vec![];
    for path in requested {
        let path = path.as_str().ok_or_else(|| {
            anyhow::anyhow!("invalid_arguments: repository path must be a string")
        })?;
        let target = root.join(path);
        match snapshot(&target) {
            Ok(mut data) => {
                let id = data["repository_id"].as_str().unwrap().to_owned();
                if !seen.insert(id) {
                    continue;
                }
                crate::capabilities::views::select_page(
                    &mut data,
                    &json!({"limit":args.get("limit").cloned().unwrap_or(json!(50)),"offset":args.get("offset").cloned().unwrap_or(json!(0))}),
                )?;
                related.push(json!({"path":path,"availability":"available","progress":data}));
            }
            Err(error) => related
                .push(json!({"path":path,"availability":"unavailable","error":error.to_string()})),
        }
    }
    primary["related_repositories"] = json!(related);
    Ok(primary)
}
