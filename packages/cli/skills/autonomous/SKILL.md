---
name: autonomous
description: Advance a milestone or phase range through OpenSpec or Spec Kit using prepared work packets and the host's existing fresh-context agents.
---

Use the Spec Autonomous capabilities. Prefer MCP `sa_inspect`, `sa_next`, `sa_prepare`, and `sa_apply_result`; `sa_tools` discovers granular tools. CLI equivalents are fixed public entry points (`spec-autonomous prepare`, `next`, `apply-result`, `tools call`). Do not assemble Git/file/state command chains: the capabilities already perform those operations.

The npm entry point prepares missing native tools automatically. Use `sa_providers` with `operation: status` for installation diagnostics, or `operation: ensure` with the selected provider to retry setup; CLI equivalents are `spec-autonomous providers status|ensure`. For an empty repository, preserve the user's choice and run `spec-autonomous init --provider openspec|speckit --agent codex|claude`. Native tool commands remain available through `spec-autonomous providers exec <provider> -- <arguments>` even when the managed tool is absent from the host's PATH. Offline or failed installation is a setup blocker, not completed work.

A goal uses prepare with `goal`, optional `id`, and `mode: autonomous`; existing work uses `milestone_id`, `change`, or `feature`. Carry the user's `from`/`to`/`only` range unchanged. After the first response, use its run ID for continuation. Native mode hands back the existing framework workflow after roadmap planning.

When prepare returns `awaiting_host`, the work packets describe what the host must do. For each issued request:

1. Have the host allocate a unique fresh work context and claim the request through `work.claim`, using the supplied token and a host-assigned session identity. Do not reuse a parent conversation as a fresh child.
2. Dispatch through the host's existing agent facility. Pass only the packet and its assigned project/worktree. The child reads its complete context references, follows the installed native SDD contract, and returns the specified WorkerResult. The child does not update shared checkboxes or the ledger.
3. Submit the returned JSON through apply-result with the same ownership identity. The CLI verifies, integrates and returns the next eligible work; repeat until a terminal state or an actionable blocker.

If the host cannot provide fresh contexts/worktree access, report that capability gap; do not substitute an embedded CLI runner. Execute independent packets concurrently only within the declared host capacity. Send work.heartbeat during long host work. Do not dispatch claimed/stale requests again: inspect the host session, then work.revoke only after confirming it stopped. Receiving/submitted receipts need recovery, not a new agent.

Use progress for bounded updates. `scope_completed` is only the selected phase range; native checkboxes and verified completion are distinct. Archive is a separate explicit capability with preview and hash-checked apply, following the user's scope. Preserve the user's native SDD framework and specifications.

Before dispatch, inspect verification_readiness. Deferred future files are not failed tests; warnings about Node directory operands or missing runtime tools need a concrete command choice. Use environment information and native requirements to correct planning expectations. Prefer a coherent implementation plus its tests in one execution task with multiple source_ids where native ordering permits; retain genuinely independent work.

For multiple issued packets, allocate distinct fresh host session identities and use work.claim-batch with run_id and requests[{request_id,token,host}]. The whole batch is validated before any claim. Dispatch the claimed packets through the host, submit receipts as they finish, and continue using returned work rather than repeatedly listing the same run.

A planning candidate rejected by verification_preflight_failed is corrected through prepare with the same run_id, which issues a fresh planner with diagnostics. Use run.revise only for checks already accepted into the active run.

If a verifier's argv/cwd/fixture expectation is wrong, inspect state.get and the native requirement before proposing a correction. Use run.revise with run_id, reason and task_checks[{phase_id,task_id,checks:[{argv,cwd}]}], phase_checks[{phase_id,checks}], or milestone_checks. First preview, review its before/after diff, then apply the identical payload plus apply:true and plan_hash. Resolve/revoke outstanding work only after the host confirms it stopped. Continue prepare with the same run_id. This retains accepted work and historical evidence; the corrected checks still must pass. Do not use prepare(run_id,plan), manually commit to the integration branch, drop checks just to make them green, or cancel a recoverable run because one verification string is wrong. Overrides are run-local; update the baseline plan through the native workflow when the run is idle if later runs should use the correction.

At a terminal result, inspect run.cleanup. Apply its current plan_hash within the user's cleanup scope; optionally request delete_branches:true for safely merged owned refs. Stop/revoke outstanding sessions first. Report retained dirty/locked/unknown resources and integration/evidence separately; never force-delete them to obtain an empty inventory.

Before prepare for a phase, call `discussion.next` (or `spec-autonomous tools call discussion.next`). Render each returned card as a one-click choice: the `recommended` option first, then `alternatives`, with `why` and `impact` visible. Apply clicks with `discussion.apply` using the returned source_hash. Use `auto:true` only when the host/user explicitly chooses the recommended defaults; unresolved cards are reported and remain visible. Never silently invent a choice or edit native Markdown to answer a card. The generated phase CONTEXT and DISCUSSION-LOG are derived decision records that the next planning packet may read.

Autonomous mode checks Smart Discuss automatically before each phase. Empty cards mean there is no user-owned gray area: display decisions and continue. Cards with `requires_user` pause the phase until the host applies a click selection.
