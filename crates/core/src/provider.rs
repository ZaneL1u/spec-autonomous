//! Native planning contracts and source extraction. Inspect never creates artifacts.
use crate::{Framework, config::Config, markdown, model::*, paths, process};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

fn files(root: &Path, directory: &str) -> Result<Vec<String>> {
    let base = paths::inside(root, directory)?;
    let mut result = vec![];
    if !base.is_dir() {
        return Ok(result);
    }
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            bail!("path_outside_scope: source symlink");
        }
        let path = paths::localize(root, &entry.path())?;
        if kind.is_dir() {
            result.extend(files(root, &path)?);
        } else if [
            ".md", ".yaml", ".yml", ".toml", ".json", ".sh", ".ps1", ".py",
        ]
        .iter()
        .any(|ext| path.ends_with(ext))
        {
            result.push(path);
        }
    }
    result.sort();
    Ok(result)
}
pub fn openspec_argv(root: &Path, config: &Config) -> Vec<String> {
    if !config.provider.openspec_command.is_empty() {
        return config.provider.openspec_command.clone();
    }
    let local = root.join("node_modules/.bin/openspec");
    if local.is_file() {
        vec![local.to_string_lossy().into()]
    } else {
        vec!["openspec".into()]
    }
}
pub fn openspec(root: &Path, config: &Config, args: &[&str]) -> Result<Value> {
    let mut argv = openspec_argv(root, config);
    argv.extend(args.iter().map(|s| s.to_string()));
    let tmp = tempfile::tempdir()?;
    let env = BTreeMap::from([
        ("OPENSPEC_TELEMETRY".into(), "0".into()),
        ("DO_NOT_TRACK".into(), "1".into()),
    ]);
    let result = process::execute(process::Request {
        argv: &argv,
        cwd: root,
        env: &env,
        stdin: None,
        directory: tmp.path(),
        timeout: Duration::from_secs(30),
        max_log_bytes: 4 * 1024 * 1024,
        cancel: Arc::new(AtomicBool::new(false)),
    })?;
    let output = fs::read_to_string(tmp.path().join("stdout.log"))?;
    let data: Value = serde_json::from_str(&output)
        .context("provider_protocol_error: OpenSpec did not return JSON")?;
    if result.code != 0 {
        bail!("provider_failed: OpenSpec {args:?}: {data}");
    }
    if let Some(path) = data.pointer("/root/path").and_then(Value::as_str) {
        if Path::new(path).canonicalize()? != root.canonicalize()? {
            bail!("external_spec_root_unsupported: OpenSpec resolved another root");
        }
    }
    Ok(data)
}
fn contexts(root: &Path, paths_: Vec<String>, config: &Config) -> Result<Vec<ContextFile>> {
    let mut result = vec![];
    let mut total = 0usize;
    let mut paths_ = paths_;
    paths_.sort();
    paths_.dedup();
    for path in paths_ {
        let content = paths::read(root, &path, config.execution.max_source_bytes)?;
        total += content.len();
        if total > config.execution.max_source_bytes {
            bail!("context_too_large: split the source scope before dispatch");
        }
        result.push(ContextFile {
            hash: paths::hash(&content),
            path,
            content,
        });
    }
    Ok(result)
}
fn finish(mut s: Snapshot) -> Result<Snapshot> {
    s.source_hash = paths::hash(serde_json::to_vec(
        &s.context_files
            .iter()
            .map(|f| (&f.path, &f.hash))
            .collect::<Vec<_>>(),
    )?);
    let mut documents = vec![];
    let mut acceptance = vec![];
    let requirement_id = regex::Regex::new(r"(?:FR|NFR|SC)-[0-9]+")?;
    for file in &s.context_files {
        if !file.path.ends_with(".md") {
            continue;
        }
        match markdown::parse(&file.path,&file.content,s.framework){
            Ok(doc)=>{
                let mut found=false;
                for heading in &doc.headings{
                    let normative=heading.text.starts_with("Requirement:")||heading.text.starts_with("Scenario:");
                    if normative{
                        let id=format!("accept-{}",&paths::hash(format!("{}:{}:{}",file.path,heading.level,heading.text))[..20]);
                        acceptance.push(json!({"id":id,"source_path":file.path,"line":heading.line,"description":heading.text}));found=true;
                    }
                }
                let feature_spec=file.path==format!("{}/spec.md",s.source_dir);
                if s.framework==Framework::Speckit&&feature_spec{
                    for (line,text) in file.content.lines().enumerate(){if let Some(m)=requirement_id.find(text){acceptance.push(json!({"id":format!("{}#{}",file.path,m.as_str()),"source_path":file.path,"line":line+1,"description":text.trim()}));found=true;}}
                }
                if (!found&&feature_spec)||file.path==".specify/memory/constitution.md"{
                    acceptance.push(json!({"id":format!("{}#document",file.path),"source_path":file.path,"line":1,"description":"Verify all requirements and constraints in this native document"}));
                }
                documents.push(json!({"source_path":file.path,"source_hash":file.hash,"parser_profile":doc.parser_profile,"frontmatter":doc.frontmatter,"headings":doc.headings,"diagnostics":doc.diagnostics}));
            },Err(error)=>documents.push(json!({"source_path":file.path,"source_hash":file.hash,"diagnostics":[error.to_string()]})),
        }
    }
    acceptance.sort_by_key(|a| a["id"].as_str().unwrap_or("").to_string());
    acceptance.dedup_by(|a, b| a["id"] == b["id"]);
    if acceptance.is_empty() {
        for task in &s.tasks {
            acceptance.push(json!({"id":format!("{}#{}",task.source_path,task.id),"source_path":task.source_path,"line":task.line,"description":task.description}));
        }
    }
    s.metadata["documents"] = json!(documents);
    s.metadata["acceptance"] = json!(acceptance);
    if !s.diagnostics.is_empty() && s.next_action.kind == "implement" {
        s.next_action.kind = "blocked".into();
        s.next_action.instruction = s.diagnostics.join("; ");
    }
    Ok(s)
}
pub fn check_audit(snapshot: &Snapshot, audit: &[AuditItem]) -> Result<()> {
    let required: std::collections::BTreeSet<_> = snapshot.metadata["acceptance"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v["id"].as_str())
        .collect();
    if required.is_empty() {
        bail!("acceptance_missing: no native acceptance references found");
    }
    let actual: std::collections::BTreeSet<_> =
        audit.iter().map(|a| a.requirement.as_str()).collect();
    if actual != required
        || actual.len() != audit.len()
        || audit.iter().any(|a| a.evidence.trim().is_empty())
    {
        bail!(
            "audit_contract_error: audit must cover every exact metadata.acceptance ID once with evidence"
        );
    }
    Ok(())
}
pub fn check_hooks(root: &Path, config: &Config) -> Result<()> {
    let path = paths::inside(root, ".specify/extensions.yml")?;
    if !path.exists() {
        return Ok(());
    }
    let value: Value = serde_yaml::from_str(&fs::read_to_string(path)?)?;
    let Some(events) = value["hooks"].as_object() else {
        return Ok(());
    };
    for (event, hooks) in events {
        {
            for hook in hooks
                .as_array()
                .context("hook_invalid: expected hook array")?
            {
                if hook["enabled"] == false {
                    continue;
                }
                let command = hook["command"]
                    .as_str()
                    .context("hook_invalid: missing command")?;
                let supported = [
                    "constitution",
                    "specify",
                    "plan",
                    "tasks",
                    "implement",
                    "converge",
                ]
                .iter()
                .any(|stage| {
                    event == &format!("before_{stage}") || event == &format!("after_{stage}")
                });
                if !supported {
                    if hook["optional"] != true {
                        bail!(
                            "hook_unsupported: native stage event {event} is not part of this execution profile"
                        );
                    }
                    continue;
                }
                if hook["optional"] == true && !config.hooks.contains_key(command) {
                    continue;
                }
                if hook["condition"].as_str().is_some_and(|s| !s.is_empty()) {
                    bail!("hook_unsupported: conditional hook {command}");
                }
                if hook["optional"] != true && !config.hooks.contains_key(command) {
                    bail!("hook_unsupported: missing argv binding for mandatory {event} {command}");
                }
                if let Some(binding) = config.hooks.get(command) {
                    crate::config::validate_check(&Check {
                        argv: binding.argv.clone(),
                        cwd: ".".into(),
                    })?;
                }
            }
        }
    }
    Ok(())
}
pub fn inspect(
    root: &Path,
    framework: Framework,
    selector: &str,
    config: &Config,
) -> Result<Snapshot> {
    if config.execution.max_context_bytes < 1024 {
        bail!("context_too_large: even a minimal worker packet cannot fit the configured budget");
    }
    match framework {
        Framework::Openspec => inspect_openspec(root, selector, config),
        Framework::Speckit => inspect_speckit(root, selector, config),
    }
}
fn inspect_openspec(root: &Path, selector: &str, config: &Config) -> Result<Snapshot> {
    paths::valid_id(selector)?;
    let source_dir = format!("openspec/changes/{selector}");
    let source = paths::inside(root, &source_dir)?;
    if !source.exists() {
        // Confirm a real local provider root before proposing creation.
        let _ = openspec(root, config, &["list", "--json"])?;
        return finish(Snapshot {
            schema_version: SCHEMA,
            framework: Framework::Openspec,
            selector: selector.into(),
            source_dir,
            next_action: NativeAction {
                kind: "create".into(),
                artifact: "change".into(),
                instruction: "Create a native OpenSpec change".into(),
                outputs: vec![],
            },
            planning_ready: false,
            tracking_file: None,
            tasks: vec![],
            context_files: vec![],
            diagnostics: vec![],
            source_hash: String::new(),
            metadata: json!({}),
        });
    }
    let status = openspec(root, config, &["status", "--change", selector, "--json"])?;
    let mut context_paths = files(root, &source_dir)?;
    if root.join("openspec/config.yaml").is_file() {
        context_paths.push("openspec/config.yaml".into());
    }
    if root.join("AGENTS.md").is_file() {
        context_paths.push("AGENTS.md".into());
    }
    let mut s = Snapshot {
        schema_version: SCHEMA,
        framework: Framework::Openspec,
        selector: selector.into(),
        source_dir,
        planning_ready: false,
        tracking_file: None,
        tasks: vec![],
        context_files: vec![],
        diagnostics: vec![],
        source_hash: String::new(),
        next_action: NativeAction {
            kind: "blocked".into(),
            artifact: String::new(),
            instruction: "No ready native artifact".into(),
            outputs: vec![],
        },
        metadata: status.clone(),
    };
    let artifacts = status["artifacts"]
        .as_array()
        .context("provider_protocol_error: missing artifacts")?;
    if let Some(artifact) = artifacts.iter().find(|a| a["status"] == "ready") {
        let id = artifact["id"]
            .as_str()
            .context("provider_protocol_error: artifact id")?;
        let instructions = openspec(
            root,
            config,
            &["instructions", id, "--change", selector, "--json"],
        )?;
        let output = instructions["outputPath"]
            .as_str()
            .context("provider_protocol_error: outputPath")?;
        let output_path = format!("{}/{}", s.source_dir, output);
        paths::relative(&output_path)?;
        s.next_action = NativeAction {
            kind: "planning".into(),
            artifact: id.into(),
            instruction: serde_json::to_string_pretty(&instructions)?,
            outputs: vec![output_path],
        };
    } else if artifacts
        .iter()
        .all(|a| a["status"] == "done" || a["status"] == "skipped")
    {
        let apply = openspec(
            root,
            config,
            &["instructions", "apply", "--change", selector, "--json"],
        )?;
        if !["ready", "all_done", "blocked"].contains(&apply["state"].as_str().unwrap_or("")) {
            bail!("provider_protocol_error: unknown OpenSpec apply state");
        }
        let _ = openspec(
            root,
            config,
            &[
                "validate",
                selector,
                "--strict",
                "--json",
                "--no-interactive",
            ],
        )?;
        let context = apply["contextFiles"]
            .as_object()
            .context("provider_protocol_error: contextFiles")?;
        for paths_ in context.values() {
            for p in paths_
                .as_array()
                .context("provider_protocol_error: context file array")?
            {
                context_paths.push(paths::localize(
                    root,
                    Path::new(p.as_str().context("invalid context path")?),
                )?);
            }
        }
        // Native apply exposes the tracked tasks context. A single concrete file
        // is required; schemas with no tracking contract remain unsupported.
        let tracks: Vec<_> = context
            .get("tasks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let tracked = if tracks.len() == 1 {
            paths::localize(root, Path::new(tracks[0]))?
        } else {
            // Custom artifact IDs: find the unique context document with the exact
            // native apply task descriptions. Never assume its filename.
            let native: Vec<_> = apply["tasks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v["description"].as_str())
                .collect();
            let mut candidates = vec![];
            for path in &context_paths {
                if !path.ends_with(".md") {
                    continue;
                }
                let doc = markdown::parse(
                    path,
                    &paths::read(root, path, config.execution.max_source_bytes)?,
                    Framework::Openspec,
                )?;
                if !native.is_empty()
                    && doc
                        .tasks
                        .iter()
                        .map(|t| t.description.as_str())
                        .collect::<Vec<_>>()
                        == native
                {
                    candidates.push(path.clone());
                }
            }
            candidates.sort();
            candidates.dedup();
            if candidates.len() != 1 {
                bail!("tracking_unsupported: expected one uniquely identifiable tracking document");
            }
            candidates.remove(0)
        };
        let doc = markdown::parse(
            &tracked,
            &paths::read(root, &tracked, config.execution.max_source_bytes)?,
            Framework::Openspec,
        )?;
        let total = apply
            .pointer("/progress/total")
            .and_then(Value::as_u64)
            .context("tracking_unsupported: no native progress")?;
        if total != doc.tasks.len() as u64 {
            bail!("tracking_unsupported: native and source task counts differ");
        }
        let native = apply["tasks"]
            .as_array()
            .context("tracking_unsupported: missing native task identities")?;
        if native.len() != doc.tasks.len()
            || native.iter().zip(&doc.tasks).any(|(native, task)| {
                native["description"].as_str() != Some(task.description.as_str())
                    || native["done"].as_bool() != Some(task.done)
            })
        {
            bail!(
                "tracking_unsupported: native task descriptions or completion differ from source"
            );
        }
        s.diagnostics.extend(doc.diagnostics);
        s.tasks = doc.tasks;
        s.tracking_file = Some(tracked);
        s.planning_ready = true;
        s.next_action = NativeAction {
            kind: if apply["state"] == "blocked" {
                "blocked"
            } else {
                "implement"
            }
            .into(),
            artifact: "apply".into(),
            instruction: serde_json::to_string_pretty(&apply)?,
            outputs: vec![],
        };
    }
    s.context_files = contexts(root, context_paths, config)?;
    finish(s)
}
pub fn select_feature(root: &Path, explicit: Option<&str>) -> Result<String> {
    let selected = if let Some(v) = explicit {
        v.to_string()
    } else if let Ok(v) = std::env::var("SPECIFY_FEATURE_DIRECTORY") {
        v
    } else {
        let file = paths::inside(root, ".specify/feature.json")?;
        if !file.exists() {
            bail!("selection_required: specify --feature or set native feature context");
        }
        serde_json::from_slice::<Value>(&fs::read(file)?)?["feature_directory"]
            .as_str()
            .context("invalid_feature_context: feature_directory is missing")?
            .into()
    };
    if Path::new(&selected).is_absolute() {
        paths::localize(root, Path::new(&selected))
    } else {
        paths::inside(root, &selected)?;
        Ok(selected)
    }
}
fn native_skill(root: &Path, stage: &str, config: &Config) -> Result<ContextFile> {
    for p in [
        format!(".agents/skills/speckit-{stage}/SKILL.md"),
        format!(".claude/commands/speckit.{stage}.md"),
        format!(".specify/templates/commands/{stage}.md"),
    ] {
        if paths::inside(root, &p)?.is_file() {
            let content = paths::read(root, &p, config.execution.max_source_bytes)?;
            return Ok(ContextFile {
                path: p,
                hash: paths::hash(&content),
                content,
            });
        }
    }
    bail!(
        "native_bridge_unavailable: install the native Spec Kit {stage} skill/template before planning"
    )
}
fn inspect_speckit(root: &Path, selector: &str, config: &Config) -> Result<Snapshot> {
    let selector = select_feature(root, Some(selector))?;
    let mut paths_ = files(root, &selector)?;
    let constitution = ".specify/memory/constitution.md";
    if root.join(constitution).is_file() {
        paths_.push(constitution.into());
    }
    for path in [".specify/extensions.yml", ".specify/integration.json"] {
        if paths::inside(root, path)?.is_file() {
            paths_.push(path.into());
        }
    }
    if root.join("AGENTS.md").is_file() {
        paths_.push("AGENTS.md".into());
    }
    let mut s = Snapshot {
        schema_version: SCHEMA,
        framework: Framework::Speckit,
        selector: selector.clone(),
        source_dir: selector.clone(),
        planning_ready: false,
        tracking_file: None,
        tasks: vec![],
        context_files: vec![],
        diagnostics: vec![],
        source_hash: String::new(),
        next_action: NativeAction {
            kind: "implement".into(),
            artifact: "implement".into(),
            instruction: String::new(),
            outputs: vec![],
        },
        metadata: json!({"profile":"feature-directory-v1"}),
    };
    let stages = [
        ("constitution", constitution.to_string()),
        ("specify", format!("{selector}/spec.md")),
        ("plan", format!("{selector}/plan.md")),
        ("tasks", format!("{selector}/tasks.md")),
    ];
    for (stage, path) in stages {
        if !paths::inside(root, &path)?.is_file() {
            match native_skill(root, stage, config) {
                Ok(skill) => {
                    s.next_action = NativeAction {
                        kind: "planning".into(),
                        artifact: stage.into(),
                        instruction: format!(
                            "Follow the installed native Spec Kit skill. Feature directory: {selector}\n{}",
                            skill.content
                        ),
                        outputs: match stage {
                            "specify" => vec![path, format!("{selector}/checklists/**")],
                            "plan" => vec![
                                path,
                                format!("{selector}/research.md"),
                                format!("{selector}/data-model.md"),
                                format!("{selector}/quickstart.md"),
                                format!("{selector}/contracts/**"),
                            ],
                            _ => vec![path],
                        },
                    };
                    paths_.push(skill.path);
                }
                Err(e) => {
                    s.next_action = NativeAction {
                        kind: "blocked".into(),
                        artifact: stage.into(),
                        instruction: e.to_string(),
                        outputs: vec![],
                    }
                }
            }
            s.context_files = contexts(root, paths_, config)?;
            return finish(s);
        }
    }
    for path in files(root, &format!("{selector}/checklists"))? {
        let text = paths::read(root, &path, config.execution.max_source_bytes)?;
        if text.lines().any(|l| l.trim_start().starts_with("- [ ]")) {
            s.diagnostics.push(format!("checklist_incomplete: {path}"));
        }
    }
    let tracking = format!("{selector}/tasks.md");
    let doc = markdown::parse(
        &tracking,
        &paths::read(root, &tracking, config.execution.max_source_bytes)?,
        Framework::Speckit,
    )?;
    s.diagnostics.extend(doc.diagnostics);
    s.metadata["document"] = serde_json::to_value(&doc.headings)?;
    s.tasks = doc.tasks;
    s.tracking_file = Some(tracking);
    s.planning_ready = true;
    s.next_action.instruction="Implement only the assigned native task, respecting spec.md, plan.md, constitution, phase/story dependencies and checklists. The coordinator alone updates tasks.md and lifecycle hooks.".into();
    if let Ok(skill) = native_skill(root, "implement", config) {
        s.next_action.instruction.push_str(&format!(
            "\nNative implementation contract (coordinator retains checklist/hook ownership):\n{}",
            skill.content
        ));
        paths_.push(skill.path);
    }
    if let Err(error) = check_hooks(root, config) {
        s.diagnostics.push(error.to_string());
    }
    s.context_files = contexts(root, paths_, config)?;
    finish(s)
}
pub fn create_source(
    root: &Path,
    framework: Framework,
    selector: &str,
    config: &Config,
) -> Result<()> {
    match framework {
        Framework::Openspec => {
            openspec(root, config, &["new", "change", selector, "--json"])?;
        }
        Framework::Speckit => {
            fs::create_dir_all(paths::inside(root, selector)?)?;
        }
    }
    Ok(())
}
pub fn copy_ignored_context(origin: &Path, target: &Path) -> Result<()> {
    for dir in ["openspec", ".specify", ".agents/skills", ".claude/commands"] {
        for file in files(origin, dir)? {
            // Only import native rules and scripts, never conventional secret
            // stores that happen to use the same JSON/TOML/YAML extensions.
            let name = Path::new(&file)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase();
            let stem = Path::new(&name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            if name.starts_with(".env")
                || [
                    "auth",
                    "credentials",
                    "secrets",
                    "secret",
                    "tokens",
                    "token",
                ]
                .contains(&stem.as_ref())
                    && !name.ends_with(".md")
            {
                continue;
            }
            let dest = paths::inside(target, &file)?;
            if !dest.exists() {
                paths::atomic_write(&dest, paths::read(origin, &file, 512 * 1024)?)?;
                fs::set_permissions(
                    &dest,
                    fs::metadata(paths::inside(origin, &file)?)?.permissions(),
                )?;
            }
        }
    }
    Ok(())
}
/// Restore only the native checkout selector, retaining all original bytes.
/// Spec Kit's specify stage may update it locally; environment selection drives
/// our workers, so that ephemeral pointer is never part of an integration patch.
pub fn restore_feature_pointer(origin: &Path, worker: &Path) -> Result<()> {
    let source = paths::inside(origin, ".specify/feature.json")?;
    let target = paths::inside(worker, ".specify/feature.json")?;
    if source.is_file() {
        paths::atomic_write(&target, fs::read(source)?)?;
    } else if target.exists() {
        fs::remove_file(target)?;
    }
    Ok(())
}
pub fn framework(root: &Path, explicit: Option<Framework>) -> Result<Framework> {
    let report = crate::detect(root, explicit)?;
    report
        .selected
        .context("selection_required: initialize your native SDD provider or select --framework")
}
pub fn milestone_path(root: &Path, id: &str) -> Result<PathBuf> {
    paths::valid_id(id)?;
    paths::inside(
        root,
        &format!(".spec-autonomous/milestones/{id}/milestone.toml"),
    )
}
pub fn load_milestone(root: &Path, id: &str) -> Result<Milestone> {
    let m: Milestone = toml::from_str(&fs::read_to_string(milestone_path(root, id)?)?)?;
    crate::plan::validate_milestone(&m)?;
    Ok(m)
}
pub fn save_milestone(root: &Path, m: &Milestone) -> Result<()> {
    crate::plan::validate_milestone(m)?;
    let path = milestone_path(root, &m.id)?;
    let roadmap = path.with_file_name("ROADMAP.md");
    let marker = "<!-- Generated by spec-autonomous; edit milestone.toml, then regenerate. -->\n";
    let body = format!(
        "{marker}# {}\n\n{}\n\n{}",
        m.id,
        m.goal,
        m.phases
            .iter()
            .map(|p| format!(
                "- **{} {}** — {} (`{}`), depends: {}\n",
                p.label,
                p.title,
                p.source.selector,
                p.id,
                p.depends_on.join(", ")
            ))
            .collect::<String>()
    );
    if roadmap.exists() {
        let old = fs::read_to_string(&roadmap)?;
        let hashfile = path.with_file_name("roadmap.sha256");
        if !hashfile.exists() || fs::read_to_string(&hashfile)?.trim() != paths::hash(&old) {
            bail!("roadmap_view_modified: preserve user edits before regenerating");
        }
    }
    paths::atomic_write(&path, toml::to_string_pretty(m)?)?;
    paths::atomic_write(&roadmap, &body)?;
    paths::atomic_write(&path.with_file_name("roadmap.sha256"), paths::hash(&body))
}
