# Repository guide

## Product intent

Spec Autonomous starts from a milestone goal or existing OpenSpec/Spec Kit artifacts,
creates a roadmap through native planning workflows, then drives a whole milestone or
from/to phase range to verified delivery. Native and autonomous modes share the same
Markdown specs. Skills call the CLI; TOML stores orchestration declarations. Progress
must cover all worktrees in the Git repository, including external and unknown ones.
The CLI never starts agents or model sessions. It exposes deterministic compound capabilities and granular tools; the host owns fresh contexts and dispatch.
The user-facing primary entry is /autonomous with /auto as an exact alias where the
host supports slash commands. It depends on the user's existing SDD provider; init
binds commands/skills and never silently replaces the framework or its workflows.
Fresh workers, task DAGs and isolation support this end-to-end product outcome.

## Start here

- Read README.md for the current implemented boundary.
- Read openspec/changes/host-driven-capabilities/{proposal,design,tasks}.md for the current implementation contract. The earlier autonomous-orchestration change and validation/autonomous.md are historical alpha.1 evidence.
- Use the installed OpenSpec CLI and the existing spec-driven schema for behavioral changes.
- Do not mark planned features complete because proposal/design/tasks exist.
- Update the relevant OpenSpec checkbox only after its stated verification passes.

## Structure and tools

- Rust owns passive workflow capabilities, work packets, native bridges and verified transitions; Node is the thin npm launcher. Only the independent tests/mock-host.mjs fixture dispatches semantic test workers.
- Bun 1.4.2 owns workspace dependencies and scripts; commit bun.lock and Cargo.lock.
- Source ~/.cargo/env if cargo is not on PATH. Use the pinned rust-toolchain.toml.
- Verify with bun run test:all (equivalent: node scripts/test-all.mjs). The suite includes lint, unit/contracts, real OpenSpec, npm assets, Git/process e2e and strict specs.
- Check npm packaging with bun run pack:local. Only claim platforms actually tested.
- .references contains locked upstream research copies. Do not modify them or ship them.
- Never check secrets, runtime logs, native build outputs or node_modules into Git.

## Design invariants

- Existing specs retain requirement authority; the execution ledger owns attempts and evidence.
- No worker receives the entire coordinator history or writes shared task checklists.
- Unknown write sets serialize. New processes alone do not imply a filesystem sandbox.
- Exit zero, checked tasks, validation and integrated code are distinct states.
- Recovery reconciles durable intent and actual Git state; it never blindly reruns side effects.
