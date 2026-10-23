//! Per-wave artifact identity and durable, bounded command diagnostics.
use crate::{
    config::Config,
    git,
    model::{Check, Evidence},
    paths, process,
};
use anyhow::{Context, Result};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

pub struct Wave {
    pub directory: PathBuf,
    pub revision: String,
    environment: BTreeMap<String, String>,
    checkout: PathBuf,
    pub stage: String,
}
pub struct CheckResult {
    pub evidence: Evidence,
    pub failure: Option<String>,
}
impl Wave {
    pub fn new(directory: PathBuf, checkout: &Path, stage: &str, config: &Config) -> Result<Self> {
        fs::create_dir_all(
            directory
                .parent()
                .context("verification_output: missing wave parent")?,
        )?;
        fs::create_dir(&directory).context("verification_output: wave directory must be new")?;
        fs::create_dir(directory.join("artifacts"))?;
        let revision = git::head(checkout)?;
        let mut environment = config.runner.environment.clone();
        for (key, value) in [
            (
                "SA_VERIFY_OUTPUT_DIR",
                directory.to_string_lossy().into_owned(),
            ),
            (
                "SA_VERIFY_WAVE_ID",
                directory
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            ),
            ("SA_VERIFY_STAGE", stage.to_owned()),
            ("SA_VERIFY_REVISION", revision.clone()),
            (
                "SA_VERIFY_CHECKOUT",
                checkout.to_string_lossy().into_owned(),
            ),
        ] {
            environment.insert(key.into(), value);
        }
        paths::atomic_write(
            &directory.join("wave.json"),
            serde_json::to_vec_pretty(
                &json!({"schema_version":1,"stage":stage,"revision":revision,"checkout":checkout,"output_directory":directory.join("artifacts"),"diagnostics_directory":directory}),
            )?,
        )?;
        Ok(Self {
            directory,
            revision,
            environment,
            checkout: checkout.into(),
            stage: stage.into(),
        })
    }

    pub fn execute(
        &self,
        check: &Check,
        preparation: bool,
        config: &Config,
        cancel: Arc<AtomicBool>,
    ) -> Result<CheckResult> {
        crate::config::validate_check(check)?;
        let directory =
            self.directory
                .join(paths::id(if preparation { "prepare" } else { "check" }));
        fs::create_dir(&directory)?;
        let cwd = paths::inside(&self.checkout, &check.cwd)?;
        // Consumer artifacts must not share a directory with live process logs.
        let artifacts = self.directory.join("artifacts");
        let mut environment = self.environment.clone();
        environment.insert(
            "SA_VERIFY_OUTPUT_DIR".into(),
            artifacts.to_string_lossy().into_owned(),
        );
        let start = Instant::now();
        let result = process::execute(process::Request {
            argv: &check.argv,
            cwd: &cwd,
            env: &environment,
            stdin: None,
            directory: &directory,
            timeout: Duration::from_secs(config.execution.attempt_timeout_seconds),
            max_log_bytes: config.execution.max_log_bytes,
            cancel,
        });
        let duration_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let (exit_code, mut failure) = match result {
            Ok(output) => (
                output.code,
                if output.cancelled {
                    Some("cancelled")
                } else if output.timed_out {
                    Some("timeout")
                } else if output.code != 0 {
                    Some("check_failed")
                } else {
                    None
                },
            ),
            Err(ref error) => (
                -1,
                Some(if error.to_string().starts_with("log_limit_exceeded") {
                    "log_limit"
                } else {
                    "execution_error"
                }),
            ),
        };
        // Full output stays in the runtime; public errors never embed secrets from it.
        let log = directory.join("stdout.log");
        let stderr = directory.join("stderr.log");
        let stdout_hash = fs::read(&log).ok().map(paths::hash);
        let stderr_hash = fs::read(&stderr).ok().map(paths::hash);
        let tree_unchanged = git::head(&self.checkout).is_ok_and(|h| h == self.revision)
            && git::clean(&self.checkout).unwrap_or(false);
        if !tree_unchanged {
            failure = Some("source_changed");
        }
        let manifest = directory.join("result.json");
        let details = json!({"schema_version":1,"wave_id":self.directory.file_name().unwrap().to_string_lossy(),"stage":self.stage,"step":if preparation {"preparation"} else {"check"},"duration_ms":duration_ms,"failure_kind":failure,"exit_code":exit_code,"argv":check.argv,"cwd":cwd,"revision":self.revision,"tree_unchanged":tree_unchanged,"stdout":log,"stdout_sha256":stdout_hash,"stderr":stderr,"stderr_sha256":stderr_hash,"manifest":manifest,"output_directory":artifacts,"diagnostics_directory":directory});
        paths::atomic_write(&manifest, serde_json::to_vec_pretty(&details)?).with_context(
            || {
                format!(
                    "verification_evidence_storage: cannot persist {}",
                    manifest.display()
                )
            },
        )?;
        Ok(CheckResult {
            evidence: Evidence {
                argv: check.argv.clone(),
                cwd: cwd.to_string_lossy().into_owned(),
                revision: self.revision.clone(),
                exit_code,
                log: log.to_string_lossy().into_owned(),
                log_hash: stdout_hash.unwrap_or_default(),
                finished_at: paths::now(),
                tree_unchanged,
                details: Some(details),
            },
            failure: failure.map(str::to_owned),
        })
    }
}

/// Identity is local to one checkout and stage. No cross-revision/cross-checkout PASS.
/// External service checks must opt out using execution.reuse_verification=false.
pub fn input_identity(
    project: &Path,
    checks: &[Check],
    label: &str,
    config: &Config,
) -> Result<String> {
    let mut commands = checks.to_vec();
    if let Some(hook) = config.hooks.get("verification_prepare") {
        commands.push(Check {
            argv: hook.argv.clone(),
            cwd: ".".into(),
        });
    }
    let report = crate::verification_preflight::inspect_checks_with_environment(
        project,
        &commands,
        &[],
        &config.runner.environment,
    );
    let executables: Vec<_> = report
        .checks
        .iter()
        .map(|c| -> Result<_> {
            let path = c
                .executable
                .as_ref()
                .context("verification executable unavailable")?;
            Ok((path, paths::hash(fs::read(path)?)))
        })
        .collect::<Result<_>>()?;
    // Persist only the digest; never serialize inherited environment secrets.
    let environment: BTreeMap<_, _> = std::env::vars_os()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        })
        .collect();
    Ok(paths::hash(serde_json::to_vec(&(
        project.canonicalize()?,
        paths::hash(fs::read(std::env::current_exe()?)?),
        git::head(project)?,
        checks,
        label,
        config,
        environment,
        executables,
    ))?))
}

pub fn evidence_files(directory: &Path) -> Result<BTreeMap<String, String>> {
    fn visit(root: &Path, dir: &Path, files: &mut BTreeMap<String, String>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_symlink() {
                anyhow::bail!("verification evidence symlink");
            }
            if ty.is_dir() {
                visit(root, &entry.path(), files)?;
            } else if ty.is_file() {
                files.insert(
                    entry
                        .path()
                        .strip_prefix(root)?
                        .to_string_lossy()
                        .into_owned(),
                    paths::hash(fs::read(entry.path())?),
                );
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    visit(directory, directory, &mut files)?;
    Ok(files)
}

pub fn reusable(receipt: &Path, identity: &str) -> bool {
    let validate = || -> Result<bool> {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(receipt)?)?;
        if value["input_identity"] != identity || value["status"] != "passed" {
            return Ok(false);
        }
        let dir = Path::new(
            value["directory"]
                .as_str()
                .context("receipt directory missing")?,
        );
        let files: BTreeMap<String, String> = serde_json::from_value(value["files"].clone())?;
        Ok(!files.is_empty() && evidence_files(dir)? == files)
    };
    validate().unwrap_or(false)
}

pub fn complete_receipt(receipt: &Path, identity: &str, directory: &Path) -> Result<()> {
    let files = evidence_files(directory)?;
    paths::atomic_write(
        receipt,
        serde_json::to_vec_pretty(
            &json!({"schema_version":1,"status":"passed","input_identity":identity,"directory":directory,"files":files}),
        )?,
    )
}
