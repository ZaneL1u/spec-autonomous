//! Change scoping shared by incremental audits and affected-scope verification. Each
//! rule answers one question: may a conclusion reached on an earlier commit stand for
//! the current one? The answer is "no" whenever it cannot be shown from the Git diff
//! and the write sets the plan declared, so unknown ownership always means "redo".
use crate::{git, model::Run, plan};
use std::path::Path;

/// Project-relative paths that differ between two commits, renames listed under both
/// names. None when `from` is not an ancestor of `to`, Git fails, or a change lies
/// outside the project directory.
pub fn changed_between(
    checkout: &Path,
    project_relative: &str,
    from: &str,
    to: &str,
) -> Option<Vec<String>> {
    if from.is_empty() || !git::ancestor(checkout, from, to) {
        return None;
    }
    let text = git::command(
        checkout,
        &["diff", "--no-renames", "--name-only", "-z", from, to, "--"],
        None,
    )
    .ok()?;
    let prefix = prefix(project_relative);
    let mut files = vec![];
    for path in text.split('\0').filter(|p| !p.is_empty()) {
        files.push(path.strip_prefix(&prefix)?.to_string());
    }
    Some(files)
}

fn prefix(project_relative: &str) -> String {
    if project_relative == "." {
        String::new()
    } else {
        format!("{}/", project_relative.trim_end_matches('/'))
    }
}

/// Inputs that can change any check or requirement: dependency manifests and lockfiles,
/// shared test configuration, submodule pointers, coordinator configuration, roadmap
/// and native tooling. Recorded plans are bookkeeping, not inputs.
pub fn global_input(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    (path.starts_with(".spec-autonomous/") && !path.starts_with(".spec-autonomous/plans/"))
        || path.starts_with(".specify/")
        || path.starts_with(".agents/")
        || path.starts_with(".codex/")
        || path.starts_with(".claude/")
        || path == "AGENTS.md"
        || path == "openspec/config.yaml"
        || [
            ".gitmodules",
            ".gitattributes",
            "package.json",
            "bun.lock",
            "bun.lockb",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
            "package-lock.json",
            "yarn.lock",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "pyproject.toml",
            "uv.lock",
            "poetry.lock",
            "requirements.txt",
            "setup.py",
            "setup.cfg",
            "tox.ini",
            "pytest.ini",
            "conftest.py",
            "go.mod",
            "go.sum",
            "tsconfig.json",
            "Makefile",
        ]
        .contains(&name)
}

/// Coordinator bookkeeping that changes no behavior: recorded execution plans, and a
/// native `tasks.md` whose only changes are checkbox flips and appended lines (accepted
/// tasks and convergence repairs). A new or rewritten tasks.md is not bookkeeping.
pub fn bookkeeping(
    checkout: &Path,
    project_relative: &str,
    from: &str,
    to: &str,
    path: &str,
) -> bool {
    if path.starts_with(".spec-autonomous/plans/") {
        return true;
    }
    if path.rsplit('/').next() != Some("tasks.md") {
        return false;
    }
    let full = format!("{}{path}", prefix(project_relative));
    let read = |rev: &str| git::command(checkout, &["show", &format!("{rev}:{full}")], None).ok();
    let (Some(old), Some(new)) = (read(from), read(to)) else {
        return false;
    };
    let (old, new) = (unchecked(&old), unchecked(&new));
    // Whole lines only: extending the last task's text is an edit, not an append.
    !old.trim().is_empty() && (new == old || new.starts_with(&format!("{old}\n")))
}

/// Text with every task checkbox reset, so checking a task off is not a change.
fn unchecked(text: &str) -> String {
    text.lines()
        .map(|line| {
            let body = line.trim_start();
            let indent = &line[..line.len() - body.len()];
            for marker in ["- [x]", "- [X]", "* [x]", "* [X]"] {
                if let Some(rest) = body.strip_prefix(marker) {
                    return format!("{indent}{}[ ]{rest}", &marker[..2]);
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// True when some plan task of this run declared a write set covering `path`. Files
/// outside every declared write set have unknown owners.
pub fn owned(run: &Run, path: &str) -> bool {
    run.plans
        .values()
        .flat_map(|p| &p.tasks)
        .any(|t| !t.writes.is_empty() && plan::allowed(path, &t.writes))
}

/// Why a change set invalidates an earlier conclusion that depends on `related`
/// (paths or write patterns), or None when it does not. Native documents in `context`
/// other than bookkeeping, global inputs and files nobody declared all invalidate.
#[allow(clippy::too_many_arguments)]
pub fn invalidated(
    run: &Run,
    checkout: &Path,
    from: &str,
    to: &str,
    changed: &[String],
    related: &[String],
    context: &[String],
) -> Option<String> {
    for path in changed {
        if plan::allowed(path, related) {
            return Some(format!("{path} is in scope"));
        }
        if bookkeeping(checkout, &run.project_relative, from, to, path) {
            continue;
        }
        if global_input(path) {
            return Some(format!("{path} is a global input"));
        }
        if context.iter().any(|c| c == path) {
            return Some(format!("native document {path} changed"));
        }
        if !owned(run, path) {
            return Some(format!("{path} is outside every declared write set"));
        }
    }
    None
}
