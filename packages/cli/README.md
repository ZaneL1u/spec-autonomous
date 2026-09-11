# Spec Autonomous

Native deterministic capabilities for LLM hosts and Skills using OpenSpec or Spec Kit. **The CLI never launches agents or calls models.** The host owns fresh-context sessions; the CLI prepares work, verifies receipts, manages isolated worktrees and safely updates native state.

This is the local `0.1.0-alpha.4` development release. No npm registry publication has been performed.

The command interface uses Commander.js 14.0.3 and Rust Clap. Native command definitions are exported from Clap, and the same native parser checks arguments before provider installation or project writes. Commander owns provider subcommands, nested help and structured syntax errors. npm installs the pinned Commander dependency automatically; installed runtime still requires neither Rust nor Bun.

The JS wrapper selects the matching Rust executable and prepares missing native SDD tools. Installed users need Node.js 22+ and Git. Bun is supported but optional; missing uv / Python are prepared for Spec Kit in a user-owned directory. No npm lifecycle script downloads providers.

```sh
spec-autonomous init --agent codex --mcp
# Empty repository: explicitly choose the native framework
spec-autonomous init --provider openspec --agent codex --mcp
spec-autonomous init --provider speckit --agent codex --mcp
spec-autonomous providers status --json
spec-autonomous providers ensure speckit --json
spec-autonomous providers exec openspec -- --version
spec-autonomous help providers ensure
spec-autonomous inspect --json
spec-autonomous progress --all-worktrees --json
spec-autonomous prepare --milestone M001 --from 1 --to 3 --json
spec-autonomous next --run-id <run-id> --json
spec-autonomous tools list --all --limit 200 --json
```

Seven complete workflow capabilities are public by default: inspect, progress, prepare, next, apply-result, archive and doctor. Granular document/frontmatter, roadmap, task/work, state, history, Git, worktree, verification and repair tools share the same service and schemas. Run `spec-autonomous mcp` for stdio MCP; `sa_tools` discovers and invokes advanced capabilities. The npm transport adds `sa_providers` with structured status/ensure operations and automatically prepares dependencies before native operations. Progress, doctor and tool discovery do not download anything.

OpenSpec 1.13.0, Spec Kit 1.0.6 and uv 0.12.13 are the managed baselines. Existing usable project/PATH tools and explicit OpenSpec argv configuration are preserved. `--managed` on providers ensure selects an isolated installation instead. uv downloads are SHA256 verified. Concurrent installs share a lock and only validated generations receive a readiness record; failed installs can be retried. Set `SPEC_AUTONOMOUS_PROVIDER_HOME` to override the user data directory or `SPEC_AUTONOMOUS_OFFLINE=1` to forbid downloads. Managed native tools are available through providers exec; the installer does not edit global PATH, shell profiles or project package manifests.

Prepare returns immutable packets, worktree paths, ownership tokens and host constraints. The host claims each unit with a unique fresh session, executes it using its own agent facility and submits a structured WorkerResult through apply-result. Awaiting host work is not completion; checked native tasks and verified progress remain distinct.

Archive uses a source-bound preview and isolated Git candidate. The native OpenSpec archive operation or Spec Kit feature-directory archive runs before verified fast-forward delivery. Existing user changes and active work are protected.

Five Skills are included: autonomous, auto (exact alias), milestone, progress and resume. Installation binds them to the selected project and optionally adds a protected MCP entry; existing unrelated host configuration is preserved. Host trust rules remain in effect.

Official release packaging uses separate, exact-version platform packages referenced by optionalDependencies. Install with optional dependencies enabled. A locally packed tarball includes only its build platform and is not a universal release artifact. Windows/other-platform support requires the declared native CI matrix; a workflow file alone is not test evidence.
