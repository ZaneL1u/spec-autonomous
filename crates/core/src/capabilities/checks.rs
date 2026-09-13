use super::*;
pub fn invoke(root: &Path, name: &str, args: &Value) -> Result<Value> {
    if name == "verify.source" {
        let m = resolve_milestone(root, args)?;
        let p = select_phase(&m, args)?;
        let s = provider::inspect(root, m.framework, &p.source.selector, &Config::load(root)?)?;
        return Ok(
            json!({"source_hash":s.source_hash,"planning_ready":s.planning_ready,"execution_ready":s.next_action.kind=="implement","diagnostics":s.diagnostics,"native_tasks":s.tasks.len(),"native_checked":s.tasks.iter().filter(|t|t.done).count()}),
        );
    }
    if name == "verify.plan" {
        let m = resolve_milestone(root, args)?;
        let p = select_phase(&m, args)?;
        let s = provider::inspect(root, m.framework, &p.source.selector, &Config::load(root)?)?;
        let plan: crate::model::Plan = serde_json::from_value(args["plan"].clone())?;
        crate::plan::validate_plan(&plan, &s)?;
        let config = Config::load(root)?;
        let readiness = crate::verification_preflight::inspect_plan_with_environment(
            root,
            &plan,
            &config.verification,
            &config.runner.environment,
        );
        return Ok(
            json!({"valid":true,"phase_id":p.id,"source_hash":s.source_hash,"verification_readiness":readiness}),
        );
    }
    if name == "verify.references" {
        let file = text(args, "file")?;
        let body = paths::read(root, file, 2 * 1024 * 1024)?;
        let regex = regex::Regex::new(r"\]\(([^\s)]+)\)")?;
        let parent = Path::new(file).parent().unwrap_or(Path::new(""));
        let mut references = vec![];
        for capture in regex.captures_iter(&body) {
            let target = &capture[1];
            if ["http:", "https:", "#"]
                .iter()
                .any(|prefix| target.starts_with(prefix))
            {
                continue;
            }
            let target = target.split('#').next().unwrap();
            let path = root.join(parent).join(target);
            let canonical = path.canonicalize().ok();
            let valid = canonical.as_ref().is_some_and(|p| p.starts_with(root));
            references.push(json!({"reference":target,"exists_within_project":valid}));
        }
        return Ok(
            json!({"file":file,"source_hash":paths::hash(&body),"valid":references.iter().all(|r|r["exists_within_project"]==true),"references":references}),
        );
    }
    let repo = Repository::discover(root)?;
    let store = Store::open(&repo, false)?.context("run_not_found")?;
    let run = store.get(text(args, "run_id")?)?;
    if name == "history.summaries" {
        return Ok(
            json!({"run_id":run.id,"summaries":run.attempts.iter().filter(|a|a.finished_at.is_some()).map(|a|json!({"request_id":a.id,"kind":a.kind,"task_id":a.task_id,"phase_id":a.phase_id,"status":a.status,"summary":a.summary,"error":a.error,"finished_at":a.finished_at})).collect::<Vec<_>>()}),
        );
    }
    let mut findings = vec![];
    for p in &run.milestone.phases {
        if !run.completed_phases.contains(&p.id) {
            findings.push(json!({"kind":"phase_unverified","phase_id":p.id}));
        }
        match provider::inspect(
            &Path::new(&run.integration).join(&run.project_relative),
            run.milestone.framework,
            &p.source.selector,
            &run.config,
        ) {
            Ok(s) => {
                for task in s.tasks.iter().filter(|t| !t.done) {
                    findings.push(json!({"kind":"native_task_open","phase_id":p.id,"source_path":task.source_path,"task_id":task.id}));
                }
                for message in s.diagnostics {
                    findings.push(
                        json!({"kind":"source_diagnostic","phase_id":p.id,"message":message}),
                    );
                }
            }
            Err(e) => findings
                .push(json!({"kind":"source_unavailable","phase_id":p.id,"message":e.to_string()})),
        }
    }
    if let Some(host) = &run.host {
        for (id, message) in &host.blockers {
            findings.push(json!({"kind":"blocker","id":id,"message":message}));
        }
    }
    for a in run.attempts.iter().filter(|a| {
        matches!(
            a.status.as_str(),
            "issued" | "claimed" | "receiving" | "submitted"
        )
    }) {
        findings.push(json!({"kind":"host_work_outstanding","request_id":a.id,"status":a.status}));
    }
    let failed = run.evidence.iter().filter(|e| e.exit_code != 0).count();
    Ok(
        json!({"run_id":run.id,"findings":findings,"completed":run.terminal()&&run.status!="cancelled","historical_failed_checks":failed,"verification_evidence":run.evidence.len()}),
    )
}
