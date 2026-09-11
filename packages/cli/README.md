# spec-autonomous

Plan a milestone with OpenSpec or Spec Kit, then let a lightweight native supervisor
drive its implementation, verification, repair, and integration using fresh-context agents.

**Current version is the bootstrap preview:** only read-only `detect`, `--help`, and
`--version` are implemented. Autonomous execution is specified and not yet implemented.
No registry release has been made as part of repository initialization.

After a registry release:

```sh
npm install -g spec-autonomous@next
spec-autonomous detect --json
spec-autonomous detect --path /path/to/repo --framework openspec
```

Node.js 22+ is required. End users do not need Rust or Bun when installing a
published package with its native optional dependency. npm optional dependencies
must remain enabled. There is no postinstall download script.

Target matrix: macOS arm64/x64, Linux glibc arm64/x64, Windows arm64/x64.
At bootstrap only macOS arm64 was run locally. Other targets require CI validation
before publication. Linux musl is explicitly unsupported.

For local source development, run `bun run build` then
`node packages/cli/bin/spec-autonomous.mjs detect --json` from the repository root.
`bun run pack:local` produces a tarball containing the current host's native binary;
that local tarball is only for the matching platform. The release assembler produces
separate native packages for the full matrix.
