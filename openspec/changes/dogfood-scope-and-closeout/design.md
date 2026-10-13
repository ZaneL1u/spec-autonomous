## Context

Dogfood in `test-spec-autonomous` and `code-wrapper` used the host-driven protocol correctly (fresh claim, isolated worktree, apply-result) but two engine rules blocked completion: `integrate_task` treated every `snapshot.context_files` path as immutable even when `task.writes` named it, and `invoke_host` keyed outstanding submitted receipts only by `request_key`, so a retried roadmap left the first receipt `submitted`. Heartbeat schema rejected `host`, progress hard-coded `usage: unavailable`, and doctor did not mention skill/CLI drift.

## Goals / Non-Goals

**Goals:**

- Allow declared write-set edits to context files.
- Reconcile leftover submitted receipts when a later success exists.
- Keep heartbeat usable when hosts copy claim arguments.
- Surface drift and stale work in doctor; record optional usage.
- Teach skills to close out, and not confuse this product with OpenGSD.

**Non-Goals:**

- Auto-revoke issued/claimed work without `host_stopped`.
- Rewrite JSON/MCP required fields or WorkerResult schema.
- Mutate user project worktrees from this change.
- Force-upgrade SQLite user_version 2 ledgers that have no verification revisions.

## Decisions

1. Extract `plan::worker_write_forbidden` and use it from `integrate_task`. Empty writes keep current serialization: anything except context files and `.spec-autonomous/` may change.
2. Add `Coordinator::supersede_replaced_submissions` and call it on accept/integrate and at the start of `recover`. Matching key is `(kind, task_id, phase_id)`.
3. `invoke_host` first supersedes replaced submissions; if a submitted receipt still exists with no later success, process that receipt before issuing a new key.
4. Heartbeat schema adds optional `host` with the same object shape as claim. Other extra keys remain invalid.
5. Optional `usage` on apply-result is stored on `Attempt.usage`. Progress sums numeric `input_tokens`/`output_tokens`/`cached_input_tokens` when any attempt has usage.
6. Doctor reads `CARGO_PKG_VERSION`, skills manifest, `PRAGMA user_version`, and stale leases from progress inventory. Diagnostics are strings with stable prefixes.
7. Skills: first paragraph is not OpenGSD; `doctor`/`inspect` for readiness; session end inspects outstanding work, heartbeats live sessions, revokes only after stop, then `run.cleanup` on terminal runs.

## Risks / Trade-offs

- [Superseding skips a valid first receipt] → only when a later attempt for the same unit is already accepted/integrated; the first receipt stays on disk.
- [Optional host on heartbeat hides typos] → only `host` is ignored; other unknown fields still fail.
- [Usage is host-declared] → never infer dollars; missing usage stays `unavailable`.

## Migration Plan

Existing runs pick up reconciliation on the next prepare/resume. Users reinstall skills (`init` or skill install) to receive text updates. Old CLI writers still cannot write schema 3 ledgers.

## Open Questions

- None for this change. Leftover issued work remains a host closeout duty by design.
