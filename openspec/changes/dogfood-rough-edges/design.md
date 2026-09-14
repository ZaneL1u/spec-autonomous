## Decisions

1. Audit validation computes sorted set differences and duplicate counts while preserving the stable `audit_contract_error` code. The message contains missing, unexpected, duplicate and no-evidence IDs with bounded lists.
2. Cleanup preview hashes the complete requested scope, including `delete_branches` and `delete_integration`. A stale apply error reports the requested scope, expected hash, current hash and a command to preview the same scope.
3. Source reconciliation reports the run ID and every outstanding request ID/status, followed by the exact `work.revoke` action needed. It remains conservative: it never auto-revokes host work.
4. Integration cleanup is opt-in (`delete_integration`) and only applies to a terminal run when the integration worktree is clean, at the expected head, not current/checked out/shared, and its branch is safely merged. The ledger, evidence and run record remain. Default cleanup behavior is unchanged.
5. `playground/` is a normal checked-in OpenSpec repository fixture with a script that creates an isolated copy, runs provider initialization, writes a small todo spec, drives prepare/claim/receipt/apply-result and runs cleanup preview. It uses the independent mock host only for semantic worker receipts and never changes the source playground.
