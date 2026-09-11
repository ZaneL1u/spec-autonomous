//! Pure graph validation and scheduling. No disk or process access.
use crate::{config::validate_check, model::*, paths};
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};

fn acyclic(edges: &BTreeMap<String, Vec<String>>) -> Result<()> {
    let mut done = BTreeSet::new();
    loop {
        let before = done.len();
        for (id, deps) in edges {
            if deps.iter().any(|d| !edges.contains_key(d)) {
                bail!("invalid_plan: missing dependency of {id}");
            }
            if deps.iter().all(|d| done.contains(d)) {
                done.insert(id.clone());
            }
        }
        if done.len() == edges.len() {
            return Ok(());
        }
        if done.len() == before {
            bail!("invalid_plan: dependency cycle");
        }
    }
}
pub fn validate_milestone(m: &Milestone) -> Result<()> {
    paths::valid_id(&m.id)?;
    if m.schema_version != SCHEMA || m.goal.trim().is_empty() || m.phases.is_empty() {
        bail!("invalid_milestone: missing goal/phases or unsupported schema");
    }
    let mut ids = BTreeMap::new();
    let mut labels = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for p in &m.phases {
        paths::valid_id(&p.id)?;
        if p.label.is_empty()
            || !labels.insert(p.label.clone())
            || ids.insert(p.id.clone(), p.depends_on.clone()).is_some()
            || !sources.insert(p.source.selector.clone())
        {
            bail!("invalid_milestone: duplicate phase, label or source");
        }
        match m.framework {
            crate::Framework::Openspec => {
                if p.source.kind != "openspec-change" {
                    bail!("invalid_milestone: provider mismatch");
                }
                paths::valid_id(&p.source.selector)?;
            }
            crate::Framework::Speckit => {
                if p.source.kind != "speckit-feature" {
                    bail!("invalid_milestone: provider mismatch");
                }
                paths::relative(&p.source.selector)?;
            }
        }
        for c in &p.verification {
            validate_check(c)?;
        }
    }
    for c in &m.verification {
        validate_check(c)?;
    }
    acyclic(&ids)
}
pub fn select(m: &Milestone, range: &Range, completed: &[String]) -> Result<Vec<String>> {
    validate_milestone(m)?;
    if range.only.is_some() && (range.from.is_some() || range.to.is_some()) {
        bail!("invalid_range: only conflicts with from/to");
    }
    let index = |key: &str| -> Result<usize> {
        let matches: Vec<_> = m
            .phases
            .iter()
            .enumerate()
            .filter(|(_, p)| p.id == key || p.label == key)
            .collect();
        if matches.len() != 1 {
            bail!("selection_required: unknown or ambiguous phase {key}");
        }
        Ok(matches[0].0)
    };
    let first = if let Some(v) = range.only.as_ref().or(range.from.as_ref()) {
        index(v)?
    } else {
        0
    };
    let last = if let Some(v) = range.only.as_ref().or(range.to.as_ref()) {
        index(v)?
    } else {
        m.phases.len() - 1
    };
    if first > last {
        bail!("invalid_range: from follows to");
    }
    let selected: Vec<_> = m.phases[first..=last]
        .iter()
        .map(|p| p.id.clone())
        .collect();
    for p in &m.phases[first..=last] {
        for d in &p.depends_on {
            if !selected.contains(d) && !completed.contains(d) {
                bail!("prerequisite_outside_range: phase {} requires {d}", p.id);
            }
        }
    }
    Ok(selected)
}
pub fn validate_plan(plan: &Plan, snapshot: &Snapshot) -> Result<()> {
    if plan.schema_version != SCHEMA
        || plan.source_hash != snapshot.source_hash
        || plan.tasks.is_empty()
    {
        bail!("invalid_plan: source revision or empty tasks");
    }
    let mut edges = BTreeMap::new();
    let mut coverage = BTreeSet::new();
    let source: BTreeSet<_> = snapshot
        .tasks
        .iter()
        .filter(|t| !t.done)
        .map(|t| t.id.clone())
        .collect();
    for t in &plan.tasks {
        paths::valid_id(&t.id)?;
        if t.description.trim().is_empty()
            || t.source_ids.is_empty()
            || edges.insert(t.id.clone(), t.depends_on.clone()).is_some()
        {
            bail!("invalid_plan: duplicate or ungrounded task");
        }
        for s in &t.source_ids {
            if !source.contains(s) {
                bail!("invalid_plan: unknown source task {s}");
            }
            coverage.insert(s.clone());
        }
        for p in t.writes.iter().chain(&t.reads) {
            paths::relative(p)?;
            let normalized = paths::relative(p)?
                .components()
                .filter_map(|c| {
                    if let std::path::Component::Normal(s) = c {
                        Some(s.to_string_lossy().into_owned())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
            if normalized == ".git"
                || normalized.starts_with(".git/")
                || normalized == ".spec-autonomous"
                || normalized.starts_with(".spec-autonomous/")
            {
                bail!("invalid_plan: reserved write scope");
            }
            globset::Glob::new(p)?;
        }
        for c in &t.verification {
            validate_check(c)?;
        }
    }
    if coverage != source {
        bail!("invalid_plan: missing source task coverage");
    }
    acyclic(&edges)?;
    // Native task order and stage barriers remain binding. Only explicit parallel
    // siblings in the same native phase can omit source-order dependencies.
    for (i, current) in snapshot
        .tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.done && snapshot.framework == crate::Framework::Speckit)
    {
        for prior in snapshot.tasks[..i].iter().filter(|t| !t.done) {
            if current.parallel && prior.parallel && current.phase == prior.phase {
                continue;
            }
            let a: Vec<_> = plan
                .tasks
                .iter()
                .filter(|t| t.source_ids.contains(&current.id))
                .collect();
            let b: Vec<_> = plan
                .tasks
                .iter()
                .filter(|t| t.source_ids.contains(&prior.id))
                .collect();
            for x in &a {
                for y in &b {
                    if x.id != y.id && !depends(&edges, &x.id, &y.id, &mut BTreeSet::new()) {
                        bail!("invalid_plan: native order {} requires {}", x.id, y.id);
                    }
                }
            }
        }
    }
    Ok(())
}
fn depends(
    edges: &BTreeMap<String, Vec<String>>,
    a: &str,
    b: &str,
    seen: &mut BTreeSet<String>,
) -> bool {
    if !seen.insert(a.into()) {
        return false;
    }
    edges
        .get(a)
        .is_some_and(|ds| ds.iter().any(|d| d == b || depends(edges, d, b, seen)))
}
fn prefix(pattern: &str) -> &str {
    pattern
        .split(['*', '?', '[', '{'])
        .next()
        .unwrap_or("")
        .trim_end_matches('/')
}
fn normalized(pattern: &str) -> String {
    std::path::Path::new(pattern)
        .components()
        .filter_map(|c| {
            if let std::path::Component::Normal(p) = c {
                Some(p.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}
fn overlaps(a: &str, b: &str) -> bool {
    let a_normal = normalized(a);
    let b_normal = normalized(b);
    let a = a_normal.as_str();
    let b = b_normal.as_str();
    let wildcard_a = a.contains(['*', '?', '[', '{']);
    let wildcard_b = b.contains(['*', '?', '[', '{']);
    let (a, b) = (prefix(a), prefix(b));
    a.is_empty()
        || b.is_empty()
        || a == b
        || a.starts_with(&format!("{b}/"))
        || b.starts_with(&format!("{a}/"))
        || (wildcard_a && b.starts_with(a))
        || (wildcard_b && a.starts_with(b))
}
pub fn conflicts(a: &Task, b: &Task) -> bool {
    a.writes.is_empty()
        || b.writes.is_empty()
        || a.writes
            .iter()
            .any(|x| b.writes.iter().chain(&b.reads).any(|y| overlaps(x, y)))
        || b.writes
            .iter()
            .any(|x| a.reads.iter().any(|y| overlaps(x, y)))
}
pub fn allowed(path: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| {
        globset::Glob::new(p).is_ok_and(|g| g.compile_matcher().is_match(path))
            || p == path
            || path.starts_with(&format!("{}/", p.trim_end_matches('/')))
    })
}
pub fn ready<'a>(
    plan: &'a Plan,
    done: &[String],
    running: &[&Task],
    limit: usize,
) -> Vec<&'a Task> {
    let mut selected: Vec<&Task> = vec![];
    for task in &plan.tasks {
        if selected.len() + running.len() >= limit {
            break;
        }
        if done.contains(&task.id)
            || running.iter().any(|x| x.id == task.id)
            || !task.depends_on.iter().all(|d| done.contains(d))
        {
            continue;
        }
        if running
            .iter()
            .chain(selected.iter())
            .any(|other| conflicts(task, other))
        {
            continue;
        }
        selected.push(task);
    }
    selected
}
