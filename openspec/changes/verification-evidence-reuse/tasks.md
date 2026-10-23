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
