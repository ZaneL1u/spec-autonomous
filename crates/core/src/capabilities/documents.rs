//! Source-aware document operations. Writes use an expected digest and retain
//! all Markdown body bytes when changing only structured frontmatter.
use super::*;
use crate::markdown;

fn file_arg(args: &Value) -> Result<&str> {
    text(args, "file")
}
fn check_target(file: &str) -> Result<()> {
    let normalized = paths::relative(file)?.to_string_lossy().replace('\\', "/");
    if normalized
        .split('/')
        .any(|p| p.eq_ignore_ascii_case(".git"))
    {
        bail!("path_outside_scope: Git metadata is not a document");
    }
    Ok(())
}
fn contents(root: &Path, file: &str) -> Result<String> {
    check_target(file)?;
    paths::read(root, file, 2 * 1024 * 1024)
}
fn selected_framework(root: &Path, args: &Value) -> Framework {
    framework_arg(args)
        .ok()
        .flatten()
        .or_else(|| crate::detect(root, None).ok().and_then(|r| r.selected))
        .unwrap_or(Framework::Openspec)
}
fn split_frontmatter(text: &str) -> Result<Option<(usize, usize)>> {
    let first = text.find('\n').map(|i| i + 1).unwrap_or(text.len());
    if text[..first]
        .trim_end_matches(['\r', '\n'])
        .trim_start_matches('\u{feff}')
        != "---"
    {
        return Ok(None);
    }
    let mut offset = first;
    for line in text[first..].split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Ok(Some((first, offset + line.len())));
        }
        offset += line.len();
    }
    bail!("invalid_document: unterminated frontmatter")
}
fn patch_map(target: &mut Value, patch: &Value) -> Result<()> {
    let patch = patch
        .as_object()
        .context("invalid_patch: expected object")?;
    let target = target
        .as_object_mut()
        .context("invalid_document: metadata must be an object")?;
    for (key, value) in patch {
        if value.is_null() {
            target.remove(key);
        } else {
            target.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}
pub fn invoke(root: &Path, name: &str, args: &Value) -> Result<Value> {
    let file = file_arg(args)?;
    check_target(file)?;
    if name == "document.scaffold" {
        guard_project_idle(root)?;
        let kind = text(args, "kind")?;
        if !["summary", "verification", "handoff", "decision"].contains(&kind) {
            bail!(
                "native_template_required: use native.instructions for provider specification artifacts"
            );
        }
        let path = paths::inside(root, file)?;
        if path.exists() {
            bail!("document_exists: scaffold never overwrites a document");
        }
        let title = args["title"].as_str().unwrap_or(kind);
        let body = format!(
            "---\nschema_version: 1\nkind: {kind}\nstatus: draft\n---\n\n# {title}\n\n## Context\n\n## Evidence\n\n## Next actions\n"
        );
        let repo = Repository::discover(root)?;
        let _lock = Lease::acquire(&repo)?;
        guard_project_idle(root)?;
        if path.exists() {
            bail!("document_exists");
        }
        paths::atomic_write(&path, &body)?;
        return Ok(json!({"file":file,"source_hash":paths::hash(&body),"created":true}));
    }
    let before = contents(root, file)?;
    if name == "document.inspect" || name == "frontmatter.get" {
        let mut view = if file.ends_with(".toml") {
            json!({"file":file,"source_hash":paths::hash(&before),"format":"toml","data":toml::from_str::<toml::Value>(&before)?})
        } else {
            serde_json::to_value(markdown::parse(
                file,
                &before,
                selected_framework(root, args),
            )?)?
        };
        if args["content"] == true {
            view["content"] = json!(before);
        }
        if name == "frontmatter.get" {
            return Ok(
                json!({"file":file,"source_hash":paths::hash(&before),"frontmatter":view.get("frontmatter").or_else(||view.get("data")).cloned().unwrap_or(json!({}))}),
            );
        }
        return Ok(view);
    }
    guard_project_idle(root)?;
    if paths::hash(&before) != text(args, "expected_hash")? {
        bail!("source_drift: document changed since inspection");
    }
    let after = match name {
        "frontmatter.patch" => {
            let mut metadata =
                markdown::parse(file, &before, selected_framework(root, args))?.frontmatter;
            if metadata.is_null() {
                metadata = json!({});
            }
            let old = metadata.clone();
            patch_map(&mut metadata, &args["patch"])?;
            if old == metadata {
                before.clone()
            } else {
                let ending = if before.contains("\r\n") {
                    "\r\n"
                } else {
                    "\n"
                };
                let yaml = serde_yaml::to_string(&metadata)?.replace('\n', ending);
                let body = if let Some((_, end)) = split_frontmatter(&before)? {
                    &before[end..]
                } else {
                    &before
                };
                format!("---{ending}{yaml}---{ending}{body}")
            }
        }
        "document.patch" => {
            let find = text(args, "find")?;
            if find.is_empty() || before.matches(find).count() != 1 {
                bail!("ambiguous_patch: exact text must occur once");
            }
            before.replacen(
                find,
                args["replace"]
                    .as_str()
                    .context("invalid_patch: replacement must be a string")?,
                1,
            )
        }
        "toml.patch" => {
            let mut value = serde_json::to_value(toml::from_str::<toml::Value>(&before)?)?;
            patch_map(&mut value, &args["patch"])?;
            toml::to_string_pretty(&value)?
        }
        _ => bail!("unknown_capability"),
    };
    if file == ".spec-autonomous/config.toml" {
        let config: Config = toml::from_str(&after)?;
        config.validate()?;
    }
    let repo = Repository::discover(root)?;
    let _lock = Lease::acquire(&repo)?;
    guard_project_idle(root)?;
    if paths::hash(contents(root, file)?) != text(args, "expected_hash")? {
        bail!("source_drift: concurrent document write");
    }
    if after != before {
        paths::atomic_write(&paths::inside(root, file)?, &after)?;
    }
    Ok(
        json!({"file":file,"previous_hash":paths::hash(&before),"source_hash":paths::hash(&after),"changed":before!=after}),
    )
}
