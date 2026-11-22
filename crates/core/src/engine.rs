//! Deterministic coordinator. Agent results are proposals; only this module can
//! accept a Git revision, advance dependencies and write native completion state.
use crate::{
    Framework,
    config::Config,
    git::{self, Repository},
    model::*,
    paths, plan, process, provider,
    state::{Lease, Store},
    work_packet,
};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

mod baseline;
mod execution;
mod host;
mod lifecycle;
mod maintenance;
mod planning;
mod recovery;
pub use host::{
    ClaimRequest, apply_result, auto_claim, claim, claim_batch, derived_identity, heartbeat,
    host_control, next, request_get, revoke, work_context, work_view, work_view_with,
};

pub type Observer<'a> = &'a mut dyn FnMut(&Run, &str);
pub struct Start {
    pub milestone: Milestone,
    pub range: Range,
    pub mode: String,
    pub create_roadmap: bool,
    pub plan: Option<Plan>,
}
#[derive(Default)]
pub struct ResumeOptions {
    pub mode: Option<String>,
    pub reload_config: bool,
    pub extend_seconds: u64,
    pub max_attempts: Option<u32>,
}
struct Coordinator<'a> {
    repo: Repository,
    store: Store,
    run: Run,
    cancel: Arc<AtomicBool>,
    observer: Observer<'a>,
    started: Instant,
    elapsed_before: u64,
}

pub fn source_milestone(framework: Framework, selector: &str) -> Milestone {
    Milestone {
        schema_version: SCHEMA,
        id: format!("source-{}", &paths::hash(selector)[..12]),
        goal: format!("Complete the existing native source {selector}"),
        framework,
        revision: 1,
        phases: vec![Phase {
            id: "P001".into(),
            label: "1".into(),
            title: selector.into(),
            depends_on: vec![],
            workstream: None,
            owner: None,
            external_ids: vec![],
            source: Source {
                kind: if framework == Framework::Openspec {
                    "openspec-change"
                } else {
                    "speckit-feature"
                }
                .into(),
                selector: selector.into(),
            },
            verification: vec![],
        }],
        verification: vec![],
    }
}
pub fn goal_milestone(id: Option<String>, goal: String, framework: Framework) -> Milestone {
    Milestone {
        schema_version: SCHEMA,
        id: id.unwrap_or_else(|| format!("M-{}", &paths::id("m")[2..10])),
        goal,
        framework,
        revision: 1,
        phases: vec![],
        verification: vec![],
    }
}
pub fn start(
    root: &Path,
    start: Start,
    mut config: Config,
    cancel: Arc<AtomicBool>,
    observer: Observer<'_>,
) -> Result<Run> {
    if !["native", "autonomous", "plan"].contains(&start.mode.as_str()) {
        bail!("invalid_mode: choose native or autonomous");
    }
    paths::valid_id(&start.milestone.id)?;
    if start.create_roadmap && start.range.bounded() {
        bail!("invalid_range: create the roadmap before selecting phase bounds");
    }
    let detected = provider::framework(root, Some(start.milestone.framework))?;
    if detected == Framework::Openspec {
        config.provider.openspec_command = provider::openspec_argv(root, &config);
    }
    config.validate()?;
    config.schema_version = 2;
    if start.mode == "autonomous" && detected == Framework::Speckit {
        provider::check_hooks(root, &config)?;
    }

    let repo = Repository::discover(root)?;
    repo.preflight()?;
    let _lease = Lease::acquire(&repo)?;
    let store = Store::open(&repo, true)?.unwrap();
    let origin_head = repo.head()?;
    let origin_branch = repo.branch()?;
    let id = paths::id("run");
    let relative = paths::localize(&repo.root, root)?;
    if let Some(existing) = store.list()?.into_iter().find(|r| {
        r.host.is_some()
            && (!r.terminal() && !["plan_ready", "handed_off"].contains(&r.status.as_str())
                || r.has_outstanding_host_work())
            && r.project_relative == relative
    }) {
        if repo.head()? != existing.origin_head {
            bail!(
                "source_drift: run `spec-autonomous prepare --run-id {}` without --change/--feature to reconcile committed changes",
                existing.id
            );
        }
        if existing.milestone.id == start.milestone.id
            && existing.milestone.goal == start.milestone.goal
            && serde_json::to_string(&existing.range)? == serde_json::to_string(&start.range)?
            && existing.mode == start.mode
        {
            return Ok(existing);
        }
        bail!(
            "project_in_use: active run {0} must be continued (`spec-autonomous prepare --run-id {0}` without --change/--feature) or cancelled before starting another workflow for this project",
            existing.id
        );
    }

    let mut completed = vec![];
    let mut phase_hashes = BTreeMap::new();
    for old in store
        .list()?
        .iter()
        .filter(|r| r.milestone.id == start.milestone.id)
    {
        if old.accepted_head != origin_head
            || serde_json::to_string(&(
                &old.milestone,
                &old.config.verification,
                &old.config.environment,
                &old.config.hooks,
            ))? != serde_json::to_string(&(
                &start.milestone,
                &config.verification,
                &config.environment,
                &config.hooks,
            ))?
        {
            continue;
        }
        for phase in &start.milestone.phases {
            if !old.completed_phases.contains(&phase.id) {
                continue;
            }
            if let Some(hash) = old.phase_hashes.get(&phase.id) {
                if provider::inspect(root, detected, &phase.source.selector, &config)
                    .is_ok_and(|s| s.source_hash == *hash)
                {
                    completed.push(phase.id.clone());
                    phase_hashes.insert(phase.id.clone(), hash.clone());
                }
            }
        }
    }
    completed.sort();
    completed.dedup();
    let selected = if start.create_roadmap {
        vec![]
    } else {
        plan::select(&start.milestone, &start.range, &completed)?
    };
    if let Some(supplied) = &start.plan {
        let phase = start
            .milestone
            .phases
            .iter()
            .find(|p| p.id == supplied.phase_id)
            .context("invalid_plan: plan phase is absent from its milestone")?;
        let snapshot = provider::inspect(root, detected, &phase.source.selector, &config)?;
        if !snapshot.planning_ready || snapshot.next_action.kind != "implement" {
            bail!("invalid_plan: native planning prerequisites are not ready");
        }
        plan::validate_plan(supplied, &snapshot)?;
    }
    let (integration, branch) = repo.add_worktree(&id, &origin_head)?;
    if config.execution.init_submodules {
        let logs = tempfile::tempdir()?;
        let mut env = config.environment.clone();
        env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
        prepare_step(
            &config,
            cancel.clone(),
            "submodules",
            &["git", "submodule", "update", "--init", "--recursive"].map(String::from),
            &integration,
            &env,
            logs.path(),
        )
        .context("submodules_failed: the integration checkout could not initialize submodules")?;
    }
    let project = integration.join(&relative);
    provider::copy_ignored_context(root, &project)?;
    let accepted = git::commit(&integration, "sa: preserve native planning context")?;
    // The file-derived configuration, before prepare's run-local overrides.
    let loaded_config_hash = Config::load(root).ok().and_then(|c| config_hash(&c).ok());
    let now = paths::now();
    let run = Run {
        verification_reuses: 0,
        schema_version: SCHEMA,
        id,
        milestone: start.milestone,
        mode: start.mode,
        range: start.range,
        selected_phases: selected,
        origin: repo.root.to_string_lossy().into(),
        origin_head,
        origin_branch,
        project_relative: relative,
        integration: integration.to_string_lossy().into(),
        integration_branch: branch,
        accepted_head: accepted,
        status: "preparing".into(),
        stage: "preparing".into(),
        current_phase: None,
        started_at: now.clone(),
        updated_at: now,
        elapsed_ms: 0,
        active_at: None,
        blocker: None,
        config,
        completed_phases: completed,
        stale_phases: vec![],
        phase_hashes,
        plans: BTreeMap::new(),
        completed_tasks: vec![],
        attempts: vec![],
        intents: vec![],
        origin_sources: BTreeMap::new(),
        evidence: vec![],
        repair_rounds: 0,
        hook_results: BTreeMap::new(),
        host: Some(HostState {
            protocol_version: 1,
            config_hash: loaded_config_hash,
            ..Default::default()
        }),
    };
    let mut c = Coordinator {
        repo,
        store,
        run,
        cancel,
        observer,
        started: Instant::now(),
        elapsed_before: 0,
    };
    if let Some(p) = start.plan {
        c.run.plans.insert(p.phase_id.clone(), p);
    }
    c.capture_sources()?;
    c.save("created")?;
    let _watchdog = Watchdog::start(&c.repo, &c.run, c.elapsed_before, c.cancel.clone());
    let result = c.drive(start.create_roadmap);
    c.finish(result)
}
pub fn resume(
    root: &Path,
    id: &str,
    mode: Option<String>,
    cancel: Arc<AtomicBool>,
    observer: Observer<'_>,
) -> Result<Run> {
    resume_with(
        root,
        id,
        ResumeOptions {
            mode,
            ..Default::default()
        },
        cancel,
        observer,
    )
}
pub fn resume_with(
    root: &Path,
    id: &str,
    options: ResumeOptions,
    cancel: Arc<AtomicBool>,
    observer: Observer<'_>,
) -> Result<Run> {
    let repo = Repository::discover(root)?;
    let _lease = Lease::acquire(&repo)?;
    let store = Store::open(&repo, true)?.unwrap();
    let mut run = store.get(id)?;
    if run.terminal() {
        return Ok(run);
    }
    if run.host.is_none() {
        bail!(
            "legacy_run_read_only: inspect or import the native source into a new host-driven run"
        );
    }
    let mode = options.mode;
    if options.reload_config {
        // Reloading never shortens the granted deadline: earlier extend_seconds live only
        // in the run's timeout, so keep the larger of the file value and the run value.
        let granted = run.config.execution.run_timeout_seconds;
        run.config = Config::load(&Path::new(&run.origin).join(&run.project_relative))?;
        run.host.as_mut().unwrap().config_hash = Some(config_hash(&run.config)?);
        run.config.execution.run_timeout_seconds =
            run.config.execution.run_timeout_seconds.max(granted);
        if run.milestone.framework == Framework::Openspec {
            run.config.provider.openspec_command = provider::openspec_argv(
                &Path::new(&run.origin).join(&run.project_relative),
                &run.config,
            );
        }
    }
    run.config.execution.run_timeout_seconds = run
        .config
        .execution
        .run_timeout_seconds
        .checked_add(options.extend_seconds)
        .context("invalid_budget: timeout overflow")?;
    if let Some(max) = options.max_attempts {
        run.config.execution.max_attempts = max;
    }
    run.config.validate()?;
    if let Some(mode) = &mode {
        if !["native", "autonomous"].contains(&mode.as_str()) {
            bail!("invalid_mode");
        }
    }
    if mode.as_deref().unwrap_or(&run.mode) == "autonomous" {
        work_packet::doctor(&run.config)?;
    }
    store.control(id, "")?;
    let elapsed_before = elapsed(&run);
    let mut c = Coordinator {
        repo,
        store,
        run,
        cancel,
        observer,
        started: Instant::now(),
        elapsed_before,
    };
    let _watchdog = Watchdog::start(&c.repo, &c.run, c.elapsed_before, c.cancel.clone());
    let result = (|| {
        c.recover()?;
        if c.run.terminal() {
            return Ok(());
        }
        if let Some(mode) = mode {
            c.run.mode = mode;
        }
        c.run.blocker = None;
        c.drive(c.run.milestone.phases.is_empty())
    })();
    c.finish(result)
}

pub fn resolve_hook(
    root: &Path,
    id: &str,
    key: &str,
    outcome: &str,
    evidence: &str,
) -> Result<Run> {
    if !["completed", "not-run"].contains(&outcome) || evidence.trim().is_empty() {
        bail!("invalid_hook_resolution: explicit outcome and evidence are required");
    }
    let repo = Repository::discover(root)?;
    let _lease = Lease::acquire(&repo)?;
    let mut store = Store::open(&repo, true)?.unwrap();
    let mut run = store.get(id)?;
    if run.hook_results.get(key).map(String::as_str) != Some("intent") {
        bail!("invalid_hook_resolution: hook is not awaiting an outcome");
    }
    for attempt in run
        .attempts
        .iter()
        .filter(|a| a.kind == "hook" && a.task_id == key)
    {
        let dir = store.attempt_dir(id, &attempt.id)?;
        process::reconcile_process(&dir.join("process.json"))?;
    }
    if !git::clean(Path::new(&run.integration))? {
        return Err(git::dirty_error(
            Path::new(&run.integration),
            "inspect and commit hook effects in the integration checkout first",
        ));
    }
    run.accepted_head = git::head(Path::new(&run.integration))?;
    if outcome == "completed" {
        run.hook_results.insert(key.into(), "done".into());
    } else {
        run.hook_results.remove(key);
    }
    for attempt in &mut run.attempts {
        if attempt.kind == "hook" && attempt.task_id == key {
            attempt.status = "operator_resolved".into();
            attempt.summary = format!("{outcome}: {evidence}");
        }
    }
    run.status = "paused".into();
    run.stage = "hook_resolved".into();
    run.blocker = None;
    run.updated_at = paths::now();
    store.save(&run, "hook_operator_resolution")?;
    Ok(run)
}

impl Coordinator<'_> {
    fn project(&self) -> PathBuf {
        Path::new(&self.run.integration).join(&self.run.project_relative)
    }
    fn origin_project(&self) -> PathBuf {
        Path::new(&self.run.origin).join(&self.run.project_relative)
    }
    fn supersede_replaced_submissions(&mut self) -> Result<()> {
        let mut latest_success = BTreeMap::<(String, String, String), usize>::new();
        for (index, attempt) in self.run.attempts.iter().enumerate() {
            if matches!(attempt.status.as_str(), "accepted" | "integrated") {
                latest_success.insert(
                    (
                        attempt.kind.clone(),
                        attempt.task_id.clone(),
                        attempt.phase_id.clone(),
                    ),
                    index,
                );
            }
        }
        let now = paths::now();
        let mut changed = false;
        for (index, attempt) in self.run.attempts.iter_mut().enumerate() {
            let Some(&success) = latest_success.get(&(
                attempt.kind.clone(),
                attempt.task_id.clone(),
                attempt.phase_id.clone(),
            )) else {
                continue;
            };
            if index >= success || attempt.status != "submitted" {
                continue;
            }
            attempt.status = "superseded".into();
            attempt.error =
                Some("superseded: a later accepted attempt replaced this receipt".into());
            attempt.finished_at = Some(now.clone());
            if let Some(lease) = self
                .run
                .host
                .as_mut()
                .and_then(|host| host.requests.get_mut(&attempt.id))
            {
                lease.revoked = true;
            }
            changed = true;
        }
        if changed {
            self.save("receipt_superseded")?;
        }
        Ok(())
    }
    fn save(&mut self, event: &str) -> Result<()> {
        self.run.updated_at = paths::now();
        self.run.elapsed_ms = self.elapsed_before + self.started.elapsed().as_millis() as u64;
        self.run.active_at = Some(self.run.updated_at.clone());
        self.store.save(&self.run, event)?;
        (self.observer)(&self.run, event);
        Ok(())
    }
    fn check(&mut self) -> Result<()> {
        if let Some(host) = &self.run.host {
            if !host.blockers.is_empty() {
                bail!(
                    "needs_input: {}",
                    host.blockers
                        .values()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("; ")
                );
            }
        }
        let control = self.store.requested(&self.run.id)?;
        if !control.is_empty() {
            self.cancel.store(true, Ordering::Relaxed);
            bail!("{control}: requested by user");
        }
        if let Some(message) = self.budget_exhausted() {
            self.cancel.store(true, Ordering::Relaxed);
            bail!(message);
        }
        if self.cancel.load(Ordering::Relaxed) || process::interrupted() {
            bail!("paused: interrupted");
        }
        for (path, expected) in self.run.origin_sources.clone() {
            let full = paths::inside(&self.origin_project(), &path)?;
            let actual = if full.is_file() {
                paths::hash(fs::read(full)?)
            } else {
                String::new()
            };
            if actual != expected {
                // Work the host holds or that is being integrated stays valid against the
                // integration checkout; merge the new source once it has landed.
                let in_flight = self.run.attempts.iter().any(|a| {
                    matches!(
                        a.status.as_str(),
                        "issued" | "claimed" | "receiving" | "submitted" | "candidate"
                    )
                });
                if in_flight {
                    let host = self.run.host.as_mut().context("legacy_run_read_only")?;
                    if !host.reconcile_pending {
                        host.reconcile_pending = true;
                        self.save("source_drift_deferred")?;
                    }
                    return Ok(());
                }
                bail!("source_drift: original source changed: {path}");
            }
        }
        Ok(())
    }
    fn capture_sources(&mut self) -> Result<()> {
        for phase in self.run.milestone.phases.clone() {
            if let Ok(s) = provider::inspect(
                &self.origin_project(),
                self.run.milestone.framework,
                &phase.source.selector,
                &self.run.config,
            ) {
                for file in s.context_files {
                    self.run.origin_sources.insert(file.path, file.hash);
                }
            }
        }
        Ok(())
    }
    /// Debug-build test seam for interruptions between two verification checks. A file
    /// `<git common dir>/spec-autonomous/failpoints/<name>` fires once and is consumed,
    /// so tests in separate repositories stay isolated. Compiled out of release builds.
    fn failpoint_after_check(&mut self, stage: &str) -> Result<()> {
        #[cfg(debug_assertions)]
        {
            let armed = |name: &str| {
                self.repo.runtime().is_ok_and(|dir| {
                    std::fs::remove_file(dir.join("failpoints").join(format!("{name}-{stage}")))
                        .is_ok()
                })
            };
            if armed("pause_after_check") {
                self.store.control(&self.run.id, "pause")?;
            }
            if armed("budget_after_check") {
                self.elapsed_before = self
                    .run
                    .config
                    .execution
                    .run_timeout_seconds
                    .saturating_mul(1000);
            }
        }
        #[cfg(not(debug_assertions))]
        let _ = stage;
        Ok(())
    }
    /// The budget_exhausted blocker once the active-time budget is used, with the time
    /// used, the current limit and the smallest extend_seconds that reopens the budget.
    /// When that extension cannot fit under the 72 h cap it says unsafe_budgets is needed.
    fn budget_exhausted(&self) -> Option<String> {
        let elapsed_ms = self.elapsed_before + self.started.elapsed().as_millis() as u64;
        let limit = self.run.config.execution.run_timeout_seconds;
        if elapsed_ms < limit.saturating_mul(1000) {
            return None;
        }
        let elapsed = elapsed_ms / 1000;
        let minimum = elapsed.saturating_sub(limit) + 1;
        let cap = crate::config::RUN_TIMEOUT_CAP_SECONDS;
        let headroom = cap.saturating_sub(limit);
        let capped = if self.run.config.execution.unsafe_budgets {
            String::new()
        } else if minimum > headroom {
            format!(
                "; run_timeout_seconds cannot exceed {cap} (72h) without [execution] unsafe_budgets = true, so this run needs unsafe_budgets = true in the project config and reload_config before any extension"
            )
        } else {
            format!(
                "; without [execution] unsafe_budgets = true the total run_timeout_seconds stays at most {cap} (72h), so extend_seconds can add at most {headroom}"
            )
        };
        Some(format!(
            "budget_exhausted: run deadline reached: {elapsed}s elapsed (active time: coordinator work and host-held packets; time the run spends stopped for input or paused is not counted) of run_timeout_seconds={limit}; continue with `spec-autonomous prepare --run-id {} --extend-seconds <n>` where n (extend_seconds) is at least {minimum} plus the time the remaining work needs{capped}",
            self.run.id,
        ))
    }
    fn finish(mut self, result: Result<()>) -> Result<Run> {
        if let Err(error) = result {
            let mut message = format!("{error:#}");
            let control = self.store.requested(&self.run.id).unwrap_or_default();
            if control == "cancel" {
                message = "cancel: requested by user".into();
            } else if control == "pause" {
                message = "paused: requested by user".into();
            } else if let Some(exhausted) = self.budget_exhausted() {
                message = exhausted;
            }
            self.run.status = if message.starts_with("awaiting_host:") {
                "awaiting_host"
            } else if message.starts_with("cancel:") {
                "cancelled"
            } else if message.contains("delivery_pending") {
                "delivery_pending"
            } else if message.contains("needs_input") || message.contains("hook_outcome_unknown") {
                "needs_input"
            } else {
                "paused"
            }
            .into();
            if !message.starts_with("awaiting_host:") {
                // A planning candidate kept for an environment fix stays a candidate.
                if let Some(attempt) = self.run.attempts.iter_mut().rev().find(|a| {
                    a.status == "candidate"
                        && !a
                            .error
                            .as_deref()
                            .is_some_and(|e| e.starts_with(planning::ENVIRONMENT_UNAVAILABLE))
                }) {
                    attempt.status = "failed".into();
                    attempt.error = Some(message.clone());
                }
            }
            self.run.blocker = Some(message);
            self.save("stopped")?;
        }
        // Closed worktrees go now that their evidence is durable; keeping them for a
        // manual cleanup is what grew runtimes to tens of gigabytes.
        match crate::reclaim::reclaim(&self.repo, &self.store, &mut self.run) {
            Ok(report) => {
                if !report["removed"].as_array().is_none_or(Vec::is_empty) {
                    self.save("worktrees_reclaimed")?;
                }
            }
            Err(error) => {
                self.run.blocker.get_or_insert_with(String::new);
                let _ = error;
            }
        }
        if self.run.config.execution.automatic_cleanup {
            match self
                .store
                .list()
                .and_then(|runs| crate::cleanup::at_handoff(&self.repo, &self.run, &runs))
            {
                Ok(result) => {
                    let _ = paths::atomic_write(
                        &self
                            .store
                            .root
                            .join("runs")
                            .join(&self.run.id)
                            .join("maintenance.json"),
                        serde_json::to_vec_pretty(&result)?,
                    );
                }
                Err(error) => {
                    // Maintenance never turns verified work into a failed task.
                    let _ = paths::atomic_write(
                        &self
                            .store
                            .root
                            .join("runs")
                            .join(&self.run.id)
                            .join("maintenance.json"),
                        serde_json::to_vec_pretty(
                            &serde_json::json!({"error":error.to_string(),"evidence_retained":true}),
                        )?,
                    );
                }
            }
        }
        let body = format!(
            "# {}\n\nRun: {}\nStatus: {}\nStage: {}\nCheckout: {}\nAccepted revision: {}\nSelected phases: {}\nCompleted phases: {}\nVerified tasks: {}\nAttempts: {}\nElapsed ms: {}\nUsage: unavailable\n\n{}\n\n## Evidence\n\n{}\n",
            self.run.milestone.goal,
            self.run.id,
            self.run.status,
            self.run.stage,
            self.run.integration,
            self.run.accepted_head,
            self.run.selected_phases.join(", "),
            self.run.completed_phases.join(", "),
            self.run.completed_tasks.len(),
            self.run.attempts.len(),
            self.run.elapsed_ms,
            self.run
                .blocker
                .clone()
                .unwrap_or_else(|| "All checks required for the selected scope passed.".into()),
            self.run
                .evidence
                .iter()
                .map(|e| format!(
                    "- {:?}: exit {}, revision {}, log {} ({})",
                    e.argv, e.exit_code, e.revision, e.log, e.log_hash
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
        paths::atomic_write(
            &crate::state::report_path(&self.store.root, &self.run.id),
            body,
        )?;
        Ok(self.run)
    }
    fn make_attempt(
        &mut self,
        kind: &str,
        task: &str,
        phase: &str,
    ) -> Result<(usize, PathBuf, PathBuf)> {
        self.check()?;
        let id = paths::id("attempt");
        let name = format!(
            "{}-{}",
            &self.run.id[..self.run.id.len().min(12)],
            &id[..20]
        );
        // Read-only planning and audit packets reuse a closed clean checkout when one
        // exists; writing workers always get their own.
        let (worktree, branch) = if matches!(kind, "plan-tasks" | "audit") {
            match self.review_checkout()? {
                Some(checkout) => checkout,
                None => self.repo.add_worktree(&name, &self.run.accepted_head)?,
            }
        } else {
            self.repo.add_worktree(&name, &self.run.accepted_head)?
        };
        provider::copy_ignored_context(
            &self.project(),
            &worktree.join(&self.run.project_relative),
        )?;
        let dir = self.store.attempt_dir(&self.run.id, &id)?;
        let writing = !matches!(kind, "plan-tasks" | "audit");
        let index = self.run.attempts.len();
        self.run.attempts.push(Attempt {
            id,
            task_id: task.into(),
            phase_id: phase.into(),
            kind: kind.into(),
            worktree: worktree.to_string_lossy().into(),
            branch,
            base_commit: self.run.accepted_head.clone(),
            status: "running".into(),
            started_at: paths::now(),
            finished_at: None,
            summary: String::new(),
            error: None,
            pid: None,
            process_identity: None,
            evidence: vec![],
            usage: None,
            patch_hash: None,
            worktree_state: None,
        });
        self.save("attempt_started")?;
        if writing && let Err(error) = self.prepare_worker(index, &worktree, &dir) {
            // Nothing was issued: the attempt closes without counting as a task failure.
            let message = format!(
                "needs_input: worker_prepare_failed: {error:#}. Fix the environment, [execution] init_submodules or [hooks.worker_prepare] (pass reload_config after a configuration change), then run `spec-autonomous prepare --run-id {}`",
                self.run.id
            );
            let attempt = &mut self.run.attempts[index];
            attempt.status = "superseded".into();
            attempt.finished_at = Some(paths::now());
            attempt.error = Some(message.clone());
            self.save("worker_prepare_failed")?;
            bail!(message);
        }
        Ok((index, worktree, dir))
    }
    /// Initialize submodules in a checkout the coordinator created, when configured.
    /// Logs go to `<dir>/submodules/`.
    pub(crate) fn init_submodules(&self, checkout: &Path, dir: &Path) -> Result<()> {
        if !self.run.config.execution.init_submodules {
            return Ok(());
        }
        let argv = ["git", "submodule", "update", "--init", "--recursive"].map(String::from);
        let mut env = self.run.config.environment.clone();
        env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
        prepare_step(
            &self.run.config,
            self.cancel.clone(),
            "submodules",
            &argv,
            checkout,
            &env,
            dir,
        )
    }
    /// Bring a fresh writing checkout to the state its packet assumes: submodules
    /// (`[execution] init_submodules`) and the `[hooks.worker_prepare]` hook, both run at
    /// the worktree root before anything is issued.
    fn prepare_worker(&mut self, index: usize, worktree: &Path, dir: &Path) -> Result<()> {
        self.init_submodules(worktree, dir)?;
        let Some(hook) = self
            .run
            .config
            .hooks
            .get(crate::config::WORKER_PREPARE)
            .cloned()
        else {
            return Ok(());
        };
        let attempt = &self.run.attempts[index];
        let mut env = self.run.config.environment.clone();
        for (key, value) in [
            (
                "SA_WORKER_CHECKOUT",
                worktree.to_string_lossy().into_owned(),
            ),
            (
                "SA_WORKER_PROJECT",
                worktree
                    .join(&self.run.project_relative)
                    .to_string_lossy()
                    .into_owned(),
            ),
            ("SA_WORKER_ATTEMPT_ID", attempt.id.clone()),
            ("SA_WORKER_KIND", attempt.kind.clone()),
            ("SA_WORKER_TASK_ID", attempt.task_id.clone()),
            ("SA_WORKER_PHASE_ID", attempt.phase_id.clone()),
            ("SA_WORKER_BASE_COMMIT", attempt.base_commit.clone()),
            ("SA_ORIGIN_CHECKOUT", self.run.origin.clone()),
            ("SA_RUN_ID", self.run.id.clone()),
        ] {
            env.insert(key.into(), value);
        }
        prepare_step(
            &self.run.config,
            self.cancel.clone(),
            "worker-prepare",
            &hook.argv,
            worktree,
            &env,
            dir,
        )
    }
    fn input(
        &self,
        index: usize,
        instruction: String,
        snapshot: Option<Snapshot>,
        task: Option<Task>,
        failure: Option<String>,
    ) -> WorkerInput {
        let a = &self.run.attempts[index];
        WorkerInput {
            schema_version: SCHEMA,
            run_id: self.run.id.clone(),
            task_id: a.task_id.clone(),
            attempt_id: a.id.clone(),
            kind: a.kind.clone(),
            goal: self.run.milestone.goal.clone(),
            framework: self.run.milestone.framework,
            base_commit: a.base_commit.clone(),
            instruction,
            task,
            snapshot,
            milestone: Some(self.run.milestone.clone()),
            failure,
            previous_candidate: None,
        }
    }
    /// The uncommitted changes of the latest failed attempt of the same task, copied to
    /// `previous-candidate.diff` in the new attempt directory. Reads the checkout while
    /// it exists and the patch kept by reclamation after it was removed.
    fn previous_candidate(&self, index: usize, dir: &Path) -> Result<Option<PreviousCandidate>> {
        let current = &self.run.attempts[index];
        let Some(previous) = self.run.attempts[..index].iter().rev().find(|a| {
            a.kind == current.kind
                && a.task_id == current.task_id
                && a.phase_id == current.phase_id
                && a.status == "failed"
        }) else {
            return Ok(None);
        };
        let worktree = Path::new(&previous.worktree);
        let patch = if worktree.is_dir() {
            git::patch(worktree, &previous.base_commit).ok()
        } else {
            self.store
                .attempt_dir(&self.run.id, &previous.id)
                .ok()
                .and_then(|d| fs::read_to_string(d.join("patch.diff")).ok())
        };
        let Some(patch) = patch.filter(|p| !p.trim().is_empty()) else {
            return Ok(None);
        };
        let path = dir.join("previous-candidate.diff");
        paths::atomic_write(&path, patch.as_bytes())?;
        Ok(Some(PreviousCandidate {
            attempt_id: previous.id.clone(),
            base_commit: previous.base_commit.clone(),
            patch_path: path.to_string_lossy().into_owned(),
            files: git::patch_files(&patch),
            same_base: previous.base_commit == current.base_commit,
        }))
    }
    /// Files an attempt changed against its base. Reads the checkout while it exists and
    /// the stored patch after the worktree was reclaimed.
    fn attempt_changes(&self, index: usize) -> Result<Vec<String>> {
        let attempt = &self.run.attempts[index];
        let worktree = Path::new(&attempt.worktree);
        if worktree.is_dir() {
            return git::changed(worktree, &attempt.base_commit);
        }
        let patch = self
            .store
            .attempt_dir(&self.run.id, &attempt.id)?
            .join("patch.diff");
        let text = fs::read_to_string(&patch).with_context(|| {
            format!(
                "patch_unavailable: worktree {} was reclaimed and {} is missing",
                attempt.worktree,
                patch.display()
            )
        })?;
        Ok(git::patch_files(&text))
    }
    fn end_attempt(&mut self, index: usize, result: &Result<WorkerResult>) -> Result<()> {
        let a = &mut self.run.attempts[index];
        a.finished_at = Some(paths::now());
        match result {
            Ok(r) => {
                a.status = "candidate".into();
                a.summary = r.summary.clone();
            }
            Err(e) => {
                a.status = "failed".into();
                a.error = Some(format!("{e:#}"));
            }
        }
        self.save("attempt_finished")
    }
    fn invoke(
        &mut self,
        kind: &str,
        task: &str,
        phase: &str,
        instruction: String,
        snapshot: Option<Snapshot>,
    ) -> Result<(WorkerResult, PathBuf, usize)> {
        self.invoke_host(kind, task, phase, instruction, snapshot)
    }
}
/// Run one checkout preparation step with bounded time and logs in `<dir>/<name>/`.
pub(crate) fn prepare_step(
    config: &crate::config::Config,
    cancel: Arc<AtomicBool>,
    name: &str,
    argv: &[String],
    cwd: &Path,
    env: &BTreeMap<String, String>,
    dir: &Path,
) -> Result<()> {
    let logs = dir.join(name);
    let output = process::execute(process::Request {
        argv,
        cwd,
        env,
        stdin: None,
        directory: &logs,
        timeout: Duration::from_secs(config.execution.attempt_timeout_seconds),
        max_log_bytes: config.execution.max_log_bytes,
        cancel,
    })
    .with_context(|| format!("{name} could not start {argv:?}"))?;
    if output.timed_out || output.cancelled || output.code != 0 {
        let tail: String = fs::read_to_string(logs.join("stderr.log"))
            .unwrap_or_default()
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        bail!(
            "{name} {argv:?} {} in {}; logs in {}: {tail}",
            if output.timed_out {
                "timed out".to_string()
            } else if output.cancelled {
                "was cancelled".to_string()
            } else {
                format!("exited with {}", output.code)
            },
            cwd.display(),
            logs.display()
        );
    }
    Ok(())
}
fn failpoint(name: &str) {
    #[cfg(debug_assertions)]
    if std::env::var("SPEC_AUTONOMOUS_TEST_FAILPOINT").as_deref() == Ok(name) {
        std::process::exit(86);
    }
    #[cfg(not(debug_assertions))]
    let _ = name;
}

struct Watchdog {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Watchdog {
    /// `elapsed` is the active time already used, the same base budget_exhausted uses.
    fn start(repo: &Repository, run: &Run, elapsed: u64, cancel: Arc<AtomicBool>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let repo = repo.clone();
        let id = run.id.clone();
        let remaining = Duration::from_millis(
            (run.config.execution.run_timeout_seconds * 1000).saturating_sub(elapsed),
        );
        let handle = thread::spawn(move || {
            let start = Instant::now();
            let store = Store::open(&repo, false);
            let Ok(Some(store)) = store else {
                cancel.store(true, Ordering::Relaxed);
                return;
            };
            while !end.load(Ordering::Relaxed) {
                if start.elapsed() >= remaining || store.requested(&id).is_ok_and(|s| !s.is_empty())
                {
                    cancel.store(true, Ordering::Relaxed);
                    break;
                }
                thread::sleep(Duration::from_millis(25));
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn repeated(keys: Option<&Vec<String>>, limit: u32) -> bool {
    keys.is_some_and(|v| {
        v.len() >= limit as usize
            && v.iter()
                .rev()
                .take(limit as usize)
                .all(|k| Some(k) == v.last())
    })
}

/// Identity of a loaded project configuration (user and project files merged).
pub(crate) fn config_hash(config: &Config) -> Result<String> {
    Ok(paths::hash(serde_json::to_vec(config)?))
}
/// A short notice when the project configuration differs from the one the run loaded at
/// start or its last reload_config. The run keeps its frozen configuration.
pub(crate) fn config_drift(run: &Run) -> Option<String> {
    let recorded = run.host.as_ref()?.config_hash.as_ref()?;
    if run.terminal() {
        return None;
    }
    match Config::load(&Path::new(&run.origin).join(&run.project_relative)) {
        Ok(current) if config_hash(&current).ok().as_ref() == Some(recorded) => None,
        Ok(_) => Some(
            "config_drift: project configuration changed since the run started; pass reload_config to apply it"
                .into(),
        ),
        Err(error) => Some(format!(
            "config_drift: project configuration changed since the run started and does not load ({error:#}); fix it, then pass reload_config"
        )),
    }
}
/// Active time used so far: the recorded coordinator time plus, while the host holds
/// issued work (`awaiting_host`), the time since the coordinator last recorded it. A run
/// stopped in `needs_input`, `paused` or a terminal state accrues nothing, so a day away
/// from a blocked run no longer spends its budget. Runs saved before 0.2.0 have no
/// `active_at` and resume from their recorded `elapsed_ms`.
pub(crate) fn elapsed(run: &Run) -> u64 {
    let held = (run.status == "awaiting_host")
        .then_some(run.active_at.as_deref())
        .flatten()
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .map(|at| (chrono::Utc::now().timestamp_millis() - at.timestamp_millis()).max(0) as u64)
        .unwrap_or(0);
    run.elapsed_ms.saturating_add(held)
}
