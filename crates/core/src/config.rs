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
    pub verification: Vec<Check>,
    pub hooks: BTreeMap<String, Hook>,
    pub policy: Policy,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Execution {
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
}
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
            hooks: BTreeMap::new(),
            policy: Policy::default(),
        }
    }
}
impl Default for Execution {
    fn default() -> Self {
        Self {
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
        for c in &self.verification {
            validate_check(c)?;
        }
        for hook in self.hooks.values() {
            validate_check(&Check {
                argv: hook.argv.clone(),
                cwd: ".".into(),
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
pub fn validate_check(check: &Check) -> Result<()> {
    if check.argv.is_empty()
        || check.argv[0].is_empty()
        || check.argv.iter().any(|x| x.contains('\0'))
    {
        bail!("invalid_command: argv must have an executable");
    }
    paths::relative(&check.cwd)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HostConfig {
    pub max_concurrency: usize,
    pub lease_seconds: u64,
}
impl Default for HostConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 1,
            lease_seconds: 1800,
        }
    }
}
impl Runner {
    fn is_disabled(&self) -> bool {
        self.profile == "disabled" && self.command.is_empty() && self.environment.is_empty()
    }
}
