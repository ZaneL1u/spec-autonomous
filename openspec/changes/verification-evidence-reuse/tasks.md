## 1. Evidence reuse
- [x] 1.1 Add `git::tree`, `HostState.verification_cache`, `execution.verification_reuse` and tree/class/command keyed PASS reuse in `verify()` with lazy preparation; record `reused_from` evidence and `verification_reused` events; skip post-hook when hooks did not move accepted_head. Verify with contract tests counting real executions per stage.

## 2. Recovery without resets
- [x] 2.1 Replace `completed_phases.clear()`/`plans.clear()` in reconcile and handed_off recovery with ancestry shortcut, three-dot change classification, non-cascading `reopen` and `stale_phases` re-verification before delivery. Verify with contract tests for fast-forward, foreign commit and native source change.

## 3. Observability and docs
- [x] 3.1 Add `executed_checks`/`reused_checks` to progress overhead; update docs/long-running-runs.md and config docs. Verify with progress contract test and `bun run test:all`.
