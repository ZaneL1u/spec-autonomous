use super::*;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    id: String,
    name: String,
    path: String,
    branch: String,
    base: String,
    status: String,
}
fn index(repo: &Repository) -> Result<PathBuf> {
    paths::inside(&repo.common, "spec-autonomous/tool-worktrees.json")
}
fn read(repo: &Repository) -> Result<Vec<Entry>> {
    let path = index(repo)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn save(repo: &Repository, entries: &[Entry]) -> Result<()> {
    paths::atomic_write(&index(repo)?, serde_json::to_vec_pretty(entries)?)?;
    Ok(())
}
fn revision(root: &Path, value: &str) -> Result<String> {
    if value.starts_with('-') || value.contains('\0') {
        bail!("invalid_revision");
    }
    Ok(git::command(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{value}^{{commit}}"),
        ],
        None,
    )?
    .trim()
    .into())
}
pub fn invoke(root: &Path, name: &str, args: &Value) -> Result<Value> {
    let repo = Repository::discover(root)?;
    if name == "worktree.list" {
        let entries = read(&repo)?;
        return Ok(
            json!({"worktrees":repo.worktrees()?.iter().map(|w|json!({"id":paths::hash(&w.path),"path":w.path,"branch":w.branch,"head":w.head,"locked":w.locked,"prunable":w.prunable,"tool":entries.iter().find(|e|e.path==w.path).map(|e|json!({"id":e.id,"name":e.name,"status":e.status}))})).collect::<Vec<_>>()}),
        );
    }
    if name == "worktree.diff" {
        let target = text(args, "worktree")?;
        let inventory = repo.worktrees()?;
        let wt = inventory
            .iter()
            .find(|w| w.path == target || paths::hash(&w.path) == target || w.branch == target)
            .context("worktree_not_found")?;
        let base = revision(Path::new(&wt.path), args["base"].as_str().unwrap_or("HEAD"))?;
        return Ok(
            json!({"worktree":wt,"base":base,"changed_files":git::changed(Path::new(&wt.path),&base)?,"stat":git::command(Path::new(&wt.path),&["diff","--no-ext-diff","--no-textconv","--stat",&base,"--"],None)?,"dirty":!git::clean(Path::new(&wt.path))?}),
        );
    }
    let _lease = Lease::acquire(&repo)?;
    let mut entries = read(&repo)?;
    if name == "worktree.create" {
        let label = text(args, "name")?;
        paths::valid_id(label)?;
        if entries
            .iter()
            .any(|e| e.name == label && e.status != "removed")
        {
            bail!("worktree_exists: a managed workspace already uses this name");
        }
        let base = revision(root, args["base"].as_str().unwrap_or("HEAD"))?;
        let id = paths::id("tool");
        let (path, branch) = repo.add_worktree(&id, &base)?;
        let entry = Entry {
            id,
            name: label.into(),
            path: path.to_string_lossy().into(),
            branch,
            base,
            status: "created".into(),
        };
        entries.push(entry.clone());
        save(&repo, &entries)?;
        return Ok(serde_json::to_value(entry)?);
    }
    let id = text(args, "worktree_id")?;
    let at=entries.iter().position(|e|e.id==id||e.name==id).context("worktree_not_owned: only tool-created workspaces can be mutated here; use run cleanup for completed workers")?;
    let entry = entries[at].clone();
    let path = PathBuf::from(&entry.path);
    let relative = paths::localize(&repo.common, &path)?;
    if !relative.starts_with("spec-autonomous/worktrees/")
        || !entry.branch.starts_with("codex/sa/tool-")
    {
        bail!("worktree_not_owned");
    }
    let inventory = repo.worktrees()?;
    let wt = inventory
        .iter()
        .find(|w| w.path == entry.path && w.branch == entry.branch)
        .context("worktree_not_found")?;
    if wt.locked || !git::clean(&path)? {
        bail!("worktree_protected: dirty or locked workspace is retained");
    }
    guard_project_idle(root)?;
    if name == "worktree.remove" {
        if git::head(&path)? != text(args, "expected_head")? {
            bail!("source_drift: worktree changed");
        }
        git::command(root, &["worktree", "remove", "--", &entry.path], None)?;
        entries[at].status = "removed".into();
        save(&repo, &entries)?;
        return Ok(json!({"removed":entry.path,"branch_retained":entry.branch}));
    }
    if name == "worktree.merge" {
        repo.preflight()?;
        let head = git::head(&path)?;
        if head != text(args, "expected_head")? || repo.head()? != text(args, "expected_target")? {
            bail!("source_drift: source or destination changed");
        }
        if !git::ancestor(root, &repo.head()?, &head) {
            bail!(
                "merge_conflict: only verified fast-forward merges are supported; branches retained"
            );
        }
        let checks = lifecycle::verify_configured(
            &path,
            &Config::load(root)?,
            &repo,
            &paths::id("merge-check"),
        )?;
        if repo.head()? != text(args, "expected_target")?
            || git::head(&path)? != head
            || !git::clean(&repo.root)?
        {
            bail!("source_drift: workspace changed during verification");
        }
        git::advance(&repo.root, &head)?;
        entries[at].status = "merged".into();
        save(&repo, &entries)?;
        return Ok(
            json!({"merged":entry.path,"accepted_head":head,"verification":checks,"branch_retained":true}),
        );
    }
    bail!("unknown_capability")
}
