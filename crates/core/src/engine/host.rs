//! Host-owned semantic work. All process execution remains outside this module.
use super::*;
use serde_json::{Value, json};

fn now_ms() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}
fn host_state(run: &Run) -> Result<&HostState> {
    if run.host.as_ref().is_some_and(|h| h.protocol_version != 1) {
        bail!("host_protocol_unsupported");
    }
    run.host
        .as_ref()
        .context("legacy_run_read_only: import the native source into a host-driven run")
}
fn pending(status: &str) -> bool {
    matches!(status, "issued" | "claimed" | "submitted" | "receiving")
}
fn own(run: &Run, attempt: &str, token: &str) -> Result<usize> {
    let meta = host_state(run)?
        .requests
        .get(attempt)
        .context("request_not_found")?;
    if meta.token != token || meta.revoked {
        bail!("request_ownership_mismatch: invalid or revoked token");
    }
    run.attempts
        .iter()
        .position(|a| a.id == attempt)
        .context("request_not_found")
}
fn validate_owner(run: &Run, attempt: &str, owner: &HostIdentity) -> Result<()> {
    paths::valid_id(&owner.host_id)?;
    paths::valid_id(&owner.session_id)?;
    if !owner.fresh_context {
        bail!("fresh_context_required: host must allocate a new context for each request");
    }
    for (id, lease) in &host_state(run)?.requests {
        if id != attempt
            && lease
                .owner
                .as_ref()
                .is_some_and(|o| o.host_id == owner.host_id && o.session_id == owner.session_id)
        {
            bail!("host_session_reused: each work unit requires a distinct host session");
        }
    }
    if host_state(run)?.requests[attempt]
        .owner
        .as_ref()
        .is_some_and(|o| o != owner)
    {
        bail!("request_ownership_mismatch: host identity differs");
    }
    Ok(())
}

pub fn work_context(root: &Path, run_id: &str, attempt_id: &str) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let store = Store::open(&repo, false)?.context("run_not_found")?;
    let run = store.get(run_id)?;
    let lease = host_state(&run)?
        .requests
        .get(attempt_id)
        .context("request_not_found")?;
    let dir = paths::inside(&store.root, &format!("runs/{run_id}/attempts/{attempt_id}"))?;
    let input = fs::read(dir.join("input.json"))?;
    if paths::hash(&input) != lease.input_hash {
        bail!("context_corrupt: immutable packet changed");
    }
    Ok(serde_json::to_value(work_packet::read_input(&dir)?)?)
}
pub fn work_view(run: &Run, runtime: &Path) -> Result<Value> {
    let mut data = crate::progress::public_run(run);
    let mut work = vec![];
    if let Some(host) = &run.host {
        for a in run.attempts.iter().filter(|a| pending(&a.status)) {
            let Some(lease) = host.requests.get(&a.id) else {
                continue;
            };
            if lease.revoked {
                continue;
            }
            let dir = paths::inside(runtime, &format!("runs/{}/attempts/{}", run.id, a.id))?;
            let bytes = fs::read(dir.join("input.json"))?;
            if paths::hash(&bytes) != lease.input_hash {
                bail!("context_corrupt: work packet changed");
            }
            let input: Value = serde_json::from_slice(&bytes)?;
            let project = Path::new(&a.worktree).join(&run.project_relative);
            let mut hints = json!({});
            if run.milestone.framework == Framework::Speckit {
                hints["SPECIFY_INIT_DIR"] = json!(project);
                if let Some(selector) = input.pointer("/snapshot/selector").and_then(Value::as_str)
                {
                    hints["SPECIFY_FEATURE_DIRECTORY"] = json!(paths::inside(&project, selector)?);
                    hints["SPECIFY_FEATURE"] =
                        json!(Path::new(selector).file_name().and_then(|s| s.to_str()));
                }
            }
            work.push(json!({"request_id":a.id,"run_id":run.id,"kind":a.kind,"task_id":a.task_id,"phase_id":a.phase_id,"status":a.status,"token":lease.token,"input_hash":lease.input_hash,"worktree":a.worktree,"project":Path::new(&a.worktree).join(&run.project_relative),"base_commit":a.base_commit,"input_path":dir.join("input.json"),"prompt_path":dir.join("prompt.md"),"result_schema_path":dir.join("result.schema.json"),"result_path":dir.join("host-result.json"),"environment_hints":hints,"requirements":{"fresh_context":true,"host_owned_execution":true},"limits":{"attempt_timeout_seconds":run.config.execution.attempt_timeout_seconds,"run_remaining_ms":(run.config.execution.run_timeout_seconds*1000).saturating_sub(elapsed(run)),"max_result_bytes":65536},"stale":now_ms().saturating_sub(lease.heartbeat_at_ms)>run.config.host.lease_seconds*1000,"owner":lease.owner}));
        }
    }
    data["work"] = json!(work);
    data["starts_agents"] = json!(false);
    data["host_action"] = json!(match run.status.as_str() {
        "awaiting_host" => "execute_prepared_work",
        "paused" => "inspect_or_resume",
        "cancelled" => "stop_outstanding_host_work",
        "needs_input" => "resolve_blocker",
        _ => "observe",
    });
    Ok(data)
}
pub fn next(root: &Path, id: Option<&str>) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let Some(store) = Store::open(&repo, false)? else {
        return Ok(json!({"action":"prepare","reason":"no_run","starts_agents":false}));
    };
    let run = if let Some(id) = id {
        store.get(id)?
    } else {
        store.list()?.into_iter().next().context("run_not_found")?
    };
    let action = if run.attempts.iter().any(|a| pending(&a.status)) {
        if run.terminal() {
            "stop_host_work"
        } else {
            "host_work"
        }
    } else if run.terminal() {
        "done"
    } else if run.status == "handed_off" {
        "native_handoff"
    } else {
        "prepare"
    };
    let mut value = work_view(&run, &store.root)?;
    // Read-only previews never hand out ownership tokens; prepare/claim owns them.
    for w in value["work"].as_array_mut().into_iter().flatten() {
        w.as_object_mut().unwrap().remove("token");
    }
    Ok(
        json!({"action":action,"run_id":run.id,"status":run.status,"stage":run.stage,"blocker":run.blocker,"work":value["work"],"run_remaining_ms":(run.config.execution.run_timeout_seconds*1000).saturating_sub(elapsed(&run)),"starts_agents":false}),
    )
}
pub fn claim(
    root: &Path,
    run_id: &str,
    attempt_id: &str,
    token: &str,
    owner: HostIdentity,
) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let _lock = Lease::acquire(&repo)?;
    let mut store = Store::open(&repo, true)?.unwrap();
    let mut run = store.get(run_id)?;
    let index = own(&run, attempt_id, token)?;
    validate_owner(&run, attempt_id, &owner)?;
    validate_global_owner(&store, &run.id, &owner)?;
    if run.terminal() || !matches!(run.attempts[index].status.as_str(), "issued" | "claimed") {
        bail!("request_not_claimable");
    }
    let meta = run
        .host
        .as_mut()
        .unwrap()
        .requests
        .get_mut(attempt_id)
        .unwrap();
    meta.owner = Some(owner);
    meta.heartbeat_at_ms = now_ms();
    run.attempts[index].status = "claimed".into();
    run.updated_at = paths::now();
    store.save(&run, "host_claimed")?;
    work_view(&run, &store.root)
}
pub fn heartbeat(root: &Path, run_id: &str, attempt_id: &str, token: &str) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let _lock = Lease::acquire(&repo)?;
    let mut store = Store::open(&repo, true)?.unwrap();
    let mut run = store.get(run_id)?;
    let index = own(&run, attempt_id, token)?;
    if !matches!(run.attempts[index].status.as_str(), "issued" | "claimed") {
        bail!("request_not_active");
    }
    run.host
        .as_mut()
        .unwrap()
        .requests
        .get_mut(attempt_id)
        .unwrap()
        .heartbeat_at_ms = now_ms();
    run.updated_at = paths::now();
    store.save(&run, "host_heartbeat")?;
    Ok(
        json!({"run_id":run_id,"request_id":attempt_id,"host_action":if run.terminal(){"stop"}else if store.requested(run_id)?=="pause"{"pause"}else{"continue"}}),
    )
}
pub fn revoke(
    root: &Path,
    run_id: &str,
    attempt_id: &str,
    token: &str,
    stopped: bool,
    reason: &str,
) -> Result<Value> {
    if !stopped || reason.trim().is_empty() {
        bail!("host_stop_confirmation_required: acknowledge stopped work and provide a reason");
    }
    let repo = Repository::discover(root)?;
    let _lock = Lease::acquire(&repo)?;
    let mut store = Store::open(&repo, true)?.unwrap();
    let mut run = store.get(run_id)?;
    let index = own(&run, attempt_id, token)?;
    if !pending(&run.attempts[index].status) {
        bail!("request_not_active");
    }
    run.host
        .as_mut()
        .unwrap()
        .requests
        .get_mut(attempt_id)
        .unwrap()
        .revoked = true;
    run.attempts[index].status = "revoked".into();
    run.attempts[index].error = Some(reason.into());
    run.attempts[index].finished_at = Some(paths::now());
    run.updated_at = paths::now();
    store.save(&run, "host_revoked")?;
    Ok(json!({"run_id":run_id,"request_id":attempt_id,"revoked":true}))
}
pub fn apply_result(
    root: &Path,
    token: &str,
    result: WorkerResult,
    owner: HostIdentity,
    cancel: Arc<AtomicBool>,
    observer: Observer<'_>,
) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let mut replay = false;
    {
        let _lock = Lease::acquire(&repo)?;
        let mut store = Store::open(&repo, true)?.unwrap();
        let mut run = store.get(&result.run_id)?;
        let index = own(&run, &result.attempt_id, token)?;
        validate_owner(&run, &result.attempt_id, &owner)?;
        validate_global_owner(&store, &run.id, &owner)?;
        let bytes = serde_json::to_vec(&result)?;
        if bytes.len() > 64 * 1024 {
            bail!("worker_protocol_error: result exceeds 64 KiB");
        }
        let hash = paths::hash(&bytes);
        let meta = host_state(&run)?.requests[&result.attempt_id].clone();
        if meta.receipt_hash.is_none()
            && now_ms().saturating_sub(meta.heartbeat_at_ms) > run.config.host.lease_seconds * 1000
        {
            bail!("lease_stale: refresh host liveness before submitting this result");
        }
        if let Some(old) = &meta.receipt_hash {
            if old != &hash {
                bail!("receipt_conflict: request already has a different result");
            }
            replay = true;
            if !matches!(
                run.attempts[index].status.as_str(),
                "receiving" | "submitted"
            ) {
                let mut view = work_view(&run, &store.root)?;
                view["receipt_replayed"] = json!(true);
                return Ok(view);
            }
        }
        if run.terminal()
            || store.requested(&run.id)? == "cancel"
            || !matches!(
                run.attempts[index].status.as_str(),
                "issued" | "claimed" | "receiving" | "submitted"
            )
        {
            bail!("request_not_active");
        }
        let dir = store.attempt_dir(&run.id, &result.attempt_id)?;
        if paths::hash(fs::read(dir.join("input.json"))?) != meta.input_hash {
            bail!("context_corrupt: immutable packet changed");
        }
        let input = work_packet::read_input(&dir)?;
        if input.run_id != result.run_id
            || input.attempt_id != result.attempt_id
            || input.task_id != result.task_id
            || result.schema_version != SCHEMA
            || result.summary.len() > 8192
            || !["candidate", "blocked", "failed"].contains(&result.status.as_str())
        {
            bail!("worker_protocol_error: result identity or shape is invalid");
        }
        if !replay && dir.join("result.json").exists() {
            bail!("receipt_untracked: host must write its output outside the canonical receipt");
        }
        if !replay {
            let meta = run
                .host
                .as_mut()
                .unwrap()
                .requests
                .get_mut(&result.attempt_id)
                .unwrap();
            meta.owner = Some(owner);
            meta.receipt_hash = Some(hash.clone());
            meta.heartbeat_at_ms = now_ms();
            run.attempts[index].status = "receiving".into();
            run.updated_at = paths::now();
            store.save(&run, "receipt_intent")?;
            failpoint("before_receipt_file");
        }
        if dir.join("result.json").exists() {
            if paths::hash(fs::read(dir.join("result.json"))?) != hash {
                bail!("receipt_corrupt: canonical receipt differs from its recorded intent");
            }
        } else {
            paths::atomic_write(&dir.join("result.json"), &bytes)?;
        }
        failpoint("after_receipt_file");
        run.attempts[index].status = "submitted".into();
        run.attempts[index].finished_at = Some(paths::now());
        run.updated_at = paths::now();
        store.save(&run, "host_result_submitted")?;
        if store.requested(&run.id)? == "pause" {
            return work_view(&run, &store.root);
        }
    }
    let run = resume_with(
        root,
        &result.run_id,
        ResumeOptions::default(),
        cancel,
        observer,
    )?;
    let mut result = work_view(&run, &repo.runtime()?)?;
    if replay {
        result["receipt_replayed"] = json!(true);
    }
    Ok(result)
}
fn validate_global_owner(store: &Store, current: &str, owner: &HostIdentity) -> Result<()> {
    for run in store.list()?.iter().filter(|r| r.id != current) {
        if run.host.as_ref().is_some_and(|h| {
            h.requests.values().any(|l| {
                l.owner
                    .as_ref()
                    .is_some_and(|o| o.host_id == owner.host_id && o.session_id == owner.session_id)
            })
        }) {
            bail!("host_session_reused: session identity already belongs to another run");
        }
    }
    Ok(())
}
pub fn host_control(root: &Path, id: &str, action: &str) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let store = Store::for_control(&repo)?;
    let run = store.get(id)?;
    store.control(id, action)?;
    drop(store);
    if let Ok(_lock) = Lease::acquire(&repo) {
        let mut store = Store::open(&repo, true)?.unwrap();
        let mut changed = store.get(id)?;
        if !changed.terminal() {
            changed.status = if action == "cancel" {
                "cancelled"
            } else {
                "paused"
            }
            .into();
            changed.blocker = Some(format!(
                "{}: requested by host",
                if action == "pause" { "paused" } else { action }
            ));
            changed.updated_at = paths::now();
            store.save(&changed, "host_control")?;
        }
    }

    // Control writes are a mailbox so they can interrupt a verifier holding the
    // coordinator lock. The host remains responsible for its own agent sessions.
    Ok(
        json!({"run_id":id,"requested":action,"host_actions":run.attempts.iter().filter(|a|pending(&a.status)).map(|a|json!({"request_id":a.id,"action":action})).collect::<Vec<_>>(),"starts_agents":false}),
    )
}
impl Coordinator<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn issue(
        &mut self,
        kind: &str,
        task: &str,
        phase: &str,
        instruction: String,
        snapshot: Option<Snapshot>,
        task_spec: Option<Task>,
        key: String,
    ) -> Result<usize> {
        let (index, worktree, dir) = self.make_attempt(kind, task, phase)?;
        let mut snapshot = snapshot;
        if kind == "audit" {
            if let Some(s) = snapshot.as_mut() {
                s.metadata["execution_evidence"] =
                    crate::provenance::record(&self.run, &self.store.root, &dir)?;
            }
        }
        let failure = self.run.attempts[..index]
            .iter()
            .rev()
            .find(|a| {
                a.kind == kind && a.task_id == task && a.phase_id == phase && a.error.is_some()
            })
            .and_then(|a| a.error.clone());
        let input = self.input(index, instruction, snapshot, task_spec, failure);
        work_packet::prepare(
            &input,
            &worktree.join(&self.run.project_relative),
            &dir,
            &self.run.config,
        )?;
        self.run.attempts[index].status = "issued".into();
        self.run
            .host
            .as_mut()
            .context("legacy_run_read_only")?
            .requests
            .insert(
                self.run.attempts[index].id.clone(),
                WorkLease {
                    token: paths::id("lease"),
                    input_hash: paths::hash(fs::read(dir.join("input.json"))?),
                    request_key: key,
                    issued_at_ms: now_ms(),
                    heartbeat_at_ms: now_ms(),
                    owner: None,
                    receipt_hash: None,
                    revoked: false,
                },
            );
        self.save("work_prepared")?;
        Ok(index)
    }
    pub(super) fn invoke_host(
        &mut self,
        kind: &str,
        task: &str,
        phase: &str,
        instruction: String,
        snapshot: Option<Snapshot>,
    ) -> Result<(WorkerResult, PathBuf, usize)> {
        let key = paths::hash(serde_json::to_vec(&(
            kind,
            task,
            phase,
            &self.run.accepted_head,
            &self.run.milestone.goal,
            &instruction,
            &snapshot,
        ))?);
        let host = host_state(&self.run)?;
        for (index, a) in self.run.attempts.iter().enumerate().rev() {
            if host
                .requests
                .get(&a.id)
                .is_some_and(|m| m.request_key == key && !m.revoked)
            {
                if matches!(a.status.as_str(), "issued" | "claimed") {
                    bail!("awaiting_host: semantic work is prepared");
                }
                if matches!(a.status.as_str(), "submitted" | "accepted" | "candidate") {
                    let dir = paths::inside(
                        &self.store.root,
                        &format!("runs/{}/attempts/{}", self.run.id, a.id),
                    )?;
                    let worktree = PathBuf::from(&a.worktree);
                    let result = work_packet::read_result(&dir);
                    if a.status == "submitted" {
                        self.end_attempt(index, &result)?;
                    }
                    return result.map(|r| (r, worktree, index));
                }
            }
        }
        let failures = self
            .run
            .attempts
            .iter()
            .filter(|a| {
                a.kind == kind && a.task_id == task && a.phase_id == phase && a.status == "failed"
            })
            .count();
        if failures >= self.run.config.execution.max_attempts as usize {
            bail!("attempts_exhausted: host work failed repeatedly");
        }
        self.issue(kind, task, phase, instruction, snapshot, None, key)?;
        bail!("awaiting_host: semantic work is prepared")
    }
}
