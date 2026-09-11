---
name: auto
description: Exact alias of autonomous for host-driven OpenSpec or Spec Kit milestones and phase ranges.
---

Use exactly the autonomous capability protocol: inspect/next, prepare, then owned apply-result receipts. MCP entry points are sa_inspect, sa_next, sa_prepare and sa_apply_result; sa_tools exposes granular work.claim/heartbeat/revoke. CLI equivalents use the same service.

The npm CLI/MCP layer automatically prepares missing selected native tools. Use sa_providers status/ensure for setup diagnostics and retries, or `spec-autonomous providers status|ensure`. An empty repository requires the user's provider choice with `spec-autonomous init --provider openspec|speckit --agent codex|claude`. Use `spec-autonomous providers exec <provider> -- <arguments>` for native tools outside PATH. Setup failure or offline missing dependencies remain blockers.

The CLI never starts an agent. The host allocates each fresh context, claims the issued packet with a unique host session identity, uses its existing agent facility in the assigned worktree, and submits the returned WorkerResult. Do not assemble low-level Git/file/state commands. If host execution is unavailable, report the missing capability instead of configuring a runner.

Preserve the milestone, provider, from/to/only bounds, budgets and run ID. Do not redispatch claimed/stale work without host-stop acknowledgement. Resume receipt processing rather than repeating completed semantic work. Keep verified progress separate from native checkbox counts; scope_completed does not complete the whole milestone.
