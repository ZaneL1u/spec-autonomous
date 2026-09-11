//! Lifecycle operations run in isolated Git candidates and finalize by FF.
//! A durable operation record reconciles a crash without replaying the archive.
use super::*;
use crate::process;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Operation {
    id: String,
    kind: String,
    plan_hash: String,
    expected_head: String,
    #[serde(default)]
    origin_branch: String,
    candidate: String,
    final_head: Option<String>,
    status: String,
    result: Value,
}
fn operation_path(repo: &Repository, id: &str) -> Result<PathBuf> {
    paths::valid_id(id)?;
    paths::inside(
        &repo.common,
        &format!("spec-autonomous/operations/{id}.json"),
    )
}
fn tree_hash(root: &Path, relative: &str) -> Result<String> {
    fn walk(root: &Path, path: &str, entries: &mut Vec<(String, String)>) -> Result<()> {
        let full = paths::inside(root, path)?;
        if !full.exists() {
            return Ok(());
        }
        if full.is_dir() {
            let mut children = fs::read_dir(&full)?.collect::<std::io::Result<Vec<_>>>()?;
            children.sort_by_key(|e| e.file_name());
            for child in children {
                if child.file_type()?.is_symlink() {
                    bail!("path_outside_scope: archive refuses symlinks");
                }
                let relative = paths::localize(root, &child.path())?;
                walk(root, &relative, entries)?;
            }
        } else {
            let data = fs::read(full)?;
            entries.push((path.into(), paths::hash(data)));
        }
        if entries.len() > 10000 {
            bail!("source_too_large: archive inventory exceeds 10000 files");
        }
        Ok(())
    }
    let mut entries = vec![];
    walk(root, relative, &mut entries)?;
    Ok(paths::hash(serde_json::to_vec(&entries)?))
}
fn preview_source(root: &Path, args: &Value) -> Result<Value> {
    let config = Config::load(root)?;
    let framework = provider::framework(root, framework_arg(args)?)?;
    let selector = source_arg(root, args, framework)?;
    let snap = provider::inspect(root, framework, &selector, &config)?;
    let repo = Repository::discover(root)?;
    repo.preflight()?;
    guard_project_idle(root)?;
    if !snap.planning_ready
        || snap.next_action.kind == "blocked"
        || snap.tasks.is_empty()
        || snap.tasks.iter().any(|t| !t.done)
    {
        bail!(
            "archive_blocked: complete the native planning, checklists and tasks before archiving"
        );
    }
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let slug = Path::new(&selector)
        .file_name()
        .and_then(|p| p.to_str())
        .context("invalid_source")?;
    paths::valid_id(slug)?;
    let target = if framework == Framework::Openspec {
        format!("openspec/changes/archive/{date}-{selector}")
    } else {
        format!("specs/archive/{date}-{slug}")
    };
    let source = paths::inside(root, &snap.source_dir)?;
    let destination = paths::inside(root, &target)?;
    if source == root || destination.starts_with(&source) || destination.exists() {
        bail!("archive_scope_conflict: invalid or existing destination");
    }
    let mut plan = json!({"schema_version":1,"kind":"archive","framework":framework,"selector":selector,"source":snap.source_dir,"destination":target,"origin_branch":repo.branch()?,"expected_head":repo.head()?,"source_hash":tree_hash(root,&snap.source_dir)?,"native_specs_hash":if framework==Framework::Openspec{tree_hash(root,"openspec/specs")?}else{tree_hash(root,".specify")?},"config_hash":paths::hash(serde_json::to_vec(&config)?),"native_tasks":snap.tasks.len(),"completion_basis":"native-checklist-and-configured-verification","starts_agents":false});
    let hash = paths::hash(serde_json::to_vec(&plan)?);
    plan["plan_hash"] = json!(hash);
    plan["operation_id"] = json!(format!("archive-{}", &hash[..24]));
    Ok(plan)
}
fn preview(root: &Path, args: &Value) -> Result<Value> {
    let Some(id) = args["milestone_id"].as_str() else {
        return preview_source(root, args);
    };
    let milestone = provider::load_milestone(root, id)?;
    if framework_arg(args)?.is_some_and(|f| f != milestone.framework) {
        bail!("selection_conflict: milestone provider differs");
    }
    let mut done = vec![];
    let mut sources = vec![];
    while done.len() < milestone.phases.len() {
        let phase = milestone
            .phases
            .iter()
            .find(|p| !done.contains(&p.id) && p.depends_on.iter().all(|d| done.contains(d)))
            .context("invalid_roadmap: archive dependency cycle")?;
        let mut plan = preview_source(
            root,
            &json!({"framework":milestone.framework,"selector":phase.source.selector}),
        )?;
        plan["phase_id"] = json!(phase.id);
        sources.push(plan);
        done.push(phase.id.clone());
    }
    let source = format!(".spec-autonomous/milestones/{id}");
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let destination = format!(".spec-autonomous/milestones/archive/{date}-{id}");
    if paths::inside(root, &destination)?.exists()
        || paths::inside(root, &destination)?.starts_with(paths::inside(root, &source)?)
    {
        bail!("archive_scope_conflict");
    }
    let mut plan = json!({"schema_version":1,"kind":"milestone-archive","framework":milestone.framework,"milestone_id":id,"source":source,"destination":destination,"milestone_hash":tree_hash(root,&source)?,"sources":sources,"origin_branch":Repository::discover(root)?.branch()?,"expected_head":Repository::discover(root)?.head()?,"starts_agents":false});
    let hash = paths::hash(serde_json::to_vec(&plan)?);
    plan["plan_hash"] = json!(hash);
    plan["operation_id"] = json!(format!("archive-{}", &hash[..24]));
    Ok(plan)
}
fn archive_source(
    origin: &Path,
    project: &Path,
    framework: Framework,
    config: &Config,
    plan: &Value,
) -> Result<Value> {
    let native = if framework == Framework::Openspec {
        provider::openspec(
            project,
            config,
            &["archive", text(plan, "selector")?, "--yes", "--json"],
        )?
    } else {
        let source = paths::inside(project, text(plan, "source")?)?;
        let destination = paths::inside(project, text(plan, "destination")?)?;
        fs::create_dir_all(destination.parent().unwrap())?;
        fs::rename(source, destination)?;
        let pointer = paths::inside(project, ".specify/feature.json")?;
        if pointer.exists() {
            let before = fs::read_to_string(&pointer)?;
            let mut value: Value = serde_json::from_str(&before)?;
            if let Some(selected) = value["feature_directory"].as_str() {
                let selected = if Path::new(selected).is_absolute() {
                    paths::localize(origin, Path::new(selected))?
                } else {
                    selected.to_string()
                };
                if selected == plan["selector"].as_str().unwrap() {
                    value
                        .as_object_mut()
                        .context("invalid_feature_pointer")?
                        .remove("feature_directory");
                    paths::atomic_write(&pointer, serde_json::to_vec_pretty(&value)?)?;
                }
            }
        }
        json!({"provider_operation":"feature-directory-archive","source":plan["source"],"destination":plan["destination"]})
    };
    if paths::inside(project, text(plan, "source")?)?.exists()
        || !paths::inside(project, text(plan, "destination")?)?.is_dir()
    {
        bail!("archive_postcondition_failed: native layout differs from preview");
    }
    Ok(native)
}
fn finalize(repo: &Repository, root: &Path, mut op: Operation) -> Result<Value> {
    let target = op
        .final_head
        .clone()
        .context("operation_incomplete: no verified candidate")?;
    if op.origin_branch.is_empty() || repo.branch()? != op.origin_branch {
        bail!("delivery_pending: original archive branch changed");
    }
    let actual = repo.head()?;
    if actual != target {
        if actual != op.expected_head || !git::clean(&repo.root)? {
            bail!("delivery_pending: archive candidate retained; original checkout changed");
        }
        git::advance(&repo.root, &target)?;
        testpoint("after_archive_advance");
    }
    op.status = "completed".into();
    op.result["status"] = json!("archived");
    op.result["accepted_head"] = json!(target);
    op.result["project"] = json!(root);
    paths::atomic_write(
        &operation_path(repo, &op.id)?,
        serde_json::to_vec_pretty(&op)?,
    )?;
    Ok(op.result)
}
pub fn archive(root: &Path, args: &Value) -> Result<Value> {
    if args["apply"] != true {
        return preview(root, args);
    }
    let expected = text(args, "plan_hash")?;
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("invalid_plan_hash");
    }
    let id = format!("archive-{}", &expected[..24]);
    let repo = Repository::discover(root)?;
    let _lease = Lease::acquire(&repo)?;
    guard_project_idle(root)?;
    let record = operation_path(&repo, &id)?;
    if record.exists() {
        let op: Operation = serde_json::from_slice(&fs::read(&record)?)?;
        if op.plan_hash != expected {
            bail!("operation_identity_mismatch");
        }
        if op.status == "completed" {
            let mut r = op.result;
            r["replayed"] = json!(true);
            return Ok(r);
        }
        if op.final_head.is_some() {
            return finalize(&repo, root, op);
        }
        bail!(
            "operation_needs_repair: inspect the preserved candidate; repair can abandon an uncommitted operation before retry"
        );
    }
    let plan = preview(root, args)?;
    if plan["plan_hash"].as_str() != Some(expected) {
        bail!("source_drift: archive preview is stale");
    }
    let mut config = Config::load(root)?;
    let framework: Framework = serde_json::from_value(plan["framework"].clone())?;
    if framework == Framework::Openspec {
        config.provider.openspec_command = provider::openspec_argv(root, &config);
    }
    let relative = paths::localize(&repo.root, root)?;
    let (candidate, branch) =
        repo.add_worktree(&format!("{id}-{}", &paths::id("try")[4..12]), &repo.head()?)?;
    let project = candidate.join(&relative);
    provider::copy_ignored_context(root, &project)?;
    let mut op = Operation {
        id: id.clone(),
        kind: "archive".into(),
        plan_hash: expected.into(),
        expected_head: repo.head()?,
        origin_branch: repo.branch()?,
        candidate: candidate.to_string_lossy().into(),
        final_head: None,
        status: "prepared".into(),
        result: json!({"operation_id":id,"plan":plan,"candidate":candidate,"branch":branch}),
    };
    paths::atomic_write(&record, serde_json::to_vec_pretty(&op)?)?;
    let result = (|| -> Result<()> {
        verify_configured(&project, &config, &repo, &id)?;
        let sources = plan["sources"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![plan.clone()]);
        let mut native_results = vec![];
        for source in &sources {
            native_results.push(archive_source(root, &project, framework, &config, source)?);
        }
        op.result["native_results"] = json!(native_results);
        if plan["kind"] == "milestone-archive" {
            let source = paths::inside(&project, text(&plan, "source")?)?;
            let destination = paths::inside(&project, text(&plan, "destination")?)?;
            fs::create_dir_all(destination.parent().unwrap())?;
            fs::rename(source, destination)?;
        }
        paths::atomic_write(
            &paths::inside(&project, &format!(".spec-autonomous/archives/{id}.toml"))?,
            toml::to_string_pretty(&plan)?,
        )?;
        op.final_head = Some(git::commit(
            &candidate,
            &format!("sa: archive native source\n\nSpec-Autonomous-Operation: {id}"),
        )?);
        op.status = "verified".into();
        paths::atomic_write(&record, serde_json::to_vec_pretty(&op)?)?;
        Ok(())
    })();
    if let Err(error) = result {
        op.status = "needs_repair".into();
        op.result["error"] = json!(error.to_string());
        paths::atomic_write(&record, serde_json::to_vec_pretty(&op)?)?;
        return Err(error);
    }
    testpoint("after_archive_candidate");
    finalize(&repo, root, op)
}
pub(super) fn verify_configured(
    root: &Path,
    config: &Config,
    repo: &Repository,
    key: &str,
) -> Result<Value> {
    let mut evidence = vec![];
    for (index, check) in config.verification.iter().enumerate() {
        crate::config::validate_check(check)?;
        let before = git::head(root)?;
        let dir = paths::inside(
            &repo.common,
            &format!("spec-autonomous/operations/{key}-checks/{index}"),
        )?;
        let output = process::execute(process::Request {
            argv: &check.argv,
            cwd: &paths::inside(root, &check.cwd)?,
            env: &config.environment,
            stdin: None,
            directory: &dir,
            timeout: Duration::from_secs(config.execution.attempt_timeout_seconds),
            max_log_bytes: config.execution.max_log_bytes,
            cancel: Arc::new(AtomicBool::new(false)),
        })?;
        if output.code != 0 || git::head(root)? != before || !git::clean(root)? {
            bail!(
                "verification_failed: configured lifecycle check failed or modified files; evidence retained at {}",
                dir.display()
            );
        }
        evidence.push(json!({"revision":before,"exit_code":output.code,"log":dir}));
    }
    Ok(json!(evidence))
}
pub fn doctor(root: &Path) -> Result<Value> {
    let detection = crate::detect(root, None)?;
    let mut diagnostics = vec![];
    let config = Config::load(root);
    if let Err(error) = &config {
        diagnostics.push(error.to_string());
    }
    if let Ok(config) = &config {
        if config.runner.profile != "disabled" {
            diagnostics.push(
                "legacy_runner_ignored: migrate configuration; agent launching is not supported"
                    .into(),
            );
        }
    }
    let inventory = crate::progress::snapshot(root).unwrap_or_else(|e| {
        diagnostics.push(e.to_string());
        json!({"consistency":"unavailable"})
    });
    let repo = Repository::discover(root).ok();
    let mut operations = vec![];
    if let Some(repo) = repo {
        let dir = paths::inside(&repo.common, "spec-autonomous/operations")?;
        if dir.is_dir() {
            for file in fs::read_dir(dir)? {
                let file = file?;
                if file.file_type()?.is_symlink() {
                    diagnostics.push("operation_invalid: symlink ignored".into());
                    continue;
                }
                if file.path().extension().is_some_and(|e| e == "json") {
                    match serde_json::from_slice::<Operation>(&fs::read(file.path())?){Ok(op)=>operations.push(json!({"operation_id":op.id,"status":op.status,"candidate":op.candidate})),Err(_)=>diagnostics.push("operation_corrupt: explicit inspection required".into())}
                }
            }
        }
    }
    Ok(
        json!({"schema_version":1,"starts_agents":false,"frameworks":detection,"config_valid":config.is_ok(),"host":config.ok().map(|c|c.host),"inventory":inventory,"operations":operations,"diagnostics":diagnostics}),
    )
}
pub fn repair(root: &Path, args: &Value) -> Result<Value> {
    let kind = text(args, "kind")?;
    let repo = Repository::discover(root)?;
    let target = match kind {
        "legacy-config" => paths::inside(root, ".spec-autonomous/config.toml")?,
        "registry" => paths::inside(&repo.common, "spec-autonomous/registry.toml")?,
        "abandon-operation" => operation_path(&repo, text(args, "operation_id")?)?,
        _ => bail!(
            "repair_unsupported: supported repairs are legacy-config, registry, abandon-operation"
        ),
    };
    let bytes = if target.exists() {
        fs::read(&target)?
    } else {
        vec![]
    };
    let digest = paths::hash(&bytes);
    if args["apply"] != true {
        return Ok(
            json!({"kind":kind,"target":target,"expected_hash":digest,"preserves_backup":true,"requires_idle_project":true}),
        );
    }
    if text(args, "expected_hash")? != digest {
        bail!("source_drift: repair preview changed");
    }
    let _lock = Lease::acquire(&repo)?;
    guard_project_idle(root)?;
    let current = if target.exists() {
        fs::read(&target)?
    } else {
        vec![]
    };
    if paths::hash(&current) != digest {
        bail!("source_drift: concurrent repair");
    }
    match kind {
        "legacy-config" => {
            let mut config = Config::load(root)?;
            config.runner = crate::config::Runner::default();
            config.schema_version = 2;
            config.validate()?;
            paths::atomic_write(
                &paths::inside(
                    &repo.common,
                    &format!("spec-autonomous/backups/config-{digest}.toml"),
                )?,
                &bytes,
            )?;
            paths::atomic_write(&target, toml::to_string_pretty(&config)?)?;
        }
        "registry" => {
            let store =
                Store::open(&repo, false)?.context("state_unavailable: no ledger to index")?;
            store.list()?;
            paths::atomic_write(
                &paths::inside(
                    &repo.common,
                    &format!("spec-autonomous/backups/registry-{digest}.toml"),
                )?,
                &bytes,
            )?;
            drop(store);
            drop(Store::open(&repo, true)?);
        }
        "abandon-operation" => {
            let mut op: Operation = serde_json::from_slice(&bytes)?;
            if op.final_head.is_some() || op.status == "completed" {
                bail!("repair_unsupported: a verified operation must be finalized, not abandoned");
            }
            if repo.head()? != op.expected_head {
                bail!("source_drift: origin changed since operation");
            }
            op.status = "abandoned".into();
            paths::atomic_write(
                &paths::inside(
                    &repo.common,
                    &format!("spec-autonomous/backups/{}-{digest}.json", op.id),
                )?,
                &bytes,
            )?;
            fs::remove_file(&target)?;
        }
        _ => unreachable!(),
    }
    Ok(json!({"kind":kind,"repaired":true,"backup_preserved":true,"target":target}))
}

fn testpoint(name: &str) {
    #[cfg(debug_assertions)]
    if std::env::var("SPEC_AUTONOMOUS_TEST_FAILPOINT").as_deref() == Ok(name) {
        std::process::exit(86);
    }
    #[cfg(not(debug_assertions))]
    let _ = name;
}
