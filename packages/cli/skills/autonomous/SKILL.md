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
- Task needs files outside its write set (for example existing tests it breaks): the child returns `blocked` with `requested_writes`; the CLI adds safe literal file paths and re-issues the task itself; directories and globs always need review. When it stops instead, review the blocker and grant the paths with `run.revise` `task_writes: [{phase_id, task_id, add}]` and `apply: true` (the blocker carries the exact input), then `prepare --run-id <id>`. Never edit the source branch to widen a scope.
- Evidence only a real environment can give (a deployment, service, device, manual look): the child keeps `candidate` (audit items `passed: true` when the rest holds) and lists it in `deferred_evidence`. Treat the run's `deferred_evidence` list as follow-ups to perform or report, not as failures; it stays after `completed` with a `deferred_evidence_pending` warning.
- `verification_preflight_failed`: `prepare --run-id <id>` issues a corrected planner packet. `repair_repeated` or any `needs_input`: stop and report; the same failure twice is an operator decision, not a retry.
- Stopped runs continue with `prepare --run-id <id>` (MCP: `prepare` with only `run_id`); never add `--change` / `--feature`, which are mutually exclusive with `--run-id`. `verification_environment_unavailable`: the plan is kept and nothing is replanned; fix the environment (install the missing executable, create the directory, or correct `runner.environment` / the verification config) and `prepare --run-id <id>` reruns only the preflight, adding `reload_config: true` after a config change. `worker_prepare_failed`: no packet was issued; fix the `[hooks.worker_prepare]` hook or what it needs, then `prepare --run-id <id>` (with `reload_config: true` after a config change). A `warnings[]` entry starting with `config_drift:` means the project configuration changed after the run started; the run keeps its frozen config until a `prepare --run-id <id>` with `reload_config: true`, which you pass only when the user wants the new config (for example a restored hook) to apply.
- Source edits made while work is outstanding are merged after the receipts land; nothing needs revoking for them.
- At a terminal result: `run.cleanup` with `apply: true`; add `delete_branches: true` only within the user's scope after reviewing the preview and its `plan_hash`. Report retained dirty, locked or unknown resources honestly; never force-delete them.
- A fresh context means a unique host session, not a new disk checkout: read-only planning and audit packets may reuse a closed, clean checkout, writing workers stay isolated. Use the returned project/worktree assignments as given.
- Preserve the user's framework, run ID, scope and budgets. Native mode hands the framework workflow back after roadmap planning.
</rules>

<economy>
Every packet is a fresh context that pays its own start-up (system prompt, tool schemas, skill list) and re-reads the code it needs, so spend contexts deliberately:
- Stay a thin router. Do not open `input.json`, `prompt.md` or patches yourself and do not summarise them into the child's brief; the child reads them. Hand over the paths plus at most a few lines of project facts the packet cannot know (a broken local tool, where a test database lives); write such facts once to a file and pass its path to every child instead of repeating them in each prompt.
- When `requirements.model` is present, start the child on that model (Claude Code: the Agent tool's `model`); otherwise use the host default. Prefer a worker agent type limited to shell and file tools, without MCP servers, browser or connector tools, when the host has one (for example a user-defined `spec-autonomous-worker` agent in Claude Code): tool schemas are re-sent on every turn of every child.
- Do not ask a child to rerun checks the CLI already ran, to re-plan, or to review other packets; one packet, one context. Never start extra helper agents around a packet.
- A failed attempt is retried by the CLI with the previous error and patch attached; do not redo it yourself or start a child outside the returned packets. `no_progress`, `attempts_exhausted` and `repair_repeated` mean stop and report, not "try again" — rerunning the same scope without changing the plan, checks or environment repeats the same spend.
- Keep `[execution] max_attempts` and `max_repair_rounds` at their defaults unless the user asks; raising them multiplies fresh contexts per task.
</economy>
