# spec-autonomous

A native CLI and skill entry points for autonomous development over your existing
OpenSpec or Spec Kit workflow. TOML describes orchestration; native Markdown remains
the specification source. Workers use fresh agent sessions and separate Git worktrees.

This is the local `0.1.0-alpha.1` development release. No registry publication has
been performed. After publication:

```sh
npm install -g spec-autonomous@next
spec-autonomous init --agent codex
spec-autonomous doctor --json
spec-autonomous progress --all-worktrees
```

Init binds `$autonomous`/`$auto` for Codex or `/autonomous`/`/auto` for a supported
slash-command host (`--agent claude`). Existing user-owned skills are preserved.
Configure your existing agent launcher and meaningful verification argv in
`.spec-autonomous/config.toml`, and commit the initialized project before execution.

```sh
spec-autonomous milestone new "My milestone goal" --mode autonomous
spec-autonomous run --milestone M001 --from 2 --to 4 --mode autonomous
spec-autonomous pause <run-id>
spec-autonomous resume <run-id>
```

Node.js 22+ and Git are required. Published native optional dependencies remove the
need for Rust/Bun at runtime. The Codex profile additionally needs your installed,
authenticated Codex CLI; other launchers must implement the documented command
worker contract. OpenSpec planning requires its CLI; Spec Kit native planning uses
the target project's existing skills/templates and their original dependencies.

No postinstall downloads or automatic project modification. Optional dependencies
must remain enabled. A locally packed tarball embeds only its build host's binary;
release assembly creates separate native platform packages. Linux musl is explicitly
unsupported. Only macOS arm64 was executed locally; other platform claims require CI.
