use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

pub fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
pub fn id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub fn valid_id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 120
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("invalid_id: expected a nonempty alphanumeric/hyphen/underscore identifier");
    }
    Ok(())
}
pub fn relative(path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    if path.is_empty()
        || path.contains(['\\', '\0', '\n', '\r'])
        || path.contains(':')
        || p.components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        bail!("path_outside_scope: unsafe relative path {path:?}");
    }
    Ok(p.to_path_buf())
}
/// Check every existing ancestor, including the final file. Never traverse symlinks.
pub fn inside(root: &Path, path: &str) -> Result<PathBuf> {
    let mut full = root.canonicalize().context("project root is missing")?;
    for part in relative(path)?.components() {
        full.push(part);
        match fs::symlink_metadata(&full) {
            Ok(m) if m.file_type().is_symlink() => {
                bail!("path_outside_scope: symbolic link {}", full.display())
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(full)
}
pub fn localize(root: &Path, path: &Path) -> Result<String> {
    let root = root.canonicalize()?;
    let canonical = if path.exists() {
        path.canonicalize()?
    } else {
        path.to_path_buf()
    };
    let rel = canonical
        .strip_prefix(&root)
        .context("external_spec_root_unsupported: upstream path is outside project")?;
    let mut value = rel.to_string_lossy().replace('\\', "/");
    if value.is_empty() {
        value = ".".into();
    }
    inside(&root, &value)?;
    Ok(value)
}
pub fn read(root: &Path, path: &str, limit: usize) -> Result<String> {
    let full = inside(root, path)?;
    if fs::metadata(&full)?.len() > limit as u64 {
        bail!("context_too_large: {path}");
    }
    fs::read_to_string(full).with_context(|| format!("cannot read {path}"))
}
pub fn atomic_write(path: &Path, bytes: impl AsRef<[u8]>) -> Result<()> {
    let parent = path.parent().context("output has no parent")?;
    fs::create_dir_all(parent)?;
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("path_outside_scope: symlink output");
    }
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes.as_ref())?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
