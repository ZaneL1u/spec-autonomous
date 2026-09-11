//! One deterministic service shared by CLI and MCP. High-level capabilities
//! compose these granular operations; no transport owns a model or agent.
use crate::{
    Framework,
    config::Config,
    engine,
    git::{self, Repository},
    model::{Milestone, Phase},
    paths, provider,
    state::{Lease, Store},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};
pub mod catalog;
mod checks;
mod documents;
mod git_tools;
mod lifecycle;
mod queries;
mod roadmap;
pub mod views;
mod workspaces;

fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .with_context(|| format!("invalid_arguments: {key} is required"))
}
fn optional(args: &Value, key: &str) -> Option<String> {
    args[key].as_str().map(str::to_owned)
}
fn framework_arg(args: &Value) -> Result<Option<Framework>> {
    match args["framework"].as_str() {
        None | Some("auto") => Ok(None),
        Some("openspec") => Ok(Some(Framework::Openspec)),
        Some("speckit") => Ok(Some(Framework::Speckit)),
        _ => bail!("invalid_framework"),
    }
}
fn source_arg(root: &Path, args: &Value, framework: Framework) -> Result<String> {
    match framework {
        Framework::Openspec => Ok(args["change"]
            .as_str()
            .or_else(|| args["selector"].as_str())
            .context("selection_required: change selector required")?
            .into()),
        Framework::Speckit => provider::select_feature(
            root,
            args["feature"]
                .as_str()
                .or_else(|| args["selector"].as_str()),
        ),
    }
}
fn resolve_milestone(root: &Path, args: &Value) -> Result<Milestone> {
    if let Some(id) = args["milestone_id"].as_str() {
        return provider::load_milestone(root, id);
    }
    let framework = provider::framework(root, framework_arg(args)?)?;
    if let Some(goal) = args["goal"].as_str() {
        return Ok(engine::goal_milestone(
            optional(args, "id"),
            goal.into(),
            framework,
        ));
    }
    Ok(engine::source_milestone(
        framework,
        &source_arg(root, args, framework)?,
    ))
}
fn select_phase<'a>(milestone: &'a Milestone, args: &Value) -> Result<&'a Phase> {
    if let Some(id) = args["phase_id"].as_str() {
        return milestone
            .phases
            .iter()
            .find(|p| p.id == id || p.label == id)
            .context("phase_not_found");
    }
    if milestone.phases.len() != 1 {
        bail!("selection_required: phase_id is required for multiple phases");
    }
    milestone.phases.first().context("phase_not_found")
}
pub fn guard_project_idle(root: &Path) -> Result<()> {
    let repo = Repository::discover(root)?;
    if let Some(store) = Store::open(&repo, false)? {
        for run in store.list()? {
            let project = Path::new(&run.origin).join(&run.project_relative);
            if project.canonicalize().ok() == root.canonicalize().ok()
                && run.host.is_some()
                && (!run.terminal()
                    || run.attempts.iter().any(|a| {
                        matches!(
                            a.status.as_str(),
                            "issued" | "claimed" | "submitted" | "receiving"
                        )
                    }))
                && (!matches!(run.status.as_str(), "handed_off" | "plan_ready")
                    || run
                        .attempts
                        .iter()
                        .any(|a| matches!(a.status.as_str(), "issued" | "claimed" | "submitted")))
            {
                bail!(
                    "project_in_use: run {} owns active planning or work; pause/revoke or finish it first",
                    run.id
                );
            }
        }
    }
    Ok(())
}
pub fn invoke(root: &Path, name: &str, args: &Value) -> Result<Value> {
    catalog::validate(name, args)?;
    let root = crate::detect(root, None)?.root;
    let mut result = match name {
        "inspect" => queries::inspect(&root, args)?,
        "progress" => crate::progress::snapshot(&root)?,
        "next" => engine::next(&root, args["run_id"].as_str())?,
        "doctor" => lifecycle::doctor(&root)?,
        "archive" => lifecycle::archive(&root, args)?,
        "repair" => lifecycle::repair(&root, args)?,
        "prepare" => {
            let cancel = Arc::new(AtomicBool::new(false));
            let mut observer = |_: &crate::model::Run, _: &str| {};
            let run = if let Some(id) = args["run_id"].as_str() {
                engine::resume_with(
                    &root,
                    id,
                    engine::ResumeOptions {
                        mode: optional(args, "mode"),
                        reload_config: args["reload_config"] == true,
                        extend_seconds: args["extend_seconds"].as_u64().unwrap_or(0),
                        max_attempts: args["max_attempts"].as_u64().map(|n| n as u32),
                    },
                    cancel,
                    &mut observer,
                )?
            } else {
                let supplied: Option<crate::model::Plan> = args
                    .get("plan")
                    .map(|p| serde_json::from_value(p.clone()))
                    .transpose()?;
                let m = if let Some(plan) = &supplied {
                    plan.milestone
                        .clone()
                        .context("invalid_plan: native milestone binding required")?
                } else {
                    resolve_milestone(&root, args)?
                };
                let mut config = Config::load(&root)?;
                if let Some(n) = args["max_workers"].as_u64() {
                    config.execution.max_workers = n as usize;
                    config.host.max_concurrency = n as usize;
                }
                if let Some(v) = args["delivery"].as_str() {
                    config.execution.delivery = v.into();
                }
                let mode = optional(args, "mode").unwrap_or_else(|| config.execution.mode.clone());
                engine::start(
                    &root,
                    engine::Start {
                        milestone: m,
                        range: crate::model::Range {
                            from: optional(args, "from"),
                            to: optional(args, "to"),
                            only: optional(args, "only"),
                        },
                        mode,
                        create_roadmap: args.get("goal").is_some(),
                        plan: supplied,
                    },
                    config,
                    cancel,
                    &mut observer,
                )?
            };
            engine::work_view(&run, &Repository::discover(&root)?.runtime()?)?
        }
        "apply-result" | "task.complete" => {
            let result = serde_json::from_value(args["result"].clone())?;
            let owner = serde_json::from_value(args["host"].clone())?;
            engine::apply_result(
                &root,
                text(args, "token")?,
                result,
                owner,
                Arc::new(AtomicBool::new(false)),
                &mut |_, _| {},
            )?
        }
        "work.claim" | "task.claim" => engine::claim(
            &root,
            text(args, "run_id")?,
            text(args, "request_id")?,
            text(args, "token")?,
            serde_json::from_value(args["host"].clone())?,
        )?,
        "work.heartbeat" => engine::heartbeat(
            &root,
            text(args, "run_id")?,
            text(args, "request_id")?,
            text(args, "token")?,
        )?,
        "work.revoke" => engine::revoke(
            &root,
            text(args, "run_id")?,
            text(args, "request_id")?,
            text(args, "token")?,
            args["host_stopped"] == true,
            text(args, "reason")?,
        )?,
        "work.context" => {
            engine::work_context(&root, text(args, "run_id")?, text(args, "request_id")?)?
        }
        "run.pause" => engine::host_control(&root, text(args, "run_id")?, "pause")?,
        "run.cancel" => engine::host_control(&root, text(args, "run_id")?, "cancel")?,
        "run.cleanup" => crate::cleanup::cleanup(&root, text(args, "run_id")?)?,
        "hook.resolve" => crate::progress::public_run(&engine::resolve_hook(
            &root,
            text(args, "run_id")?,
            text(args, "key")?,
            text(args, "outcome")?,
            text(args, "evidence")?,
        )?),
        "native.instructions" => {
            let m = resolve_milestone(&root, args)?;
            let p = select_phase(&m, args)?;
            let s = provider::inspect(
                &root,
                m.framework,
                &p.source.selector,
                &Config::load(&root)?,
            )?;
            json!({"framework":m.framework,"selector":p.source.selector,"source_hash":s.source_hash,"next_action":s.next_action,"diagnostics":s.diagnostics})
        }
        "native.create" => {
            guard_project_idle(&root)?;
            let f = provider::framework(&root, framework_arg(args)?)?;
            let selected = source_arg(&root, args, f)?;
            let repo = Repository::discover(&root)?;
            let _lease = Lease::acquire(&repo)?;
            let s = provider::inspect(&root, f, &selected, &Config::load(&root)?)?;
            if s.next_action.kind != "create" && f == Framework::Openspec {
                bail!("source_exists");
            }
            provider::create_source(&root, f, &selected, &Config::load(&root)?)?;
            json!({"framework":f,"selector":selected,"created":true})
        }
        _ if name.starts_with("verify.") || name == "audit.open" || name == "history.summaries" => {
            checks::invoke(&root, name, args)?
        }
        _ if name.starts_with("git.") => git_tools::invoke(&root, name, args)?,
        "capabilities" => catalog::list(args["all"] == true),
        _ if name.starts_with("document.")
            || name.starts_with("frontmatter.")
            || name == "toml.patch" =>
        {
            documents::invoke(&root, name, args)?
        }
        _ if name.starts_with("roadmap.") => roadmap::invoke(&root, name, args)?,
        _ if name.starts_with("worktree.") => workspaces::invoke(&root, name, args)?,
        _ if name.starts_with("state.") || name.starts_with("task.") || name == "history.get" => {
            queries::state(&root, name, args)?
        }
        _ => bail!("unknown_capability"),
    };
    if args["view"] != "full" {
        result = views::compact(result);
    }
    views::select_page(&mut result, args)?;
    Ok(result)
}
