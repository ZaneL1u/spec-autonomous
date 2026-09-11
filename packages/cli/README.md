# Spec Autonomous

Native deterministic capabilities for LLM hosts and Skills using OpenSpec or Spec Kit. **The CLI never launches agents or calls models.** The host owns fresh-context sessions; the CLI prepares work, verifies receipts, manages isolated worktrees and safely updates native state.

This is the local `0.1.0-alpha.2` development release. No npm registry publication has been performed.

The wrapper selects the matching Rust executable for the current OS/CPU (and Linux libc) and forwards argv, stdio and exit status. Installed users need Node.js 22+, Git and their existing native SDD tools, not Rust or Bun.

```sh
spec-autonomous init --agent codex --mcp
spec-autonomous inspect --json
spec-autonomous progress --all-worktrees --json
spec-autonomous prepare --milestone M001 --from 1 --to 3 --json
spec-autonomous next --run-id <run-id> --json
spec-autonomous tools list --all --limit 200 --json
```

Seven complete capabilities are public by default: inspect, progress, prepare, next, apply-result, archive and doctor. Granular document/frontmatter, roadmap, task/work, state, history, Git, worktree, verification and repair tools share the same service and schemas. Run `spec-autonomous mcp` for stdio MCP; `sa_tools` discovers and invokes advanced capabilities without shell command construction.

Prepare returns immutable packets, worktree paths, ownership tokens and host constraints. The host claims each unit with a unique fresh session, executes it using its own agent facility and submits a structured WorkerResult through apply-result. Awaiting host work is not completion; checked native tasks and verified progress remain distinct.

Archive uses a source-bound preview and isolated Git candidate. The native OpenSpec archive operation or Spec Kit feature-directory archive runs before verified fast-forward delivery. Existing user changes and active work are protected.

Five Skills are included: autonomous, auto (exact alias), milestone, progress and resume. Installation binds them to the selected project and optionally adds a protected MCP entry; existing unrelated host configuration is preserved. Host trust rules remain in effect.

Official release packaging uses separate, exact-version platform packages referenced by optionalDependencies. Install with optional dependencies enabled. A locally packed tarball includes only its build platform and is not a universal release artifact. Windows/other-platform support requires the declared native CI matrix; a workflow file alone is not test evidence.
