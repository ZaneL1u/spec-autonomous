# Repository guide

## Product intent

Spec Autonomous drives a whole preplanned milestone to verified implementation using
existing OpenSpec or Spec Kit artifacts. The autonomous loop is the product; task DAGs,
fresh workers, worktrees and context reduction support that outcome.

## Start here

- Read README.md for the current implemented boundary.
- Read openspec/changes/autonomous-orchestration/{proposal,design,tasks}.md for pending work.
- Use the installed OpenSpec CLI and the existing spec-driven schema for behavioral changes.
- Do not mark planned features complete because proposal/design/tasks exist.
- Update the relevant OpenSpec checkbox only after its stated verification passes.

## Structure and tools

- Rust owns detection and future orchestration; Node is the thin npm launcher.
- Bun 1.4.2 owns workspace dependencies and scripts; commit bun.lock and Cargo.lock.
- Source ~/.cargo/env if cargo is not on PATH. Use the pinned rust-toolchain.toml.
- Verify with bun run check, bun run test and OPENSPEC_TELEMETRY=0 bun run spec:validate.
- Check npm packaging with bun run pack:local. Only claim platforms actually tested.
- .references contains locked upstream research copies. Do not modify them or ship them.
- Never check secrets, runtime logs, native build outputs or node_modules into Git.

## Design invariants

- Existing specs retain requirement authority; the execution ledger owns attempts and evidence.
- No worker receives the entire coordinator history or writes shared task checklists.
- Unknown write sets serialize. New processes alone do not imply a filesystem sandbox.
- Exit zero, checked tasks, validation and integrated code are distinct states.
- Recovery reconciles durable intent and actual Git state; it never blindly reruns side effects.
