## Context

See proposal.md. The alpha.14 consumer remains active on its installed executable. Work is isolated in a separate clone. Local GSD execution uses plan/wave lifecycle, post-merge gates, spot-checks and phase verification; UAT uses structured coverage and gap linkage. It is not an excuse to trust an unverified worker claim.

## Goals / Non-Goals

Reduce duplicate process execution and retained directories. Preserve host-owned fresh contexts, CAS source completion and crash recovery. No live consumer writes, global upgrades, release or deletion of unknown evidence.

## Decisions

- Reuse closed clean plan-tasks/audit checkouts only. Validate every historical owner of the path, current Git registration, expected branch/head and absence of ignored files before a clean checkout move. Never reuse a writing worker for another semantic task.
- Integrate on the writing worker when its base is current. Otherwise allocate the existing merge candidate. Remove the redundant worker-only test wave; the complete candidate gate remains authoritative.
- Memoize completed verification waves per exact checkout/revision, stage, argv/cwd, config and inherited environment fingerprint, resolved executable bytes and intact manifests/logs. Persist a completed receipt only after every check passes. Preparation stays part of the signature. Expose an execution switch for stateful/external checks and force post-hook execution when hooks really ran. No reuse across checkouts, commits or changed inputs.
- Reopen only changed declared scopes and their dependents. Empty/unknown scopes, unclassified paths, hooks and global config fail closed to the existing full re-review behavior.
- Run conservative cleanup at the coordinator handoff under its existing writer lease. Reuse existing cleanup planning and Git rechecks, preserve branches and runtime evidence. Retention is not a hard resource cap; report blocked reclamation honestly. A separate opt-out preserves forensic checkouts.
- Deliver the project script correction as an unapplied patch. Only hash prior completed harness-owned output directories; do not hash live coordinator diagnostics.

## Risks / Trade-offs

External service state is not represented by Git or process environment; disable verification reuse for such suites. Dirty or ignored-file checkouts may remain above the expected budget. Scope declarations must be conservative; unknown changes reopen all phases. Read-only is a checked contract, not an OS sandbox.

## Migration Plan

Build and verify this clone only. The active consumer finishes with its old executable and state. Review commits and apply a later explicitly chosen upgrade at a quiescent handoff; do not reconnect its MCP in this task. Disable reuse/automatic maintenance for forensic or externally stateful runs.
