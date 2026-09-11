use crate::paths;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn command(root: &Path, args: &[&str], input: Option<&[u8]>) -> Result<String> {
    let mut child = Command::new("git")
        .args([
            "-c",
            "core.quotePath=false",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
        ])
        .args(args)
        .current_dir(root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("git_unavailable: cannot start git")?;
    if let Some(bytes) = input {
        child.stdin.take().unwrap().write_all(bytes)?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "git_failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("git output is not UTF-8")
}
#[derive(Debug, Clone)]
pub struct Repository {
    pub root: PathBuf,
    pub common: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    pub path: String,
    pub head: String,
    pub branch: String,
    pub locked: bool,
    pub prunable: bool,
}
impl Repository {
    pub fn discover(path: &Path) -> Result<Self> {
        let root = PathBuf::from(command(path, &["rev-parse", "--show-toplevel"], None)?.trim())
            .canonicalize()?;
        let common = PathBuf::from(
            command(
                path,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
                None,
            )?
            .trim(),
        )
        .canonicalize()?;
        Ok(Self { root, common })
    }
    pub fn runtime(&self) -> Result<PathBuf> {
        paths::inside(&self.common, "spec-autonomous")
    }
    pub fn head(&self) -> Result<String> {
        head(&self.root)
    }
    pub fn branch(&self) -> Result<String> {
        let b = command(
            &self.root,
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
            None,
        )?
        .trim()
        .to_string();
        if b.is_empty() {
            bail!("git_preflight: detached origin is not supported for delivery");
        }
        Ok(b)
    }
    pub fn preflight(&self) -> Result<()> {
        self.require_local_runtime()?;
        self.head()
            .context("git_preflight: an initial commit is required")?;
        self.branch()?;
        if !clean(&self.root)? {
            bail!("dirty_checkout: commit or move existing changes before autonomous execution");
        }
        Ok(())
    }
    pub fn require_local_runtime(&self) -> Result<()> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            use std::{ffi::CString, os::unix::ffi::OsStrExt};
            let path = CString::new(self.common.as_os_str().as_bytes())?;
            let mut info: libc::statfs = unsafe { std::mem::zeroed() };
            if unsafe { libc::statfs(path.as_ptr(), &mut info) } != 0 {
                bail!("filesystem_unavailable: cannot inspect runtime filesystem");
            }
            #[cfg(target_os = "macos")]
            let remote = ["nfs", "smbfs", "webdav", "afpfs"].contains(
                &unsafe { std::ffi::CStr::from_ptr(info.f_fstypename.as_ptr()) }
                    .to_string_lossy()
                    .as_ref(),
            );
            #[cfg(target_os = "linux")]
            let remote = { [0x6969u64, 0x517b, 0xff534d42].contains(&(info.f_type as u64)) };
            if remote {
                bail!(
                    "shared_filesystem_unsupported: runtime requires local Git metadata and a single host"
                );
            }
        }
        #[cfg(windows)]
        if self.common.to_string_lossy().starts_with(r"\\?\UNC\")
            || self.common.to_string_lossy().starts_with(r"\\")
                && !self.common.to_string_lossy().starts_with(r"\\?\")
        {
            bail!("shared_filesystem_unsupported: UNC runtime is not supported");
        }
        Ok(())
    }
    pub fn add_worktree(&self, name: &str, base: &str) -> Result<(PathBuf, String)> {
        paths::valid_id(name)?;
        let path = paths::inside(&self.common, &format!("spec-autonomous/worktrees/{name}"))?;
        fs::create_dir_all(path.parent().unwrap())?;
        let branch = format!("codex/sa/{name}");
        command(
            &self.root,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                &branch,
                path.to_str().context("non-UTF8 path")?,
                base,
            ],
            None,
        )?;
        Ok((path, branch))
    }
    pub fn worktrees(&self) -> Result<Vec<Worktree>> {
        parse_inventory(&command(
            &self.root,
            &["worktree", "list", "--porcelain", "-z"],
            None,
        )?)
    }
}
pub fn parse_inventory(text: &str) -> Result<Vec<Worktree>> {
    let mut rows = vec![];
    let mut current: Option<Worktree> = None;
    for field in text.split('\0') {
        if let Some(path) = field.strip_prefix("worktree ") {
            if let Some(row) = current.take() {
                rows.push(row);
            }
            current = Some(Worktree {
                path: path.into(),
                head: String::new(),
                branch: String::new(),
                locked: false,
                prunable: false,
            });
        } else if let Some(row) = current.as_mut() {
            if let Some(v) = field.strip_prefix("HEAD ") {
                row.head = v.into();
            }
            if let Some(v) = field.strip_prefix("branch ") {
                row.branch = v.trim_start_matches("refs/heads/").into();
            }
            if field.starts_with("locked") {
                row.locked = true;
            }
            if field.starts_with("prunable") {
                row.prunable = true;
            }
        }
    }
    if let Some(row) = current {
        rows.push(row);
    }
    Ok(rows)
}
pub fn head(root: &Path) -> Result<String> {
    Ok(command(root, &["rev-parse", "HEAD"], None)?.trim().into())
}
pub fn clean(root: &Path) -> Result<bool> {
    Ok(command(
        root,
        &["status", "--porcelain", "--untracked-files=all"],
        None,
    )?
    .is_empty())
}
pub fn commit(root: &Path, message: &str) -> Result<String> {
    command(root, &["add", "--all"], None)?;
    if !command(root, &["diff", "--cached", "--name-only"], None)?.is_empty()
        || command(
            root,
            &["rev-parse", "--quiet", "--verify", "MERGE_HEAD"],
            None,
        )
        .is_ok()
    {
        command(
            root,
            &[
                "-c",
                "user.name=Spec Autonomous",
                "-c",
                "user.email=spec-autonomous@localhost",
                "commit",
                "--quiet",
                "-m",
                message,
            ],
            None,
        )?;
    }
    head(root)
}
pub fn changed(root: &Path, base: &str) -> Result<Vec<String>> {
    command(root, &["add", "--all"], None)?;
    Ok(command(
        root,
        &["diff", "--cached", "--name-only", "-z", base, "--"],
        None,
    )?
    .split('\0')
    .filter(|x| !x.is_empty())
    .map(str::to_string)
    .collect())
}
pub fn patch(root: &Path, base: &str) -> Result<String> {
    command(root, &["add", "--all"], None)?;
    command(root, &["diff", "--cached", "--binary", base, "--"], None)
}
pub fn apply(root: &Path, patch: &str) -> Result<()> {
    if !patch.trim().is_empty() {
        command(
            root,
            &["apply", "--index", "--binary", "-"],
            Some(patch.as_bytes()),
        )?;
    }
    Ok(())
}
pub fn ancestor(root: &Path, older: &str, newer: &str) -> bool {
    command(root, &["merge-base", "--is-ancestor", older, newer], None).is_ok()
}
pub fn advance(root: &Path, target: &str) -> Result<()> {
    if !clean(root)? {
        bail!("dirty_checkout: cannot advance a changed checkout");
    }
    command(root, &["merge", "--ff-only", "--no-edit", target], None)?;
    Ok(())
}
