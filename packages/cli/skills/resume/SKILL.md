---
name: resume
description: Resume a host-driven Spec Autonomous run while preserving its native SDD source, bounds, ownership and verified work.
---

<objective>
Continue an existing run (this product is Spec Autonomous, not OpenGSD) without widening its scope, resetting its progress or discarding owned work. The full protocol is docs/host-protocol.md in the Spec Autonomous repository.
</objective>

<steps>
1. Once per session: `doctor` and `inspect` (MCP sa_doctor / sa_inspect; sa_progress for a bounded overview). Doctor reports setup, ledger version, budget warnings and stale work. Do not repeat them inside the loop.
2. `next` (sa_next) with `run_id` and this `host` shows the outstanding packets and returns the tokens this host owns; `request.get` gives one packet's status, owner, heartbeat age and paths. Decide per packet: a live child keeps running; a child that is gone is revoked with `work.revoke` (`host_stopped: true`, this `host`, a reason); a child that already produced a result gets that same result submitted with `apply-result`.
3. `prepare` (sa_prepare) with `run_id` and this `host`. The capability reconciles receipts and Git intents, keeps completed phases and plans, merges original-checkout changes once nothing is in flight, and returns the remaining work already claimed. Explicit budget or config changes use prepare's `reload_config`, `extend_seconds` and `max_attempts`.
4. Dispatch and `apply-result` (sa_apply_result, `claim_next: true`) as in the autonomous skill until a terminal result, then `run.cleanup` with `apply: true` within the user's scope.
</steps>

<rules>
- Legacy runs are read-only and never reactivate their former runner; import their native scope into a new host-driven run.
- `hook_outcome_unknown` needs `hook.resolve` with explicit outcome evidence. Never force-reset dirty worktrees.
- `verification_preflight_failed` is corrected by `prepare --run-id <id>`; an accepted but wrong check is corrected with `run.revise` (`apply: true`; preview and `plan_hash` only while that scope has outstanding work). Do not use prepare with a replacement plan, drop checks to make them green, or cancel a recoverable run over one command string.
- Stopped runs continue with `prepare --run-id <id>` (MCP: `prepare` with only `run_id`); never add `--change` / `--feature`, which are mutually exclusive with `--run-id`. `verification_environment_unavailable`: the plan is kept and nothing is replanned; fix the environment (install the missing executable, create the directory, or correct `runner.environment` / the verification config) and `prepare --run-id <id>` reruns only the preflight, adding `reload_config: true` after a config change. `worker_prepare_failed`: no packet was issued; fix the `[hooks.worker_prepare]` hook or what it needs, then `prepare --run-id <id>` (with `reload_config: true` after a config change). A `warnings[]` entry starting with `config_drift:` means the project configuration changed after the run started; the run keeps its frozen config until a `prepare --run-id <id>` with `reload_config: true`, which you pass only when the user wants the new config (for example a restored hook) to apply.
- `repair_repeated`, `budget_cap` and other `needs_input` results are reported to the user, not retried. Never read state.db directly; `work.heartbeat` is only needed when a packet's limits ask for it.
</rules>
