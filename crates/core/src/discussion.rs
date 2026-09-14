//! Deterministic, host-renderable phase discussion cards.
use crate::{
    git::Repository,
    model::{Phase, Run},
    paths, provider,
    state::{Lease, Store},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn phase<'a>(run: &'a Run, id: Option<&str>) -> Result<&'a Phase> {
    let id = id
        .or(run.current_phase.as_deref())
        .context("phase_required: choose a phase_id")?;
    run.milestone
        .phases
        .iter()
        .find(|p| p.id == id || p.label == id)
        .context("phase_not_found")
}
fn cards(run: &Run, p: &Phase, snapshot: &crate::model::Snapshot) -> Value {
    let key = |suffix: &str| format!("{}-{suffix}", p.id.to_lowercase());
    let mut out = vec![];
    let has_checks = !p.verification.is_empty()
        || !run.config.verification.is_empty()
        || !run.milestone.verification.is_empty();
    out.push(json!({"id":key("verification"),"question":"Which verification contract should this phase use?","recommended":if has_checks {"native-checks"} else {"explicit-test"},"alternatives":[{"id":"native-checks","label":"Use the phase/project checks","description":"Keep existing verification commands and native requirements."},{"id":"explicit-test","label":"Add an explicit test command","description":"Choose a concrete test file or supported discovery command."}],"why":"A phase needs an executable acceptance gate before implementation is dispatched.","impact":"The choice becomes the phase verification input for planning.","status":"unresolved"}));
    let has_parallel = snapshot.tasks.iter().filter(|t| !t.done).count() > 1;
    if has_parallel {
        out.push(json!({"id":key("ordering"),"question":"Can independent native tasks run in parallel?","recommended":"respect-native-order","alternatives":[{"id":"respect-native-order","label":"Respect native order","description":"Keep conservative order unless the native task metadata marks siblings parallel."},{"id":"parallel-siblings","label":"Parallelize marked siblings","description":"Allow only explicit [P] siblings in the same phase to run together."}],"why":"The native task graph remains authoritative.","impact":"Changes scheduling only; dependency and write-set checks still apply.","status":"unresolved"}));
    }
    json!({"phase_id":p.id,"source_hash":snapshot.source_hash,"cards":out,"decisions":run.host.as_ref().map(|h| h.decisions.clone()).unwrap_or_default(),"auto_available":true})
}
pub fn next(root: &Path, run_id: &str, phase_id: Option<&str>) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let store = Store::open(&repo, false)?.context("run_not_found")?;
    let run = store.get(run_id)?;
    let p = phase(&run, phase_id)?;
    let project = Path::new(&run.integration).join(&run.project_relative);
    let snapshot = provider::inspect(
        &project,
        run.milestone.framework,
        &p.source.selector,
        &run.config,
    )?;
    Ok(cards(&run, p, &snapshot))
}
pub fn apply(
    root: &Path,
    run_id: &str,
    phase_id: Option<&str>,
    source_hash: &str,
    selections: &[Value],
    auto: bool,
) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let _lease = Lease::acquire(&repo)?;
    let mut store = Store::open(&repo, true)?.unwrap();
    let mut run = store.get(run_id)?;
    let p = phase(&run, phase_id)?;
    let phase_key = p.id.clone();
    let project = Path::new(&run.integration).join(&run.project_relative);
    let snapshot = provider::inspect(
        &project,
        run.milestone.framework,
        &p.source.selector,
        &run.config,
    )?;
    if snapshot.source_hash != source_hash {
        bail!("source_drift: discussion source changed; preview the phase cards again")
    }
    let preview = cards(&run, p, &snapshot);
    let cards = preview["cards"].as_array().unwrap();
    let mut choices = selections.to_vec();
    if auto {
        for card in cards {
            if let Some(id) = card["recommended"].as_str() {
                choices.push(json!({"card_id":card["id"],"option_id":id,"mode":"auto"}));
            }
        }
    }
    let mut applied = vec![];
    for choice in choices {
        let cid = choice["card_id"]
            .as_str()
            .context("invalid_arguments: selection.card_id")?;
        let oid = choice["option_id"]
            .as_str()
            .context("invalid_arguments: selection.option_id")?;
        let card = cards
            .iter()
            .find(|c| c["id"] == cid)
            .context("discussion_card_not_found")?;
        let options = card["alternatives"].as_array().unwrap();
        if !options.iter().any(|o| o["id"] == oid) && card["recommended"] != oid {
            bail!("discussion_option_not_found: {cid}/{oid}")
        }
        let decision = json!({"kind":"smart-discuss","phase_id":phase_key,"card_id":cid,"option_id":oid,"source_hash":source_hash,"mode":choice["mode"].as_str().unwrap_or("click"),"at":paths::now()});
        if let Some(existing) = run
            .host
            .as_ref()
            .unwrap()
            .decisions
            .iter()
            .find(|d| d["card_id"] == cid)
        {
            if existing["option_id"] != oid {
                bail!("discussion_decision_locked: {cid} already chose a different option")
            }
            continue;
        }
        run.host.as_mut().unwrap().decisions.push(decision.clone());
        applied.push(decision);
    }
    let context_dir = paths::inside(
        &project,
        &format!(".spec-autonomous/milestones/{}/phases", run.milestone.id),
    )?;
    fs::create_dir_all(&context_dir)?;
    let context = context_dir.join(format!("{}-CONTEXT.md", phase_key));
    let log = context_dir.join(format!("{}-DISCUSSION-LOG.md", phase_key));
    let body = run
        .host
        .as_ref()
        .unwrap()
        .decisions
        .iter()
        .filter(|d| d["phase_id"] == phase_key)
        .map(|d| {
            format!(
                "- `{}` = `{}` ({})\n",
                d["card_id"], d["option_id"], d["mode"]
            )
        })
        .collect::<String>();
    paths::atomic_write(
        &context,
        format!(
            "# Phase {} discussion\n\n## Decisions\n\n{}",
            phase_key, body
        ),
    )?;
    paths::atomic_write(&log, serde_json::to_string_pretty(&applied)?)?;
    run.updated_at = paths::now();
    store.save(&run, "discussion_applied")?;
    Ok(
        json!({"run_id":run_id,"phase_id":phase_key,"source_hash":source_hash,"applied":applied,"unresolved":cards.iter().filter(|c|!run.host.as_ref().unwrap().decisions.iter().any(|d|d["card_id"]==c["id"])).map(|c|c["id"].clone()).collect::<Vec<_>>()}),
    )
}
