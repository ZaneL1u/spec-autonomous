use super::*;
pub fn invoke(root: &Path, name: &str, args: &Value) -> Result<Value> {
    let repo = Repository::discover(root)?;
    if name == "git.inspect" {
        return Ok(
            json!({"root":repo.root,"head":repo.head()?,"branch":repo.branch()?,"dirty":!git::clean(root)?,"status":git::command(root,&["status","--porcelain=v1"],None)?}),
        );
    }
    guard_project_idle(root)?;
    let _lease = Lease::acquire(&repo)?;
    guard_project_idle(root)?;
    if repo.head()? != text(args, "expected_head")? {
        bail!("source_drift: Git HEAD changed");
    }
    let files = args["files"].as_array().context("invalid_files")?;
    if files.is_empty() {
        bail!("invalid_files: explicit paths required");
    }
    let mut paths_ = vec![];
    for file in files {
        let relative = file.as_str().context("invalid_files")?;
        paths::inside(root, relative)?;
        if Path::new(relative)
            .components()
            .any(|p| p.as_os_str().to_string_lossy().eq_ignore_ascii_case(".git"))
        {
            bail!("path_outside_scope");
        }
        paths_.push(relative);
    }
    let mut add = vec!["add", "--all", "--"];
    add.extend(&paths_);
    git::command(root, &add, None)?;
    if repo.head()? != text(args, "expected_head")? {
        bail!("source_drift: HEAD changed while staging; user index preserved");
    }
    let message = text(args, "message")?;
    let mut commit = vec![
        "-c",
        "user.name=Spec Autonomous",
        "-c",
        "user.email=spec-autonomous@localhost",
        "commit",
        "--only",
        "-m",
        message,
        "--",
    ];
    commit.extend(&paths_);
    git::command(root, &commit, None)?;
    Ok(json!({"head":repo.head()?,"files":files,"unrelated_staged_changes_preserved":true}))
}
