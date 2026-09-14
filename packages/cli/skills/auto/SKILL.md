---
name: auto
description: Exact alias of autonomous for host-driven OpenSpec or Spec Kit milestones and phase ranges.
---

Use exactly the autonomous capability protocol: inspect/next, prepare, then owned apply-result receipts. MCP entry points are sa_inspect, sa_next, sa_prepare and sa_apply_result; sa_tools exposes granular work.claim/heartbeat/revoke. CLI equivalents use the same service.

The npm CLI/MCP layer automatically prepares missing selected native tools. Use sa_providers status/ensure for setup diagnostics and retries, or `spec-autonomous providers status|ensure`. An empty repository requires the user's provider choice with `spec-autonomous init --provider openspec|speckit --agent codex|claude`. Use `spec-autonomous providers exec <provider> -- <arguments>` for native tools outside PATH. Setup failure or offline missing dependencies remain blockers.

The host allocates each fresh context, claims the issued packet with a unique host session identity, uses its existing agent facility in the assigned worktree, and submits the returned WorkerResult. Do not assemble low-level Git/file/state commands. If host execution is unavailable, report the missing capability instead of configuring a runner.

Preserve the milestone, provider, from/to/only bounds, budgets and run ID. Do not redispatch claimed/stale work without host-stop acknowledgement. Resume receipt processing rather than repeating completed semantic work. Keep verified progress separate from native checkbox counts; scope_completed does not complete the whole milestone.

Before dispatch, inspect verification_readiness. Deferred future files are not failed tests; warnings about Node directory operands or missing runtime tools need a concrete command choice. Use environment information and native requirements to correct planning expectations. Prefer a coherent implementation plus its tests in one execution task with multiple source_ids where native ordering permits; retain genuinely independent work.

For multiple issued packets, allocate distinct fresh host session identities and use work.claim-batch with run_id and requests[{request_id,token,host}]. The whole batch is validated before any claim. Dispatch the claimed packets through the host, submit receipts as they finish, and continue using returned work rather than repeatedly listing the same run.

A planning candidate rejected by verification_preflight_failed is corrected through prepare with the same run_id, which issues a fresh planner with diagnostics. Use run.revise only for checks already accepted into the active run.

If a verifier's argv/cwd/fixture expectation is wrong, inspect state.get and the native requirement before proposing a correction. Use run.revise with run_id, reason and task_checks[{phase_id,task_id,checks:[{argv,cwd}]}], phase_checks[{phase_id,checks}], or milestone_checks. First preview, review its before/after diff, then apply the identical payload plus apply:true and plan_hash. Resolve/revoke outstanding work only after the host confirms it stopped. Continue prepare with the same run_id. This retains accepted work and historical evidence; the corrected checks still must pass. Do not use prepare(run_id,plan), manually commit to the integration branch, drop checks just to make them green, or cancel a recoverable run because one verification string is wrong. Overrides are run-local; update the baseline plan through the native workflow when the run is idle if later runs should use the correction.

At a terminal result, inspect run.cleanup. Apply its current plan_hash within the user's cleanup scope; optionally request delete_branches:true for safely merged owned refs. Stop/revoke outstanding sessions first. Report retained dirty/locked/unknown resources and integration/evidence separately; never force-delete them to obtain an empty inventory.

Before prepare for a phase, call `discussion.next` (or `spec-autonomous tools call discussion.next`). Render each returned card as a one-click choice: the `recommended` option first, then `alternatives`, with `why` and `impact` visible. Apply clicks with `discussion.apply` using the returned source_hash. Use `auto:true` only when the host/user explicitly chooses the recommended defaults; unresolved cards are reported and remain visible. Never silently invent a choice or edit native Markdown to answer a card. The generated phase CONTEXT and DISCUSSION-LOG are derived decision records that the next planning packet may read.
