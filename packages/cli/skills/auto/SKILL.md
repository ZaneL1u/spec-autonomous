---
name: auto
description: Exact alias of autonomous for host-driven OpenSpec or Spec Kit milestones and phase ranges.
---

<objective>
`auto` is the autonomous skill under its short name; this product is Spec Autonomous, not OpenGSD. Follow the autonomous protocol exactly: one `doctor` + `inspect` per session, then `prepare` with `host`, dispatch the already-claimed packets to fresh agent contexts, and `apply-result` each receipt with the packet's `owner`, `token` and `claim_next: true` until a terminal result. The full protocol is docs/host-protocol.md in the Spec Autonomous repository.
</objective>

<entry_points>
MCP: sa_prepare, sa_apply_result, sa_next, sa_progress, sa_doctor, sa_inspect; sa_tools for work.claim, work.claim-batch, work.heartbeat, work.revoke, request.get, run.revise, run.cleanup and discussion.*. The host allocates each fresh context and dispatches through its own agent facility; packets returned by prepare are already claimed, explicit work.claim / work.claim-batch stay available. CLI: `spec-autonomous prepare|apply-result|next` and `spec-autonomous tools call <capability>`. Missing native tools are prepared by the npm layer (`sa_providers` / `spec-autonomous providers status|ensure`); an empty repository needs the user's provider choice through `spec-autonomous init --provider openspec|speckit --agent codex|claude`.
</entry_points>

<rules>
- Preserve the milestone, provider, from/to/only bounds, budgets and run ID. Do not redispatch owned work without `work.revoke` with `host_stopped: true` after the host confirms the session is gone; `stale` alone is information and `work.heartbeat` is optional.
- Completion comes from the CLI receipt, not from the agent session. Resume receipt processing instead of repeating completed semantic work; `scope_completed` does not complete the whole milestone.
- `auto` checks Smart Discuss automatically: prepare returns `needs_input` with `discussion` cards when a phase has gray areas. Show `recommended` first, then alternatives, and continue after `discussion.apply`; with the user's consent pass `discussion_auto: true` to accept the recommended defaults in the same call. Empty cards mean no user-owned gray area.
- A wrong check is corrected with `run.revise` (`apply: true`; preview and `plan_hash` only while that scope has outstanding work). `repair_repeated` and other `needs_input` results are reported, not retried.
- At a terminal result run `run.cleanup` with `apply: true` and report what was retained. Never force-delete, never read state.db directly, never assemble Git/file/state command chains.
</rules>
