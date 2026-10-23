## 1. Evidence reuse
- [x] 1.1 Add `git::tree`, `HostState.verification_cache`, `execution.verification_reuse` and tree/class/command keyed PASS reuse in `verify()` with lazy preparation; record `reused_from` evidence and `verification_reused` events; skip post-hook when hooks did not move accepted_head. Verify with contract tests counting real executions per stage.

## 2. Recovery without resets
- [x] 2.1 Replace `completed_phases.clear()`/`plans.clear()` in reconcile and handed_off recovery with ancestry shortcut, three-dot change classification, non-cascading `reopen` and `stale_phases` re-verification before delivery. Verify with contract tests for fast-forward, foreign commit and native source change.

## 3. Observability and docs
- [x] 3.1 Add `executed_checks`/`reused_checks` to progress overhead; update docs/long-running-runs.md and config docs. Verify with progress contract test and `bun run test:all`.

## 4. In-flight work and plans
- [x] 4.1 Defer source reconciliation while attempts are in flight (`reconcile_pending`), extend accepted plans for appended native tasks instead of replanning, stop `converge` from removing plans, and let `run.revise` block only on work in the revised scope. Verify with contract tests for deferred drift, plan extension and unrelated outstanding work.

## 5. Audit reuse and repeated failures
- [x] 5.1 Cache fully passed audits by phase/scope/tree/acceptance ids and stop on a repeated failure signature with `needs_input: repair_repeated`. Verify with contract tests for cache hits and an identical second phase-gate failure, then `bun run test:all`.

## 6. Stage scopes and planning guardrails
- [x] 6.1 Add `task_verification` / `phase_verification` / `milestone_verification`, reject inline-encoded or oversized argv, defer future scripts and executables in preflight, enrich the planner environment and failure history, and cap budgets with `unsafe_budgets` plus doctor warnings. Verify with contract tests for stage routing, rejection, deferral, environment facts, failure history and caps, then `bun run test:all`.

## 7. Host protocol ergonomics
- [x] 7.1 Auto-claim prepared work for the calling host, recover tokens through `next`/`request.get`, revoke by owning host, bounded lock wait, informational staleness, readiness only where needed, one-step `run.revise`/`run.cleanup`, discussion cards with prepare, and routing-style skills with docs/host-protocol.md. Verify with contract tests for claiming, recovery, revocation, lock wait and one-step apply, the skill contract test, then `bun run test:all`.

## 8. Resources and ledger
- [x] 8.1 Reclaim closed attempt and candidate worktrees automatically after storing their patch, make retry fingerprints read the stored hash, drop the ignored-output retention for closed worktrees, and add `reclaim_worktrees` / `retain_failed_worktrees`. Verify with contract tests for reclamation, retention of live work and the disable switch.
- [x] 8.2 Split the ledger into run core, attempt rows and evidence rows (schema 4) with migration and backup, add read-only `run.forensics` and `host_capacity`. Verify with contract tests for migration, incremental rows, forensics and capacity, then `bun run test:all`.
