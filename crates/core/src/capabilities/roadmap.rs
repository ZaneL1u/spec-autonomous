use super::*;
use crate::model::{Milestone, Phase};

pub fn invoke(root: &Path, name: &str, args: &Value) -> Result<Value> {
    if name == "roadmap.import" {
        guard_project_idle(root)?;
        let milestone: Milestone = serde_json::from_value(args["milestone"].clone())?;
        provider::framework(root, Some(milestone.framework))?;
        crate::plan::validate_milestone(&milestone)?;
        let path = provider::milestone_path(root, &milestone.id)?;
        let expected = args["expected_hash"].as_str().unwrap_or("");
        let repo = Repository::discover(root)?;
        let _lock = Lease::acquire(&repo)?;
        guard_project_idle(root)?;
        let actual = if path.exists() {
            paths::hash(fs::read(&path)?)
        } else {
            String::new()
        };
        if expected != actual {
            bail!("source_drift: roadmap manifest changed");
        }
        provider::save_milestone(root, &milestone)?;
        return Ok(json!({"milestone":milestone,"source_hash":paths::hash(fs::read(path)?)}));
    }
    let id = text(args, "milestone_id")?;
    let path = provider::milestone_path(root, id)?;
    let before = fs::read_to_string(&path)?;
    let mut milestone = provider::load_milestone(root, id)?;
    if name == "roadmap.get" {
        return Ok(json!({"milestone":milestone,"source_hash":paths::hash(&before),"path":path}));
    }
    if name == "roadmap.select" {
        let range = crate::model::Range {
            from: optional(args, "from"),
            to: optional(args, "to"),
            only: optional(args, "only"),
        };
        let completed = verified_phases(root, &milestone)?;
        return Ok(
            json!({"selected":crate::plan::select(&milestone,&range,&completed)?,"completed":completed,"source_hash":paths::hash(&before)}),
        );
    }
    guard_project_idle(root)?;
    if text(args, "expected_hash")? != paths::hash(&before) {
        bail!("source_drift: roadmap changed");
    }
    match name {
        "roadmap.add" | "roadmap.insert" => {
            let phase: Phase = serde_json::from_value(args["phase"].clone())?;
            if name == "roadmap.insert" {
                let after = text(args, "after")?;
                let at = milestone
                    .phases
                    .iter()
                    .position(|p| p.id == after || p.label == after)
                    .context("phase_not_found")?;
                milestone.phases.insert(at + 1, phase);
            } else {
                milestone.phases.push(phase);
            }
        }
        "roadmap.remove" => {
            let phase = text(args, "phase_id")?;
            let before = milestone.phases.len();
            milestone.phases.retain(|p| p.id != phase);
            if before == milestone.phases.len() {
                bail!("phase_not_found");
            }
        }
        "roadmap.render" => {}
        _ => bail!("unknown_capability"),
    }
    if name != "roadmap.render" {
        milestone.revision = milestone
            .revision
            .checked_add(1)
            .context("revision_overflow")?;
    }
    crate::plan::validate_milestone(&milestone)?;
    let repo = Repository::discover(root)?;
    let _lock = Lease::acquire(&repo)?;
    guard_project_idle(root)?;
    if paths::hash(fs::read(&path)?) != text(args, "expected_hash")? {
        bail!("source_drift: concurrent roadmap write");
    }
    provider::save_milestone(root, &milestone)?;
    Ok(json!({"milestone":milestone,"source_hash":paths::hash(fs::read(path)?)}))
}
fn verified_phases(root: &Path, m: &Milestone) -> Result<Vec<String>> {
    let repo = Repository::discover(root)?;
    let Some(store) = Store::open(&repo, false)? else {
        return Ok(vec![]);
    };
    let head = repo.head()?;
    let mut completed = vec![];
    for run in store
        .list()?
        .iter()
        .filter(|r| r.milestone.id == m.id && r.accepted_head == head && r.terminal())
    {
        for p in &m.phases {
            if run.completed_phases.contains(&p.id)
                && provider::inspect(root, m.framework, &p.source.selector, &Config::load(root)?)
                    .is_ok_and(|s| run.phase_hashes.get(&p.id) == Some(&s.source_hash))
            {
                completed.push(p.id.clone());
            }
        }
    }
    completed.sort();
    completed.dedup();
    Ok(completed)
}
