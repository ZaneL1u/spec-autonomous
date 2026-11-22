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
    let mut safe_args = args.to_vec();
    if safe_args.first() == Some(&"diff") {
        safe_args.insert(1, "--no-ext-diff");
        safe_args.insert(2, "--no-textconv");
    }
    let mut child = Command::new("git")
        .args([
            "-c",
            "core.quotePath=false",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
        ])
        .args(&safe_args)
        .current_dir(root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
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
            return Err(dirty_error(
                &self.root,
                "commit or move existing changes before autonomous execution",
            ));
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
    /// The main working tree: the parent of a non-bare common directory.
    fn main_checkout(&self) -> Option<&Path> {
        (self.common.file_name()? == ".git")
            .then(|| self.common.parent())
            .flatten()
    }
    /// Directory for coordinator worktrees under an `[execution] worktree_root` setting,
    /// without creating it. `git` is the runtime directory inside the Git common dir;
    /// `sibling` is `<main checkout>.sa-worktrees` next to the main checkout; an absolute
    /// directory gets a per-repository subdirectory so repositories never share one.
    pub fn worktree_root(&self, setting: &str) -> Result<PathBuf> {
        if setting == WORKTREE_ROOT_GIT {
            return paths::inside(&self.common, "spec-autonomous/worktrees");
        }
        let main = self.main_checkout().context(
            "worktree_root_unavailable: a bare or unusual repository layout supports only worktree_root = \"git\"",
        )?;
        let name = main
            .file_name()
            .and_then(|n| n.to_str())
            .context("worktree_root_unavailable: the main checkout has no UTF-8 name")?;
        let root = if setting == WORKTREE_ROOT_SIBLING {
            main.parent()
                .context("worktree_root_unavailable: the main checkout has no parent directory")?
                .join(format!("{name}.sa-worktrees"))
        } else {
            let base = Path::new(setting);
            if !base.is_absolute() {
                bail!(
                    "invalid_config: worktree_root must be git, sibling or an absolute directory"
                );
            }
            let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
            base.join(format!(
                "{name}-{}",
                &paths::hash(self.common.to_string_lossy().as_bytes())[..8]
            ))
        };
        if root.starts_with(main) {
            bail!(
                "worktree_root_unavailable: {} is inside the main checkout; nested worktrees would show up as untracked files and in project tooling",
                root.display()
            );
        }
        Ok(root)
    }
    /// Every directory that may hold this repository's coordinator worktrees: the
    /// runtime directory (older runs and coordinator-only checkouts) and the configured
    /// root. Ownership checks accept a worktree directly inside any of them.
    pub fn worktree_roots(&self, setting: &str) -> Vec<PathBuf> {
        let mut roots = vec![];
        for candidate in [WORKTREE_ROOT_GIT, setting] {
            if let Ok(root) = self.worktree_root(candidate) {
                let root = root.canonicalize().unwrap_or(root);
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
        }
        roots
    }
    /// True when `path` is a direct child of one of [`Self::worktree_roots`].
    pub fn in_worktree_roots(&self, setting: &str, path: &Path) -> bool {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        path.parent()
            .is_some_and(|parent| self.worktree_roots(setting).iter().any(|r| r == parent))
    }
    /// Coordinator-only checkouts (integration, candidates, archive trials, tools) stay
    /// in the runtime directory.
    pub fn add_worktree(&self, name: &str, base: &str) -> Result<(PathBuf, String)> {
        self.add_worktree_in(WORKTREE_ROOT_GIT, name, base)
    }
    /// Create `codex/sa/<name>` checked out at `base` under the configured root.
    pub fn add_worktree_in(
        &self,
        setting: &str,
        name: &str,
        base: &str,
    ) -> Result<(PathBuf, String)> {
        paths::valid_id(name)?;
        let path = if setting == WORKTREE_ROOT_GIT {
            let path = paths::inside(&self.common, &format!("spec-autonomous/worktrees/{name}"))?;
            fs::create_dir_all(path.parent().unwrap())?;
            path
        } else {
            let root = self.worktree_root(setting)?;
            fs::create_dir_all(&root).with_context(|| {
                format!(
                    "worktree_root_unavailable: cannot create {}; fix its permissions or set [execution] worktree_root = \"git\"",
                    root.display()
                )
            })?;
            let root = root.canonicalize()?;
            if fs::symlink_metadata(root.join(name)).is_ok() {
                bail!(
                    "worktree_exists: {} already exists",
                    root.join(name).display()
                );
            }
            root.join(name)
        };
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
/// `[execution] worktree_root` values with a fixed meaning; anything else is an
/// absolute directory.
pub const WORKTREE_ROOT_GIT: &str = "git";
pub const WORKTREE_ROOT_SIBLING: &str = "sibling";
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
/// Content identity of the committed tree; equal trees have identical tracked contents
/// regardless of commit history.
pub fn tree(root: &Path) -> Result<String> {
    Ok(command(root, &["rev-parse", "HEAD^{tree}"], None)?
        .trim()
        .into())
}
pub fn clean(root: &Path) -> Result<bool> {
    Ok(command(
        root,
        &["status", "--porcelain", "--untracked-files=all"],
        None,
    )?
    .is_empty())
}
/// Paths listed per dirty-checkout category in [`dirty_summary`].
pub const DIRTY_PATH_LIMIT: usize = 20;

/// What makes a checkout dirty, grouped as `modified` (tracked changes, staged or not,
/// including renames and conflicts), `untracked` and `submodules`. Each group carries
/// its full `count` and at most [`DIRTY_PATH_LIMIT`] `paths`; `modified` also counts the
/// `staged` entries. Read-only: it never stages or stashes anything.
pub fn dirty_summary(root: &Path) -> Result<serde_json::Value> {
    let output = command(
        root,
        &["status", "--porcelain=v2", "-z", "--untracked-files=all"],
        None,
    )?;
    let mut groups: [(usize, Vec<String>); 3] = Default::default();
    let mut staged = 0usize;
    let mut fields = output.split('\0').filter(|f| !f.is_empty());
    while let Some(entry) = fields.next() {
        let (kind, rest) = entry.split_at(entry.find(' ').unwrap_or(entry.len()));
        let rest = rest.trim_start();
        let (group, path) = match kind {
            "?" => (1, rest.to_string()),
            "1" | "2" | "u" => {
                // `1 XY sub mH mI mW hH hI path`; `2` adds a score field and is
                // followed by the original path; `u` has three stages of modes/hashes.
                let skip = match kind {
                    "1" => 7,
                    "2" => 8,
                    _ => 9,
                };
                let parts: Vec<&str> = rest.splitn(skip + 1, ' ').collect();
                if kind == "2" {
                    fields.next();
                }
                let xy = parts.first().copied().unwrap_or("..");
                let submodule = parts.get(1).is_some_and(|sub| sub.starts_with('S'));
                if !submodule && kind != "u" && !xy.starts_with('.') {
                    staged += 1;
                }
                (
                    if submodule { 2 } else { 0 },
                    parts.get(skip).copied().unwrap_or_default().to_string(),
                )
            }
            _ => continue,
        };
        let (count, paths) = &mut groups[group];
        *count += 1;
        if paths.len() < DIRTY_PATH_LIMIT {
            paths.push(path);
        }
    }
    let group = |(count, paths): &(usize, Vec<String>)| serde_json::json!({"count":count,"paths":paths,"truncated":*count > paths.len()});
    let mut modified = group(&groups[0]);
    modified["staged"] = serde_json::json!(staged);
    Ok(serde_json::json!({
        "modified": modified,
        "untracked": group(&groups[1]),
        "submodules": group(&groups[2]),
    }))
}

/// A `dirty_checkout` error naming what is dirty, so the operator need not run
/// `git status` to find out. The error code and leading sentence are unchanged.
pub fn dirty_error(root: &Path, action: &str) -> anyhow::Error {
    match dirty_summary(root) {
        Ok(summary) => anyhow::anyhow!("dirty_checkout: {action}; dirty: {summary}"),
        Err(_) => anyhow::anyhow!("dirty_checkout: {action}"),
    }
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
/// Paths touched by a stored binary patch, from its `diff --git a/<path> b/<path>` headers.
pub fn patch_files(patch: &str) -> Vec<String> {
    let mut files = vec![];
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            let path = match rest.rfind(" b/") {
                Some(split) if rest[split + 3..] == rest[..split] => rest[..split].to_string(),
                Some(split) => rest[split + 3..].to_string(),
                None => rest.to_string(),
            };
            if !files.contains(&path) {
                files.push(path);
            }
        }
    }
    files
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
        return Err(dirty_error(root, "cannot advance a changed checkout"));
    }
    command(root, &["merge", "--ff-only", "--no-edit", target], None)?;
    Ok(())
}
