//! Read-only command readiness checks. Readiness is never verification evidence.
use crate::{
    config,
    model::{Check, Milestone, Plan},
    paths, plan,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub task_id: Option<String>,
    pub check_index: Option<usize>,
}
#[derive(Debug, Clone, Serialize)]
pub struct CheckReadiness {
    pub task_id: Option<String>,
    pub check_index: usize,
    pub argv: Vec<String>,
    pub cwd: String,
    pub executable: Option<PathBuf>,
    pub status: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// False means a known configuration problem needs correction before dispatch.
    pub ready: bool,
    pub status: String,
    pub executed: bool,
    pub checks: Vec<CheckReadiness>,
    pub diagnostics: Vec<Diagnostic>,
}
impl Default for Report {
    fn default() -> Self {
        Self {
            ready: true,
            status: "ready".into(),
            executed: false,
            checks: vec![],
            diagnostics: vec![],
        }
    }
}
impl Report {
    fn note(
        &mut self,
        code: &str,
        severity: &str,
        message: String,
        task_id: Option<&str>,
        check_index: Option<usize>,
    ) {
        self.diagnostics.push(Diagnostic {
            code: code.into(),
            severity: severity.into(),
            message,
            task_id: task_id.map(str::to_owned),
            check_index,
        });
    }
    fn finish(&mut self) {
        self.ready = !self.diagnostics.iter().any(|d| d.severity == "error");
        self.status = if !self.ready {
            "needs_input"
        } else if self.checks.iter().any(|c| c.status == "deferred") {
            "deferred"
        } else {
            "ready"
        }
        .into();
    }
}

pub fn planning_guidance() -> &'static str {
    "Plan coherent deliverables, combining related implementation and its tests in one task with multiple source_ids where native order permits. Do not split a small function into serial scaffolding/export/test/assertion tasks or create a separate test-only task merely to run checks already attached to implementation. Keep genuine independent native acceptance work and phase barriers intact. Declare every file needed to repair a task in writes; an empty write set has no declared repair scope, so declare explicit writes or request plan correction. Inspect the actual runtime versions, existing package scripts and check cwd before choosing verification argv. Use explicit Node test files or supported test discovery; a directory operand such as node --test test/ is version-sensitive and is not a portable test command. Argv executes without shell expansion. Check small fixture expectations independently, rather than copying an untested arithmetic guess. Future implementation/test files may be deferred; never call preflight a passed test or execute unbuilt future checks just to obtain an exit code."
}

pub fn environment(root: &Path) -> serde_json::Value {
    environment_with_overrides(root, &BTreeMap::new())
}

pub fn environment_with_overrides(
    root: &Path,
    overrides: &BTreeMap<String, String>,
) -> serde_json::Value {
    let search = SearchEnvironment::new(overrides);
    let commands: std::collections::BTreeMap<_, _> =
        ["node", "bun", "npm", "cargo", "python3", "git"]
            .into_iter()
            .map(|name| (name, resolve_executable(root, name, &search)))
            .collect();
    serde_json::json!({"os":env::consts::OS,"arch":env::consts::ARCH,"executables":commands,"runtime_versions":"inspect selected runtime before finalizing commands","argv_shell_expansion":false,"verification_executed":false})
}

pub fn inspect_checks(root: &Path, checks: &[Check], planned_writes: &[String]) -> Report {
    inspect_checks_with_environment(root, checks, planned_writes, &BTreeMap::new())
}

pub fn inspect_checks_with_environment(
    root: &Path,
    checks: &[Check],
    planned_writes: &[String],
    overrides: &BTreeMap<String, String>,
) -> Report {
    let mut report = Report::default();
    inspect_into(
        &mut report,
        root,
        checks,
        planned_writes,
        None,
        &SearchEnvironment::new(overrides),
    );
    report.finish();
    report
}

/// A roadmap precedes concrete write sets. Missing project targets are explicitly
/// deferred, while a missing external executable is still an environment issue.
pub fn inspect_milestone(root: &Path, milestone: &Milestone) -> Report {
    inspect_milestone_with_environment(root, milestone, &BTreeMap::new())
}

pub fn inspect_milestone_with_environment(
    root: &Path,
    milestone: &Milestone,
    overrides: &BTreeMap<String, String>,
) -> Report {
    let checks: Vec<_> = milestone
        .verification
        .iter()
        .chain(milestone.phases.iter().flat_map(|p| &p.verification))
        .cloned()
        .collect();
    inspect_checks_with_environment(root, &checks, &["**".into()], overrides)
}

pub fn inspect_plan(root: &Path, execution: &Plan, configured_checks: &[Check]) -> Report {
    inspect_plan_with_environment(root, execution, configured_checks, &BTreeMap::new())
}

pub fn inspect_plan_with_environment(
    root: &Path,
    execution: &Plan,
    configured_checks: &[Check],
    overrides: &BTreeMap<String, String>,
) -> Report {
    let mut report = Report::default();
    let search = SearchEnvironment::new(overrides);
    let all_writes: Vec<_> = execution
        .tasks
        .iter()
        .flat_map(|t| t.writes.clone())
        .collect();
    for task in &execution.tasks {
        // Prerequisite output is available by the time a dependent task is checked.
        let mut available_writes = task.writes.clone();
        let mut pending = task.depends_on.clone();
        let mut visited = std::collections::BTreeSet::new();
        while let Some(id) = pending.pop() {
            if visited.insert(id.clone())
                && let Some(prior) = execution.tasks.iter().find(|t| t.id == id)
            {
                available_writes.extend(prior.writes.clone());
                pending.extend(prior.depends_on.clone());
            }
        }
        inspect_into(
            &mut report,
            root,
            &task.verification,
            &available_writes,
            Some(&task.id),
            &search,
        );
        if task.writes.is_empty() {
            report.note("empty_write_scope", "warning", "This task has no declared repair files. Prefer attaching verification to its implementation task, or explicitly declare the files a repair may change.".into(), Some(&task.id), None);
        }
    }
    inspect_into(
        &mut report,
        root,
        configured_checks,
        &all_writes,
        None,
        &search,
    );
    report.finish();
    report
}

fn future_path(relative: &str, writes: &[String], directory: bool) -> bool {
    let normalized = Path::new(relative)
        .components()
        .filter_map(|c| {
            if let std::path::Component::Normal(p) = c {
                Some(p.to_string_lossy())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    let relative = normalized.as_str();
    plan::allowed(relative, writes)
        || (directory
            && writes.iter().any(|w| {
                let prefix = w
                    .split(['*', '?', '[', '{'])
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('/');
                prefix == relative
                    || prefix.starts_with(&format!("{}/", relative.trim_end_matches('/')))
            }))
}
fn inspect_into(
    report: &mut Report,
    root: &Path,
    checks: &[Check],
    writes: &[String],
    task: Option<&str>,
    search: &SearchEnvironment,
) {
    for (index, check) in checks.iter().enumerate() {
        let before = report.diagnostics.len();
        let mut deferred = false;
        if let Err(error) = config::validate_check(check) {
            report.note(
                "invalid_check",
                "error",
                error.to_string(),
                task,
                Some(index),
            );
        }
        let cwd = match paths::inside(root, &check.cwd) {
            Ok(path) => path,
            Err(error) => {
                report.note(
                    "check_cwd_unsafe",
                    "error",
                    error.to_string(),
                    task,
                    Some(index),
                );
                root.join(&check.cwd)
            }
        };
        if !cwd.is_dir() {
            let future = !cwd.exists() && future_path(&check.cwd, writes, true);
            deferred |= future;
            report.note(
                if future {
                    "check_cwd_deferred"
                } else {
                    "check_cwd_missing"
                },
                if future { "info" } else { "error" },
                format!(
                    "Verification cwd {:?} {}.",
                    check.cwd,
                    if future {
                        "is expected from planned writes; check again before execution"
                    } else {
                        "is not an existing directory or a declared future output"
                    }
                ),
                task,
                Some(index),
            );
        }
        let executable = check
            .argv
            .first()
            .and_then(|name| resolve_executable(&cwd, name, search));
        if executable.is_none()
            && let Some(name) = check.argv.first()
        {
            let relative = Path::new(&check.cwd)
                .join(name)
                .to_string_lossy()
                .into_owned();
            let future = name.contains('/')
                && paths::relative(&relative).is_ok()
                && future_path(&relative, writes, false);
            deferred |= future;
            report.note(if future { "check_executable_deferred" } else { "check_executable_missing" }, if future { "info" } else { "error" }, format!("Executable {name:?} {}.", if future { "is a declared future output; availability and permissions must be rechecked before execution" } else { "cannot be resolved as an executable from cwd/PATH" }), task, Some(index));
        }
        if is_node(check.argv.first().map(String::as_str).unwrap_or("")) {
            for operand in node_test_operands(&check.argv) {
                if operand.contains(['*', '?', '[', '{']) {
                    continue;
                }
                let target = cwd.join(operand);
                // This is an explicit portability policy, not a claim about every Node release.
                if target.is_dir() || operand.ends_with('/') || operand.ends_with('\\') {
                    report.note("node_test_directory_operand", "warning", format!("Node test operand {operand:?} names a directory. Directory operands vary across Node versions; confirm compatibility or choose explicit test files, a supported glob, or node --test discovery. For an active run use run.revise to correct a pending verification command."), task, Some(index));
                } else if !target.exists() {
                    let relative = Path::new(&check.cwd)
                        .join(operand)
                        .to_string_lossy()
                        .into_owned();
                    let future =
                        paths::relative(&relative).is_ok() && future_path(&relative, writes, false);
                    deferred |= future;
                    report.note(
                        if future {
                            "check_target_deferred"
                        } else {
                            "check_target_missing"
                        },
                        if future { "info" } else { "error" },
                        format!(
                            "Node test target {operand:?} {}.",
                            if future {
                                "is covered by planned writes; execution remains pending"
                            } else {
                                "does not exist and is not covered by task/prerequisite writes"
                            }
                        ),
                        task,
                        Some(index),
                    );
                }
            }
        }
        let blocked = report.diagnostics[before..]
            .iter()
            .any(|d| d.severity == "error");
        report.checks.push(CheckReadiness {
            task_id: task.map(str::to_owned),
            check_index: index,
            argv: check.argv.clone(),
            cwd: check.cwd.clone(),
            executable,
            status: if blocked {
                "needs_input"
            } else if deferred {
                "deferred"
            } else {
                "ready"
            }
            .into(),
        });
    }
}
fn is_node(name: &str) -> bool {
    Path::new(name)
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|n| n == "node" || n.eq_ignore_ascii_case("node.exe"))
}
// Deliberately conservative: unknown options may consume an operand. Do not
// mistake their argument (or inline JavaScript) for a test path.
fn node_test_operands(argv: &[String]) -> Vec<&str> {
    if !argv.iter().any(|a| a == "--test") {
        return vec![];
    }
    let mut operands = vec![];
    let mut args = argv.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            operands.extend(args.map(String::as_str));
            break;
        }
        if ["-e", "--eval", "-p", "--print"].contains(&arg.as_str())
            || arg.starts_with("--eval=")
            || arg.starts_with("--print=")
        {
            return vec![];
        }
        if [
            "--test-name-pattern",
            "--test-skip-pattern",
            "--test-reporter",
            "--test-reporter-destination",
            "--test-concurrency",
            "--test-timeout",
            "--import",
            "--require",
            "-r",
        ]
        .contains(&arg.as_str())
        {
            args.next();
            continue;
        }
        if arg == "--test"
            || arg == "--no-warnings"
            || arg == "--test-only"
            || arg.starts_with("--") && arg.contains('=')
        {
            continue;
        }
        if arg.starts_with('-') {
            return vec![];
        }
        operands.push(arg.as_str());
    }
    operands
}
struct SearchEnvironment {
    path: OsString,
    #[cfg(windows)]
    extensions: OsString,
}
impl SearchEnvironment {
    fn new(overrides: &BTreeMap<String, String>) -> Self {
        Self {
            path: effective_environment("PATH", overrides).unwrap_or_default(),
            #[cfg(windows)]
            extensions: effective_environment("PATHEXT", overrides)
                .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into()),
        }
    }
}
fn effective_environment(name: &str, overrides: &BTreeMap<String, String>) -> Option<OsString> {
    // std::process::Command environment names are case insensitive on Windows.
    // Preserve that behavior, including the last overriding key in map order.
    overrides
        .iter()
        .rev()
        .find(|(key, _)| {
            if cfg!(windows) {
                key.eq_ignore_ascii_case(name)
            } else {
                key.as_str() == name
            }
        })
        .map(|(_, value)| OsString::from(value))
        .or_else(|| env::var_os(name))
}
fn resolve_executable(cwd: &Path, command: &str, search: &SearchEnvironment) -> Option<PathBuf> {
    if command.is_empty() {
        return None;
    }
    let supplied = Path::new(command);
    let candidates: Vec<_> =
        if supplied.is_absolute() || command.contains('/') || command.contains('\\') {
            vec![cwd.join(supplied)]
        } else {
            env::split_paths(&search.path)
                .map(|p| cwd.join(p).join(command))
                .collect()
        };
    candidates.into_iter().find_map(|candidate| {
        if executable_file(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        if candidate.extension().is_none() {
            for ext in search.extensions.to_string_lossy().split(';') {
                let candidate = candidate.with_extension(ext.trim_start_matches('.'));
                if executable_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
        None
    })
}
fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
