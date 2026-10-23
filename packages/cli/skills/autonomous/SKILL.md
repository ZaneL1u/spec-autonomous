---
name: autonomous
description: Advance a milestone or phase range through OpenSpec or Spec Kit using prepared work packets and the host's existing fresh-context agents.
---

<objective>
Drive the selected milestone or phase range of an existing OpenSpec / Spec Kit project to verified delivery. Spec Autonomous (this product is Spec Autonomous, not OpenGSD) owns planning, verification, integration, the ledger and recovery; the host owns fresh agent contexts and dispatch. The full protocol is docs/host-protocol.md in the Spec Autonomous repository.
</objective>

<entry_points>
MCP: sa_doctor, sa_inspect, sa_prepare, sa_apply_result, sa_next, sa_progress; sa_tools reaches granular calls (work.claim, work.claim-batch, work.heartbeat, work.revoke, request.get, run.revise, run.cleanup, discussion.*). CLI: `spec-autonomous doctor|inspect|prepare|apply-result|next|progress` and `spec-autonomous tools call <capability>`. Both are the same service. Never assemble Git/file/state command chains and never read `.git/spec-autonomous/state.db`; a missing field is a protocol gap to report, not a reason to bypass the protocol.
</entry_points>

<loop>
1. Once per session: `doctor`, then `inspect`. Setup errors (missing provider tools, invalid config, budget_cap) are blockers; resolve them with `sa_providers` / `spec-autonomous providers status|ensure` or configuration, never by starting agents. Do not repeat doctor or inspect inside the loop; prepare is not a health check.
2. Start with `prepare`: the user's `goal` (optional `id`, `mode`), or existing `milestone_id` / `change` / `feature`, carrying from/to/only unchanged. Always pass `host: {host_id, session_id, fresh_context: true}`. Every returned packet is already claimed for this host under a derived session and carries its `token` and `owner`; explicit `work.claim` / `work.claim-batch` with distinct fresh sessions remain available when the host prefers to claim itself.
3. The host allocates each fresh context: for each packet in `work[]`, start a fresh agent context and hand it only the packet (`input_path`, `prompt_path`, `result_schema_path`, its project/worktree). The child follows the installed native SDD contract and returns the WorkerResult; it never edits shared checkboxes or the ledger. Run independent packets concurrently within the declared host capacity.
4. Submit each result with `apply-result`, using the packet's `owner` as `host`, its `token` and `claim_next: true`. The response is the next work, already claimed. Repeat until `completed` / `scope_completed` or an actionable blocker. Do not poll with prepare; call `next` only when nothing is in hand.
5. `scope_completed` covers the selected range only; native checkboxes and verified completion stay distinct. Archive is a separate explicit capability within the user's scope.
</loop>

<rules>
- Completion is decided by the CLI receipt and ledger, never by the agent session's exit. A child that ended after its result was submitted is done; one that ended before it has no result: inspect `request.get`, then resubmit or redispatch the same packet.
- Never steer a working child. `stale` on a packet is information; `work.heartbeat` is only needed when a packet's `limits` demand it. When a child's session is gone, `work.revoke` with `host_stopped: true` and this `host` (no token needed), then continue.
- Lost context: `next` or `request.get` with this `host` return the tokens this host owns.
- Gray areas: prepare returns `needs_input` with `discussion` cards. Present `recommended` first with `why` and `impact`, apply clicks with `discussion.apply`, or pass `discussion_auto: true` when the user accepts the defaults. Never answer a card by editing native Markdown.
- Wrong check command: `run.revise` with `reason`, the task/phase/milestone checks and `apply: true`; a preview plus `plan_hash` is only required while work on that scope is outstanding. Never weaken a check to make it green, never commit to the integration branch by hand, never cancel a recoverable run over one command string.
- `verification_preflight_failed`: prepare with the same `run_id` issues a corrected planner packet. `repair_repeated` or any `needs_input`: stop and report; the same failure twice is an operator decision, not a retry.
- Source edits made while work is outstanding are merged after the receipts land; nothing needs revoking for them.
- At a terminal result: `run.cleanup` with `apply: true`; add `delete_branches: true` only within the user's scope after reviewing the preview and its `plan_hash`. Report retained dirty, locked or unknown resources honestly; never force-delete them.
- Preserve the user's framework, run ID, scope and budgets. Native mode hands the framework workflow back after roadmap planning.
</rules>
