use crate::{
    model::{Check, SCHEMA},
    paths,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub execution: Execution,
    #[serde(skip_serializing_if = "Runner::is_disabled")]
    pub runner: Runner,
    pub host: HostConfig,
    pub environment: BTreeMap<String, String>,
    pub provider: Provider,
    /// Checks that run at every stage: worker, combined, phase gates and the milestone gate.
    pub verification: Vec<Check>,
    /// Checks that run only when a task candidate is verified (worker and combined).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub task_verification: Vec<Check>,
    /// Checks that run only at phase gates (phase, post-hook, stale re-verification).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub phase_verification: Vec<Check>,
    /// Checks that run only at the milestone gate.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub milestone_verification: Vec<Check>,
    pub hooks: BTreeMap<String, Hook>,
    pub policy: Policy,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Execution {
    /// Reuse complete same-input verification while waiting for semantic work.
    pub reuse_verification: bool,
    /// Reclaim closed, unreferenced, clean checkouts at host handoffs.
    pub automatic_cleanup: bool,
    pub mode: String,
    pub max_workers: usize,
    pub max_attempts: u32,
    pub attempt_timeout_seconds: u64,
    pub run_timeout_seconds: u64,
    pub max_repair_rounds: u32,
    pub no_progress_limit: u32,
    pub delivery: String,
    pub max_log_bytes: u64,
    pub max_context_bytes: usize,
    pub max_source_bytes: usize,
    pub max_planner_tasks: usize,
    /// `same-tree` reuses passing evidence for an identical tree, command, environment and
    /// checkout class; `none` executes every check in every wave.
    pub verification_reuse: String,
    /// Lift the retry, repair-round and run-deadline caps. Off by default because runs
    /// with those budgets looped for days before anyone looked.
    pub unsafe_budgets: bool,
    /// `terminal` removes a writing attempt's worktree once it is integrated, superseded,
    /// revoked or an older failure, after its patch is stored; `never` keeps every
    /// worktree for manual run.cleanup. `automatic_cleanup = false` also disables it.
    pub reclaim_worktrees: String,
    /// Newest failed worktrees kept per task for inspection while the task is open.
    pub retain_failed_worktrees: u32,
    /// Run `git submodule update --init --recursive` in every checkout the coordinator
    /// creates for writing or verification. Off by default; omitted from serialized
    /// configuration while off so existing identities do not change.
    #[serde(skip_serializing_if = "is_false")]
    pub init_submodules: bool,
    /// Where worktrees handed to host workers live: `git` (default, under the Git common
    /// directory), `sibling` (`<repo>.sa-worktrees/` next to the main checkout) or an
    /// absolute directory. Omitted while `git` so existing identities do not change.
    #[serde(skip_serializing_if = "is_git_worktree_root")]
    pub worktree_root: String,
    /// `affected` lets a task check declared by another task stand on its last pass on
    /// an ancestor commit when the change since does not reach that task's declared
    /// writes and reads; `full` runs every check of every gate. Configured project,
    /// stage and phase checks and the task's own checks always run.
    #[serde(skip_serializing_if = "is_affected")]
    pub verification_scope: String,
    /// The same choice for the milestone gate. `full` (default) runs every task check
    /// once more on the delivered tree unless identical-tree evidence exists.
    #[serde(skip_serializing_if = "is_full")]
    pub milestone_verification_scope: String,
}
fn is_affected(value: &str) -> bool {
    value == "affected"
}
fn is_full(value: &str) -> bool {
    value == "full"
}
fn is_false(value: &bool) -> bool {
    !*value
}
fn is_git_worktree_root(value: &String) -> bool {
    value == crate::git::WORKTREE_ROOT_GIT
}
/// Hook run in a writing worker's checkout after it is created and before its packet
/// is issued, for example to sync a virtual environment from a shared cache.
pub const WORKER_PREPARE: &str = "worker_prepare";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Runner {
    pub profile: String,
    pub command: Vec<String>,
    pub fresh_session: bool,
    pub sandbox: String,
    pub environment: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Provider {
    pub openspec_command: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hook {
    pub argv: Vec<String>,
    #[serde(default)]
    pub idempotent: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub archive: bool,
    pub push: bool,
    pub publish: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 2,
            execution: Execution::default(),
            runner: Runner::default(),
            host: HostConfig::default(),
            environment: BTreeMap::new(),
            provider: Provider::default(),
            verification: vec![],
            task_verification: vec![],
            phase_verification: vec![],
            milestone_verification: vec![],
            hooks: BTreeMap::new(),
            policy: Policy::default(),
        }
    }
}
impl Default for Execution {
    fn default() -> Self {
        Self {
            reuse_verification: true,
            automatic_cleanup: true,
            mode: "autonomous".into(),
            max_workers: 3,
            max_attempts: 3,
            attempt_timeout_seconds: 1800,
            run_timeout_seconds: 28800,
            max_repair_rounds: 2,
            no_progress_limit: 2,
            delivery: "ff-original".into(),
            max_log_bytes: 8 * 1024 * 1024,
            max_context_bytes: 128 * 1024,
            max_source_bytes: 2 * 1024 * 1024,
            max_planner_tasks: 32,
            verification_reuse: "same-tree".into(),
            unsafe_budgets: false,
            reclaim_worktrees: "terminal".into(),
            retain_failed_worktrees: 1,
            init_submodules: false,
            verification_scope: "affected".into(),
            milestone_verification_scope: "full".into(),
            worktree_root: crate::git::WORKTREE_ROOT_GIT.into(),
        }
    }
}
impl Default for Runner {
    fn default() -> Self {
        Self {
            profile: "disabled".into(),
            command: vec![],
            fresh_session: false,
            sandbox: "runner-managed".into(),
            environment: BTreeMap::new(),
        }
    }
}
impl Config {
    pub fn load(root: &Path) -> Result<Self> {
        let user = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|p| std::path::PathBuf::from(p).join(".config"))
            })
            .map(|p| p.join("spec-autonomous/config.toml"));
        Self::load_with_user(root, user.as_deref())
    }
    pub fn load_with_user(root: &Path, user: Option<&Path>) -> Result<Self> {
        let path = paths::inside(root, ".spec-autonomous/config.toml")?;
        let mut merged = toml::Value::try_from(Self::default())?;
        for file in user
            .into_iter()
            .chain(std::iter::once(path.as_path()))
            .filter(|p| p.exists())
        {
            let value: toml::Value =
                toml::from_str(&std::fs::read_to_string(file)?).map_err(|_| {
                    anyhow::anyhow!("invalid_config: TOML parse failed at {}", file.display())
                })?;
            merge(&mut merged, value);
        }
        let value: Self = merged.try_into().map_err(|_| {
            anyhow::anyhow!("invalid_config: unsupported fields or invalid configuration types")
        })?;
        value.validate()?;
        Ok(value)
    }
    /// Every configured project check regardless of stage.
    pub fn all_verification(&self) -> impl Iterator<Item = &Check> {
        self.verification
            .iter()
            .chain(&self.task_verification)
            .chain(&self.phase_verification)
            .chain(&self.milestone_verification)
    }
    /// Budget fields that exceed twice their default, for doctor warnings.
    pub fn budget_warnings(&self) -> Vec<String> {
        let d = Execution::default();
        let x = &self.execution;
        let mut out = vec![];
        for (name, value, default) in [
            ("max_attempts", x.max_attempts as u64, d.max_attempts as u64),
            (
                "max_repair_rounds",
                x.max_repair_rounds as u64,
                d.max_repair_rounds as u64,
            ),
            (
                "run_timeout_seconds",
                x.run_timeout_seconds,
                d.run_timeout_seconds,
            ),
            (
                "no_progress_limit",
                x.no_progress_limit as u64,
                d.no_progress_limit as u64,
            ),
        ] {
            if value > default.saturating_mul(2) {
                out.push(format!(
                    "budget_warning: {name}={value} is more than twice the default {default}; long loops stay unnoticed with large budgets"
                ));
            }
        }
        if x.unsafe_budgets {
            out.push(
                "budget_warning: unsafe_budgets=true lifts the retry, repair and deadline caps"
                    .into(),
            );
        }
        out
    }
    /// Checkout preparation that looks forgotten: a `verification-prepare.*` or
    /// `worker-prepare.*` script under `.spec-autonomous/` with no hook bound to it, or
    /// Git submodules that no configured step initializes in fresh worktrees.
    pub fn preparation_warnings(&self, project: &Path, repository: Option<&Path>) -> Vec<String> {
        fn scripts(dir: &Path, depth: usize, found: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() && depth > 0 {
                    scripts(&entry.path(), depth - 1, found);
                } else if kind.is_file() {
                    found.push(entry.path());
                }
            }
        }
        let mut out = vec![];
        let base = project.join(".spec-autonomous");
        let mut files = vec![];
        scripts(&base, 1, &mut files);
        files.sort();
        for (hook, stem) in [
            ("verification_prepare", "verification-prepare"),
            (WORKER_PREPARE, "worker-prepare"),
        ] {
            if self.hooks.contains_key(hook) {
                continue;
            }
            let unbound: Vec<String> = files
                .iter()
                .filter(|path| {
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .and_then(|n| n.split_once('.'))
                        .is_some_and(|(name, _)| name == stem || name == hook)
                })
                .filter_map(|path| path.strip_prefix(project).ok())
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .collect();
            if !unbound.is_empty() {
                out.push(format!(
                    "hook_unbound_warning: {} exists but [hooks.{hook}] is not configured, so it never runs; bind it with argv and idempotent = true",
                    unbound.join(", ")
                ));
            }
        }
        let submodules = repository
            .map(|root| root.join(".gitmodules"))
            .and_then(|path| std::fs::read_to_string(path).ok())
            .is_some_and(|text| text.contains("[submodule"));
        if submodules
            && !self.execution.init_submodules
            && !self.hooks.contains_key("verification_prepare")
        {
            out.push(
                "submodules_warning: the repository declares Git submodules but [execution] init_submodules = false and no [hooks.verification_prepare] is configured; worker and verification worktrees start without submodule contents".into(),
            );
        }
        out
    }
    pub fn validate(&self) -> Result<()> {
        if ![SCHEMA, 2].contains(&self.schema_version) {
            bail!("schema_unsupported: config version");
        }
        if self.host.max_concurrency == 0
            || self.host.max_concurrency > 32
            || self.host.lease_seconds == 0
            || self.host.lease_seconds > 2_592_000
        {
            bail!("invalid_config: invalid host limits");
        }
        if !["same-tree", "none"].contains(&self.execution.verification_reuse.as_str()) {
            bail!("invalid_config: verification_reuse must be same-tree or none");
        }
        for (name, value) in [
            ("verification_scope", &self.execution.verification_scope),
            (
                "milestone_verification_scope",
                &self.execution.milestone_verification_scope,
            ),
        ] {
            if !["affected", "full"].contains(&value.as_str()) {
                bail!("invalid_config: {name} must be affected or full");
            }
        }
        let root = &self.execution.worktree_root;
        if ![
            crate::git::WORKTREE_ROOT_GIT,
            crate::git::WORKTREE_ROOT_SIBLING,
        ]
        .contains(&root.as_str())
            && (!Path::new(root).is_absolute() || root.contains('\0'))
        {
            bail!("invalid_config: worktree_root must be git, sibling or an absolute directory");
        }
        if !["terminal", "never"].contains(&self.execution.reclaim_worktrees.as_str()) {
            bail!("invalid_config: reclaim_worktrees must be terminal or never");
        }
        let x = &self.execution;
        if x.max_workers == 0
            || x.max_workers > 32
            || x.max_attempts == 0
            || x.no_progress_limit == 0
            || x.attempt_timeout_seconds == 0
            || x.run_timeout_seconds == 0
            || x.run_timeout_seconds > u64::MAX / 1000
            || x.attempt_timeout_seconds > u64::MAX / 1000
            || x.max_log_bytes < 1024
            || x.max_context_bytes < 1024
            || x.max_source_bytes < 1024
            || x.max_planner_tasks == 0
            || x.max_planner_tasks > 128
        {
            bail!("invalid_config: execution limits must be positive and workers <=32");
        }
        if !x.unsafe_budgets
            && (x.max_attempts > MAX_ATTEMPTS_CAP
                || x.max_repair_rounds > MAX_REPAIR_ROUNDS_CAP
                || x.run_timeout_seconds > RUN_TIMEOUT_CAP_SECONDS)
        {
            bail!(
                "invalid_config: budget_cap: max_attempts <= {MAX_ATTEMPTS_CAP}, max_repair_rounds <= {MAX_REPAIR_ROUNDS_CAP} and run_timeout_seconds <= {RUN_TIMEOUT_CAP_SECONDS} unless [execution] unsafe_budgets = true"
            );
        }
        if !["ff-original", "branch"].contains(&x.delivery.as_str()) {
            bail!("invalid_config: delivery must be ff-original or branch");
        }
        if !["native", "autonomous"].contains(&x.mode.as_str()) {
            bail!("invalid_config: mode must be native or autonomous");
        }
        if self.policy.archive || self.policy.push || self.policy.publish {
            bail!(
                "policy_capability_unavailable: use explicit native archive or the separate release workflow; unattended external lifecycle actions are not enabled in this local profile"
            );
        }
        if !["disabled", "command", "codex"].contains(&self.runner.profile.as_str()) {
            bail!("runner_unsupported: unknown profile");
        }
        for c in self.all_verification() {
            validate_check(c)?;
        }
        if self
            .hooks
            .get("verification_prepare")
            .is_some_and(|h| !h.idempotent)
        {
            bail!("invalid_config: verification_prepare must be idempotent for each wave identity");
        }
        if self
            .hooks
            .get(WORKER_PREPARE)
            .is_some_and(|h| !h.idempotent)
        {
            bail!(
                "invalid_config: worker_prepare must be idempotent; it may run again for a re-issued attempt"
            );
        }
        for hook in self.hooks.values() {
            validate_check(&Check {
                argv: hook.argv.clone(),
                cwd: ".".into(),
                env: Default::default(),
            })?;
        }
        Ok(())
    }
}
fn merge(base: &mut toml::Value, override_: toml::Value) {
    match (base, override_) {
        (toml::Value::Table(base), toml::Value::Table(values)) => {
            for (key, value) in values {
                if let Some(old) = base.get_mut(&key) {
                    merge(old, value);
                } else {
                    base.insert(key, value);
                }
            }
        }
        (base, value) => *base = value,
    }
}
pub const MAX_ATTEMPTS_CAP: u32 = 10;
pub const MAX_REPAIR_ROUNDS_CAP: u32 = 4;
pub const RUN_TIMEOUT_CAP_SECONDS: u64 = 72 * 3600;
/// Longest single argv element a check may carry; scripts belong in files.
pub const MAX_ARGV_ELEMENT_BYTES: usize = 1024;

pub fn validate_check(check: &Check) -> Result<()> {
    if check.argv.is_empty()
        || check.argv[0].is_empty()
        || check.argv.iter().any(|x| x.contains('\0'))
    {
        bail!("invalid_command: argv must have an executable");
    }
    // Planners once smuggled whole test suites into argv as compressed, encoded
    // payloads to get past preflight. Scripts must be files declared in the task's writes.
    for (index, arg) in check.argv.iter().enumerate() {
        if arg.len() > MAX_ARGV_ELEMENT_BYTES {
            bail!(
                "invalid_command: inline_script_argv: argv[{index}] is {} bytes; write the script to a file listed in the task's writes and run that file",
                arg.len()
            );
        }
        let lower = arg.to_ascii_lowercase();
        if [
            "b85decode",
            "b64decode",
            "a85decode",
            "base64.b",
            "zlib.decompress",
            "exec(zlib",
            "exec(compile(",
            "eval(atob",
            "from('base64')",
            "from(\"base64\")",
        ]
        .iter()
        .any(|pattern| lower.contains(pattern))
        {
            bail!(
                "invalid_command: inline_script_argv: argv[{index}] embeds an encoded or compressed script; write the script to a file listed in the task's writes and run that file"
            );
        }
    }
    paths::relative(&check.cwd)?;
    validate_check_env(&check.env)
}

/// Most variables one check may set.
pub const MAX_CHECK_ENV: usize = 32;
/// Longest value of a check variable.
pub const MAX_CHECK_ENV_VALUE_BYTES: usize = 1024;
/// Variables that locate executables, the user, the shell or the repository, and the
/// coordinator's own `SA_*` variables. A check cannot replace them.
const RESERVED_CHECK_ENV: &[&str] = &[
    "PATH",
    "PATHEXT",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "PWD",
    "OLDPWD",
    "TMPDIR",
    "TMP",
    "TEMP",
    "IFS",
    "ENV",
    "BASH_ENV",
    "SYSTEMROOT",
    "COMSPEC",
    "USERPROFILE",
];
const RESERVED_CHECK_ENV_PREFIXES: &[&str] = &["SA_", "GIT_", "LD_", "DYLD_"];
/// Name fragments that suggest a credential. Check variables live in plans, packets and
/// the ledger, so secrets belong in `[environment]` instead.
const SECRET_CHECK_ENV_MARKERS: &[&str] = &[
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "CREDENTIAL",
    "API_KEY",
    "APIKEY",
    "PRIVATE_KEY",
    "ACCESS_KEY",
    "AUTH",
    "COOKIE",
];

/// Check-level `env`: plain, non-secret variables such as `PYTHONDONTWRITEBYTECODE=1`.
/// Names are `[A-Za-z_][A-Za-z0-9_]*`; reserved names, `SA_`/`GIT_`/`LD_`/`DYLD_` and
/// credential-like names are refused, values are bounded and never contain NUL.
pub fn validate_check_env(env: &BTreeMap<String, String>) -> Result<()> {
    if env.len() > MAX_CHECK_ENV {
        bail!("invalid_command: check_env: at most {MAX_CHECK_ENV} variables per check");
    }
    for (name, value) in env {
        let mut chars = name.chars();
        let valid = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
            && name.len() <= 128;
        if !valid {
            bail!("invalid_command: check_env: {name:?} is not a valid variable name");
        }
        let upper = name.to_ascii_uppercase();
        if RESERVED_CHECK_ENV.contains(&upper.as_str())
            || RESERVED_CHECK_ENV_PREFIXES
                .iter()
                .any(|prefix| upper.starts_with(prefix))
        {
            bail!(
                "invalid_command: check_env: {name} is reserved; checks cannot replace PATH, HOME, the shell, Git or SA_* variables"
            );
        }
        if SECRET_CHECK_ENV_MARKERS
            .iter()
            .any(|marker| upper.contains(marker))
        {
            bail!(
                "invalid_command: check_env: {name} looks like a credential; check env is stored in plans and the ledger, so put secrets in [environment] instead"
            );
        }
        if value.len() > MAX_CHECK_ENV_VALUE_BYTES || value.contains('\0') {
            bail!(
                "invalid_command: check_env: {name} must be at most {MAX_CHECK_ENV_VALUE_BYTES} bytes without NUL"
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HostConfig {
    pub max_concurrency: usize,
    pub lease_seconds: u64,
    /// Declared host facilities (for example background_dispatch, nested, isolation),
    /// echoed to the host as host_capacity so packets never ask it to self-assess.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub capabilities: BTreeMap<String, String>,
}
impl Default for HostConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 1,
            lease_seconds: 1800,
            capabilities: BTreeMap::new(),
        }
    }
}
impl Runner {
    fn is_disabled(&self) -> bool {
        self.profile == "disabled" && self.command.is_empty() && self.environment.is_empty()
    }
}
