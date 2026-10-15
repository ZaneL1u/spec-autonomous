# Runtime overhead validation

Date: 2026-10-16, Asia/Shanghai. Platform: macOS arm64. Node v26.5.0; Bun 1.4.2; repository-pinned Rust 1.98.1. All repositories used for execution tests are disposable local fixtures. No real agent/model, business database, payment, global installation, live MCP replacement or consumer ledger mutation.

## Deterministic before/after experiment

Same two-phase/three-task Spec Kit fixture, host capacity 1, real Git and Node assertions. Semantic mock workers and native workflow templates are unchanged.

| Metric | Baseline | Candidate |
| --- | ---: | ---: |
| Verified implementation tasks | 3 | 3 |
| Semantic attempts | 14 | 14 |
| Verification command evidence records | 15 | 7 |
| Reused complete verification waves | 0 | 3 |
| Attempt checkout paths | 14 | 10 |
| Worktrees remaining at completion | 19 | 2 |

The candidate's intent records point at the writing workers in this serial scenario; they do not allocate additional checkout paths. The remaining two directories are the primary checkout and integration checkout. Branches and immutable runtime receipts/logs remain. This is not a benchmark of LLM speed and does not predict money-printer wall-clock savings.

Reproduce from this checkout:

```sh
node scripts/benchmark-overhead.mts --baseline /absolute/path/to/old/spec-autonomous --candidate /absolute/path/to/new/spec-autonomous
```

## Verification records

- Targeted real-Git contracts cover read-only reuse, dirty/ignored/locked retention, exact candidate integration, same-input audit waits, terminal reclamation, preserved branches/receipts/logs and native planning recovery.
- Verification contracts cover preparation state shared across checks, consumer artifacts isolated from active diagnostics, source mutation, complete receipt identity, evidence tampering, failure/timeout and missing executable diagnostics.
- Review-scope contracts cover known read/write matches, transitive dependents, unknown files, missing declarations and global configuration fallback.
- An additional progress contract confirms backlog warning at fewer than 100 checkouts and reports a single-worker budget of 7.
- Consumer patch applied and Python-compiled only in a temporary fixture; its actual helper regression checks live log churn, completed history preservation and evidence tampering. The live project was not patched.

## Completed local checks

- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed on final source, including supplemental progress contract.
- `cargo test --workspace --locked`: 193 passing Rust tests before the additional progress test; the final progress suite independently passed 18/18 (one added regression). Optional real-provider/process checks remain explicitly ignored in the ordinary Rust run; the complete driver runs the selected real OpenSpec provider contract separately.
- TypeScript typecheck: passed, including the benchmark tool.
- Consumer patch temporary application/compile and executed helper regression: passed.
- `node scripts/pack-local.mts`: built a macOS arm64 release tarball. Installed with scripts disabled into a temporary prefix; version/help passed; temporary installation deleted.
- Package: `.artifacts/local/spec-autonomous-0.1.0-alpha.14.tgz`, SHA256 `78b07eee1b897adc92fd4cbc33a60df83094a3d98cf611a266730ea2403491ab`.
- Release-binary fixture timing was 8144ms baseline and 8121ms candidate in one local run. The tiny checks are cheap and semantic attempt count is unchanged, so this is effectively equal wall time. Reported gains are directory retention and repeated command counts; no claim of a universal speed multiplier.

## Complete driver

`node scripts/test-all.mts` passed: typecheck, committed launcher build, rustfmt, clippy, workspace Rust contracts, selected real OpenSpec provider contract, 58 JS/package tests, 83 Git/process/host/MCP end-to-end tests (zero failures/skips), and strict validation of all main specs and active changes. The 83 e2e tests include six crash boundaries, concurrent integration, same-run check revisions, dependency gates, source drift, manual/automatic resource cleanup, optional hooks, process-tree cancellation and time budgets.

The extra progress warning test was added after the complete driver's Rust stage and verified separately with the complete 18-test progress suite plus clippy. The benchmark's final timing fields were typechecked separately. These test/tooling additions do not alter the packaged runtime.

Earlier development runs exposed an abandoned-intent recovery issue after combined verification failure and two manual-cleanup fixtures that needed explicit automatic_cleanup=false. Both were corrected; their original assertions remain. The final full run has no failures.


