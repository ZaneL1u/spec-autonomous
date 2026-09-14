---
name: resume
description: Resume a host-driven Spec Autonomous run while preserving its native SDD source, bounds, ownership and verified work.
---

Read sa_next and sa_progress for the run, then use sa_prepare with run_id. The capability reconciles persisted receipts and Git intents before preparing further work. Retain the original scope and policy; explicit budget/config changes use prepare's resume options.

The npm layer prepares missing native tools before continuation, including on a new machine. Use sa_providers status/ensure or `spec-autonomous providers status|ensure` to diagnose or retry installation. Preserve the run's provider; never initialize another framework to bypass a failed dependency install.

The host owns agent sessions. Do not restart claimed or stale packets: inspect the host's session state and acknowledge stopped work through work.revoke before requesting replacement work. Use work.heartbeat for live sessions. A receiving receipt needs the same result resubmitted; completed receipts are idempotent. Use sa_apply_result for new owned results, with the same host session identity.

Legacy runs are read-only and never reactivate their former runner. Inspect their native artifacts and import the desired scope into a new host-driven run. Unknown native hook effects require hook.resolve with explicit outcome evidence. Do not force-reset dirty worktrees or silently widen the selected phase range.

Before dispatch, inspect verification_readiness. Deferred future files are not failed tests; warnings about Node directory operands or missing runtime tools need a concrete command choice. Use environment information and native requirements to correct planning expectations. Prefer a coherent implementation plus its tests in one execution task with multiple source_ids where native ordering permits; retain genuinely independent work.

For multiple issued packets, allocate distinct fresh host session identities and use work.claim-batch with run_id and requests[{request_id,token,host}]. The whole batch is validated before any claim. Dispatch the claimed packets through the host, submit receipts as they finish, and continue using returned work rather than repeatedly listing the same run.

A planning candidate rejected by verification_preflight_failed is corrected through prepare with the same run_id, which issues a fresh planner with diagnostics. Use run.revise only for checks already accepted into the active run.

If a verifier's argv/cwd/fixture expectation is wrong, inspect state.get and the native requirement before proposing a correction. Use run.revise with run_id, reason and task_checks[{phase_id,task_id,checks:[{argv,cwd}]}], phase_checks[{phase_id,checks}], or milestone_checks. First preview, review its before/after diff, then apply the identical payload plus apply:true and plan_hash. Resolve/revoke outstanding work only after the host confirms it stopped. Continue prepare with the same run_id. This retains accepted work and historical evidence; the corrected checks still must pass. Do not use prepare(run_id,plan), manually commit to the integration branch, drop checks just to make them green, or cancel a recoverable run because one verification string is wrong. Overrides are run-local; update the baseline plan through the native workflow when the run is idle if later runs should use the correction.

At a terminal result, inspect run.cleanup. Apply its current plan_hash within the user's cleanup scope; optionally request delete_branches:true for safely merged owned refs. Stop/revoke outstanding sessions first. Report retained dirty/locked/unknown resources and integration/evidence separately; never force-delete them to obtain an empty inventory.

Before prepare for a phase, call `discussion.next` (or `spec-autonomous tools call discussion.next`). Render each returned card as a one-click choice: the `recommended` option first, then `alternatives`, with `why` and `impact` visible. Apply clicks with `discussion.apply` using the returned source_hash. Use `auto:true` only when the host/user explicitly chooses the recommended defaults; unresolved cards are reported and remain visible. Never silently invent a choice or edit native Markdown to answer a card. The generated phase CONTEXT and DISCUSSION-LOG are derived decision records that the next planning packet may read.
