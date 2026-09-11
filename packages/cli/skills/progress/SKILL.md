---
name: progress
description: Read milestone and host work progress across every Git worktree through a compact structured capability, without changing execution state.
---

Use MCP sa_progress or `spec-autonomous progress --all-worktrees --json`. The capability resolves the shared Git repository and joins native, runtime and worktree facts. It does not launch agents or require command-chain construction.

Progress does not require installing the native tool. Use sa_providers with operation status (or `spec-autonomous providers status --json`) only when installation diagnostics are relevant; a progress request alone does not authorize ensure or framework initialization.

Report verified progress separately from native checkbox counts. Keep external, unknown, stale, partial and host-reported states visible. A host claim/heartbeat is not a CLI observation of an agent process. Count logical tasks once across retries and preserve scope_completed versus full milestone completion.

Use pagination metadata when more worktrees/runs exist than the first page. Query sa_next for next-action hints, and use sa_tools state.get/history.get/audit.open only for relevant details. Do not ingest full worker logs unless resolving a concrete issue. This skill never cleans, repairs, archives or re-dispatches work implicitly.
