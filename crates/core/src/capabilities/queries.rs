use super::*;

pub fn inspect(root: &Path, args: &Value) -> Result<Value> {
    let detection = crate::detect(root, framework_arg(args)?)?;
    if args.get("change").is_some()
        || args.get("feature").is_some()
        || args.get("milestone_id").is_some()
    {
        let config = Config::load(root)?;
        let m = resolve_milestone(root, args)?;
        let phase = select_phase(&m, args)?;
        return Ok(serde_json::to_value(provider::inspect(
            root,
            m.framework,
            &phase.source.selector,
            &config,
        )?)?);
    }
    let mut sources = vec![];
    for found in &detection.detected {
        let base = if found.framework == Framework::Openspec {
            root.join("openspec/changes")
        } else {
            root.join("specs")
        };
        if !base.is_dir() {
            continue;
        }
        for entry in fs::read_dir(base)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() || entry.file_name() == "archive" {
                continue;
            }
            let selector = if found.framework == Framework::Openspec {
                entry.file_name().to_string_lossy().into_owned()
            } else {
                paths::localize(root, &entry.path())?
            };
            sources.push(json!({"framework":found.framework,"selector":selector}));
        }
    }
    sources.sort_by_key(|v| v.to_string());
    let inventory = crate::progress::snapshot(root)
        .unwrap_or_else(|e| json!({"consistency":"unavailable","diagnostics":[e.to_string()]}));
    Ok(
        json!({"schema_version":1,"detection":detection,"sources":sources,"progress":inventory,"starts_agents":false}),
    )
}
pub fn state(root: &Path, name: &str, args: &Value) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let id = text(args, "run_id")?;
    if matches!(
        name,
        "state.get" | "history.get" | "task.list" | "task.ready"
    ) {
        let store = Store::open(&repo, false)?.context("run_not_found")?;
        let run = store.get(id)?;
        return match name {
            "history.get" => Ok(
                json!({"run_id":id,"events":store.events(id)?,"decisions":run.host.as_ref().map(|h|&h.decisions)}),
            ),
            "state.get" => Ok(
                json!({"run":crate::progress::public_run(&run),"decisions":run.host.as_ref().map(|h|&h.decisions),"blockers":run.host.as_ref().map(|h|&h.blockers)}),
            ),
            _ => {
                let phase = args["phase_id"]
                    .as_str()
                    .or(run.current_phase.as_deref())
                    .context("phase_required")?;
                let Some(plan) = run.plans.get(phase) else {
                    return Ok(json!({"tasks":[],"next_action":"prepare_native_planning"}));
                };
                let done: Vec<_> = plan
                    .tasks
                    .iter()
                    .filter(|t| {
                        run.completed_tasks
                            .contains(&crate::model::Run::key(phase, &t.id))
                    })
                    .map(|t| t.id.clone())
                    .collect();
                let active: Vec<_> = plan
                    .tasks
                    .iter()
                    .filter(|t| {
                        run.attempts.iter().any(|a| {
                            a.phase_id == phase
                                && a.task_id == t.id
                                && matches!(a.status.as_str(), "issued" | "claimed" | "submitted")
                        })
                    })
                    .collect();
                let ready = crate::plan::ready(
                    plan,
                    &done,
                    &active,
                    run.config
                        .execution
                        .max_workers
                        .min(run.config.host.max_concurrency),
                );
                Ok(
                    json!({"run_id":id,"phase_id":phase,"tasks":if name=="task.ready"{serde_json::to_value(ready)?}else{serde_json::to_value(&plan.tasks)?},"completed":done,"source_hash":plan.source_hash}),
                )
            }
        };
    }
    if name == "task.complete" {
        bail!(
            "verification_required: submit the owned host receipt through apply-result; native checkboxes cannot be completed directly"
        );
    }
    let _lock = Lease::acquire(&repo)?;
    let mut store = Store::open(&repo, true)?.unwrap();
    let mut run = store.get(id)?;
    if let Some(expected) = args["expected_updated_at"].as_str() {
        if expected != run.updated_at {
            bail!("state_drift: state changed since read");
        }
    }
    let host = run.host.as_mut().context("legacy_run_read_only")?;
    match name{
        "state.decision"=>host.decisions.push(json!({"id":paths::id("decision"),"time":paths::now(),"summary":text(args,"summary")?,"rationale":args["rationale"]})),
        "state.block"=>{host.blockers.insert(text(args,"blocker_id")?.into(),text(args,"reason")?.into());},
        "state.unblock"=>{if host.blockers.remove(text(args,"blocker_id")?).is_none(){bail!("blocker_not_found");}},
        "state.checkpoint"=>host.decisions.push(json!({"id":paths::id("checkpoint"),"time":paths::now(),"stopped_at":text(args,"stopped_at")?,"next":args["next"]})),
        _=>bail!("unknown_capability")
    }
    run.updated_at = paths::now();
    store.save(&run, name)?;
    Ok(
        json!({"run":crate::progress::summary(&run),"decisions":run.host.as_ref().map(|h|&h.decisions),"blockers":run.host.as_ref().map(|h|&h.blockers)}),
    )
}
