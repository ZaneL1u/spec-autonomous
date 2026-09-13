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

mod execution;
mod host;
mod lifecycle;
mod planning;
mod recovery;
pub use host::{
    ClaimRequest, apply_result, claim, claim_batch, heartbeat, host_control, next, revoke,
    work_context, work_view,
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
                || r.attempts.iter().any(|a| {
                    matches!(
                        a.status.as_str(),
                        "issued" | "claimed" | "submitted" | "receiving"
                    )
                }))
            && r.project_relative == relative
    }) {
        if repo.head()? != existing.origin_head {
            bail!("source_drift: resume the existing run to reconcile committed changes");
        }
        if existing.milestone.id == start.milestone.id
            && existing.milestone.goal == start.milestone.goal
            && serde_json::to_string(&existing.range)? == serde_json::to_string(&start.range)?
            && existing.mode == start.mode
        {
            return Ok(existing);
        }
        bail!(
            "project_in_use: active run {} must be continued or cancelled before starting another workflow for this project",
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
    let project = integration.join(&relative);
    provider::copy_ignored_context(root, &project)?;
    let accepted = git::commit(&integration, "sa: preserve native planning context")?;
    let now = paths::now();
    let run = Run {
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
        blocker: None,
        config,
        completed_phases: completed,
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
    let _watchdog = Watchdog::start(&c.repo, &c.run, c.cancel.clone());
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
        run.config = Config::load(&Path::new(&run.origin).join(&run.project_relative))?;
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
    let _watchdog = Watchdog::start(&c.repo, &c.run, c.cancel.clone());
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
        bail!("dirty_checkout: inspect and commit hook effects in the integration checkout first");
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
    fn save(&mut self, event: &str) -> Result<()> {
        self.run.updated_at = paths::now();
        self.run.elapsed_ms = self.elapsed_before + self.started.elapsed().as_millis() as u64;
        self.store.save(&self.run, event)?;
        (self.observer)(&self.run, event);
        Ok(())
    }
    fn check(&self) -> Result<()> {
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
        if self.elapsed_before + self.started.elapsed().as_millis() as u64
            >= self.run.config.execution.run_timeout_seconds * 1000
        {
            self.cancel.store(true, Ordering::Relaxed);
            bail!("budget_exhausted: run deadline reached");
        }
        if self.cancel.load(Ordering::Relaxed) || process::interrupted() {
            bail!("paused: interrupted");
        }
        for (path, expected) in &self.run.origin_sources {
            let full = paths::inside(&self.origin_project(), path)?;
            let actual = if full.is_file() {
                paths::hash(fs::read(full)?)
            } else {
                String::new()
            };
            if actual != *expected {
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
    fn finish(mut self, result: Result<()>) -> Result<Run> {
        if let Err(error) = result {
            let mut message = format!("{error:#}");
            let control = self.store.requested(&self.run.id).unwrap_or_default();
            if control == "cancel" {
                message = "cancel: requested by user".into();
            } else if control == "pause" {
                message = "paused: requested by user".into();
            } else if self.elapsed_before + self.started.elapsed().as_millis() as u64
                >= self.run.config.execution.run_timeout_seconds * 1000
            {
                message = "budget_exhausted: run deadline reached".into();
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
                if let Some(attempt) = self
                    .run
                    .attempts
                    .iter_mut()
                    .rev()
                    .find(|a| a.status == "candidate")
                {
                    attempt.status = "failed".into();
                    attempt.error = Some(message.clone());
                }
            }
            self.run.blocker = Some(message);
            self.save("stopped")?;
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
        let name = format!("{}-{}", &self.run.id[..12], &id[..20]);
        let (worktree, branch) = self.repo.add_worktree(&name, &self.run.accepted_head)?;
        provider::copy_ignored_context(
            &self.project(),
            &worktree.join(&self.run.project_relative),
        )?;
        let dir = self.store.attempt_dir(&self.run.id, &id)?;
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
        });
        self.save("attempt_started")?;
        Ok((index, worktree, dir))
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
        }
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
    fn start(repo: &Repository, run: &Run, cancel: Arc<AtomicBool>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let repo = repo.clone();
        let id = run.id.clone();
        let remaining = Duration::from_millis(
            (run.config.execution.run_timeout_seconds * 1000).saturating_sub(run.elapsed_ms),
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

fn elapsed(run: &Run) -> u64 {
    chrono::DateTime::parse_from_rfc3339(&run.started_at)
        .ok()
        .map(|start| {
            (chrono::Utc::now().timestamp_millis() - start.timestamp_millis()).max(0) as u64
        })
        .unwrap_or(run.elapsed_ms)
        .max(run.elapsed_ms)
}
