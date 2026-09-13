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
