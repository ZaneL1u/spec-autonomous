//! Read-only framework discovery. Execution adapters are planned in OpenSpec.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Framework {
    Openspec,
    Speckit,
}

#[derive(Debug, Serialize)]
pub struct Detection {
    pub framework: Framework,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DetectionReport {
    pub schema_version: u32,
    pub root: PathBuf,
    pub detected: Vec<Detection>,
    pub selected: Option<Framework>,
    pub ambiguous: bool,
    pub warnings: Vec<String>,
}

// Do not follow framework-marker symlinks outside the inspected repository.
fn real_path(path: &Path, directory: bool) -> Result<bool> {
    match path.symlink_metadata() {
        Ok(meta) => Ok(!meta.file_type().is_symlink()
            && if directory {
                meta.is_dir()
            } else {
                meta.is_file()
            }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("cannot inspect {}", path.display())),
    }
}

fn inspect(root: &Path) -> Result<(Vec<Detection>, Vec<String>)> {
    let mut detected = Vec::new();
    let mut warnings = Vec::new();
    for (name, framework, markers) in [
        (
            "openspec",
            Framework::Openspec,
            &["config.yaml", "specs/", "changes/"][..],
        ),
        (
            ".specify",
            Framework::Speckit,
            &[
                "memory/constitution.md",
                "scripts/",
                "templates/",
                "feature.json",
            ][..],
        ),
    ] {
        let base = root.join(name);
        if !real_path(&base, true)? {
            if base.symlink_metadata().is_ok() {
                warnings.push(format!("{name} is not a regular directory; ignored"));
            }
            continue;
        }
        let mut evidence = Vec::new();
        for marker in markers {
            // All parent components must also be real directories.
            let rel = Path::new(marker.trim_end_matches('/'));
            let mut parents_ok = true;
            if let Some(parent) = rel.parent() {
                let mut cursor = base.clone();
                for component in parent.components() {
                    cursor.push(component);
                    parents_ok &= real_path(&cursor, true)?;
                }
            }
            if parents_ok && real_path(&base.join(rel), marker.ends_with('/'))? {
                evidence.push(format!("{name}/{marker}"));
            }
        }
        if evidence.is_empty() {
            warnings.push(format!(
                "{name}/ exists but has no recognized markers; incomplete setup"
            ));
        } else {
            detected.push(Detection {
                framework,
                evidence,
            });
        }
    }
    Ok((detected, warnings))
}

/// Find the nearest framework root, without traversing beyond a Git boundary.
/// Marker detection is evidence of installation, not proof that tasks are runnable.
pub fn detect(path: &Path, requested: Option<Framework>) -> Result<DetectionReport> {
    let start = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", path.display()))?;
    if !start.is_dir() {
        bail!("repository path must be a directory: {}", start.display());
    }
    let mut root = start.clone();
    let (detected, warnings) = loop {
        let found = inspect(&root)?;
        if !found.0.is_empty() || !found.1.is_empty() || root.join(".git").exists() {
            break found;
        }
        if !root.pop() {
            root = start;
            break (Vec::new(), Vec::new());
        }
    };
    let selected = if let Some(framework) = requested {
        if !detected.iter().any(|item| item.framework == framework) {
            bail!(
                "requested framework {framework:?} was not detected at {}",
                root.display()
            );
        }
        Some(framework)
    } else if detected.len() == 1 {
        Some(detected[0].framework)
    } else {
        None
    };
    Ok(DetectionReport {
        schema_version: 1,
        root,
        ambiguous: detected.len() > 1 && requested.is_none(),
        detected,
        selected,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn touch(root: &Path, name: &str) {
        let file = root.join(name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, "").unwrap();
    }

    #[test]
    fn finds_openspec_from_nested_directory() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "openspec/config.yaml");
        fs::create_dir_all(dir.path().join("crates/core")).unwrap();
        let report = detect(&dir.path().join("crates/core"), None).unwrap();
        assert_eq!(report.selected, Some(Framework::Openspec));
        assert_eq!(report.root, dir.path().canonicalize().unwrap());
    }

    #[test]
    fn mixed_frameworks_require_selection() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "openspec/config.yaml");
        touch(dir.path(), ".specify/feature.json");
        let report = detect(dir.path(), None).unwrap();
        assert!(report.ambiguous);
        assert_eq!(report.selected, None);
        assert_eq!(
            detect(dir.path(), Some(Framework::Speckit))
                .unwrap()
                .selected,
            Some(Framework::Speckit)
        );
    }

    #[test]
    fn arbitrary_specs_and_references_do_not_count() {
        let dir = tempdir().unwrap();
        touch(dir.path(), ".git");
        touch(dir.path(), "specs/001-demo/tasks.md");
        touch(dir.path(), ".references/openspec/openspec/config.yaml");
        assert!(detect(dir.path(), None).unwrap().detected.is_empty());
        assert!(detect(dir.path(), Some(Framework::Speckit)).is_err());
    }

    #[test]
    fn git_boundary_prevents_parent_framework_leak() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "openspec/config.yaml");
        touch(dir.path(), "nested/.git");
        fs::create_dir_all(dir.path().join("nested/src")).unwrap();
        assert!(
            detect(&dir.path().join("nested/src"), None)
                .unwrap()
                .detected
                .is_empty()
        );
    }

    #[test]
    fn partial_setup_warns_and_file_input_fails() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("openspec")).unwrap();
        let report = detect(dir.path(), None).unwrap();
        assert!(report.detected.is_empty());
        assert_eq!(report.warnings.len(), 1);
        touch(dir.path(), "a-file");
        assert!(detect(&dir.path().join("a-file"), None).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn ignores_symlinked_framework_and_nested_markers() {
        use std::os::unix::fs::symlink;
        let dir = tempdir().unwrap();
        let external = tempdir().unwrap();
        touch(external.path(), "config.yaml");
        touch(external.path(), "constitution.md");
        symlink(external.path(), dir.path().join("openspec")).unwrap();
        fs::create_dir(dir.path().join(".specify")).unwrap();
        symlink(external.path(), dir.path().join(".specify/memory")).unwrap();
        let report = detect(dir.path(), None).unwrap();
        assert!(report.detected.is_empty());
        assert_eq!(report.warnings.len(), 2);
    }
}
