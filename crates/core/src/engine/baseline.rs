//! Opt-in baseline of a newly accepted plan (`[execution] baseline_checks`): checks that
//! already fail before any task ran are reported up front instead of surfacing at the
//! last task that happens to run them.
use super::*;

impl Coordinator<'_> {
    /// Once per phase plan, before any implement work of the phase is issued: run the
    /// plan's checks whose files and executables already exist (preflight `ready`; checks
    /// waiting for files a task writes stay `deferred` and are skipped) on the integration
    /// checkout at `accepted_head`, and record the failing ones in `host.baselines`.
    /// A failure blocks, retries or repairs nothing; responses list it as a
    /// `baseline_failure:` warning. An interrupted baseline records nothing and runs again.
    pub(super) fn baseline_checks(&mut self, phase: &Phase, plan: &Plan) -> Result<()> {
        if !self.run.config.execution.baseline_checks || self.run.mode == "plan" {
            return Ok(());
        }
        let host = self.run.host.as_ref().context("legacy_run_read_only")?;
        if host.baselines.contains_key(&phase.id)
            || self
                .run
                .attempts
                .iter()
                .any(|a| a.phase_id == phase.id && a.kind == "implement")
        {
            return Ok(());
        }
        let project = self.project();
        let integration = PathBuf::from(&self.run.integration);
        let preparation = self.run.config.hooks.get("verification_prepare").cloned();
        // Verification itself refuses a non-idempotent preparation; so does the baseline.
        if preparation.as_ref().is_some_and(|h| !h.idempotent) || !git::clean(&integration)? {
            return Ok(());
        }
        let writes: Vec<String> = plan.tasks.iter().flat_map(|t| t.writes.clone()).collect();
        let mut checks: Vec<Check> = vec![];
        for check in plan.tasks.iter().flat_map(|t| self.task_checks(t)) {
            if checks.contains(&check) {
                continue;
            }
            let report = crate::verification_preflight::inspect_checks_with_config(
                &project,
                std::slice::from_ref(&check),
                &writes,
                &self.run.config,
            );
            if report.checks.first().is_some_and(|c| c.status == "ready")
                && !names_future_file(&project, &check, &writes)
            {
                checks.push(check);
            }
        }
        let head = self.run.accepted_head.clone();
        let mut failures = vec![];
        let mut prepared = None;
        if !checks.is_empty() {
            let directory = self
                .store
                .root
                .join("runs")
                .join(&self.run.id)
                .join(paths::id("baseline"));
            let wave =
                crate::verification::Wave::new(directory, &project, "baseline", &self.run.config)?;
            if let Some(hook) = preparation {
                self.check()?;
                let step = Check::new(hook.argv, ".");
                let result = wave.execute(&step, true, &self.run.config, self.cancel.clone())?;
                prepared = Some(result.failure.clone());
                if result.failure.as_deref() == Some("cancelled") {
                    self.restore_integration(&integration, &head)?;
                    bail!("paused: interrupted");
                }
            }
            if !matches!(prepared, Some(Some(_))) {
                for check in &checks {
                    self.check()?;
                    let result =
                        wave.execute(check, false, &self.run.config, self.cancel.clone())?;
                    match result.failure.as_deref() {
                        None => {}
                        Some("cancelled") => {
                            self.restore_integration(&integration, &head)?;
                            bail!("paused: interrupted");
                        }
                        Some(kind) => {
                            let details = result.evidence.details.clone().unwrap_or_default();
                            failures.push(serde_json::json!({
                                "argv": check.argv,
                                "cwd": check.cwd,
                                "exit_code": result.evidence.exit_code,
                                "failure_kind": kind,
                                "summary": crate::verification::failure_summary(&details, &check.cwd, &self.run.config),
                                "manifest": details["manifest"],
                            }));
                        }
                    }
                }
            }
            self.restore_integration(&integration, &head)?;
        }
        let record = serde_json::json!({
            "head": head,
            "at": paths::now(),
            "checks_run": if matches!(prepared, Some(Some(_))) { 0 } else { checks.len() },
            "preparation_failed": matches!(prepared, Some(Some(_))),
            "failures": failures,
        });
        self.run
            .host
            .as_mut()
            .context("legacy_run_read_only")?
            .baselines
            .insert(phase.id.clone(), record);
        self.save("baseline_recorded")
    }
    /// Checks may leave files behind; the integration checkout must stay at its head.
    fn restore_integration(&self, integration: &Path, head: &str) -> Result<()> {
        if !git::clean(integration)? {
            git::command(integration, &["reset", "--hard", "-q", head], None)?;
            git::command(integration, &["clean", "-fdq"], None)?;
        }
        Ok(())
    }
}

/// A check whose argv names a file that does not exist yet and that a task of the plan
/// writes (for example `node tests/new.mjs`) waits for that task, even where preflight
/// does not recognize the operand.
fn names_future_file(project: &Path, check: &Check, writes: &[String]) -> bool {
    check.argv.iter().skip(1).any(|arg| {
        !arg.starts_with('-')
            && !project.join(&check.cwd).join(arg).exists()
            && paths::relative(&format!("{}/{arg}", check.cwd)).is_ok_and(|relative| {
                let normal = relative
                    .components()
                    .filter_map(|c| match c {
                        std::path::Component::Normal(p) => Some(p.to_string_lossy()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("/");
                plan::allowed(&normal, writes)
            })
    })
}

/// `baseline_failure:` warnings for phases whose baseline recorded failing checks.
pub(crate) fn warnings(run: &Run) -> Vec<String> {
    let Some(host) = &run.host else {
        return vec![];
    };
    let mut out = vec![];
    for (phase, record) in &host.baselines {
        if run.completed_phases.contains(phase) {
            continue;
        }
        let head: String = record["head"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .take(12)
            .collect();
        if record["preparation_failed"] == true {
            out.push(format!(
                "baseline_failure: phase {phase}: verification_prepare failed on the accepted baseline {head}, so no baseline check ran"
            ));
        }
        for failure in record["failures"].as_array().into_iter().flatten() {
            out.push(format!(
                "baseline_failure: phase {phase}: this check already fails on the accepted baseline {head} before any task ran (expected when the plan's tasks are what make it pass; otherwise the plan did not cause it): {}",
                crate::work_packet::clip(failure["summary"].as_str().unwrap_or_default(), 600)
            ));
        }
    }
    out
}
