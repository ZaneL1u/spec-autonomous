## Context

See proposal.md. Existing source already contains uncommitted dogfood corrections, while consumers still run cached alpha.13 binaries. Tests use temporary repositories and a separate Cargo target directory.

## Goals / Non-Goals

Goals: bounded resource maintenance, reusable preparation contract, diagnostic receipts, progress discovery and safe handoff. Non-goals: live consumer migration, automatic database provisioning, test-result caching, deleting untracked evidence, publishing releases, or starting agents.

## Decisions

- Extend run.cleanup with explicit active scope; retain all accepted semantic attempts and nonterminal intents. Keep coordinator locking, branch CAS, preview hashing and dirty checks. Also retain resources referenced by outstanding immutable packets; favor retention when reference inspection fails. Active deletion never removes integration.
- Allocate a verify-wave directory under the common runtime. SA_VERIFY_OUTPUT_DIR, SA_VERIFY_WAVE_ID, SA_VERIFY_STAGE, SA_VERIFY_REVISION and SA_VERIFY_CHECKOUT are reserved environment variables. Existing idempotent hook declaration verification_prepare prepares a fresh environment for each wave; application-specific providers own database lifecycle. Each check gets its own diagnostic directory. No cached PASS results.
- Persist structured manifests beside existing bounded stdout/stderr logs. Errors reference manifests rather than embedding arbitrary output. Deduplicate combined argv/cwd exactly, without merging semantically different commands.
- Add optional progress repository_paths input and CLI flags, with at most sixteen explicitly chosen repositories and no recursive discovery. Native milestone reads are bounded and do not install providers.
- Additive evidence fields are optional for old ledgers. No change to Check schema or automatic revisions. Review run-local revision handling and preserve its behavior across replanning.
- Compare installed skill bytes with bundled source, not just version labels.

## Risks / Trade-offs

- Unknown references or ignored artifacts may hold evidence → retain with explicit reasons, never force-delete.
- External fixtures cannot be generically reset safely → provide isolated wave identities and preparation contract; require explicit consumer adoption.
- Existing long suites still take time → report actual cost and avoid duplicate checks, never weaken acceptance.

## Migration Plan

Build and test a local package without global installation. Existing commands finish on their original executable. At a coordinator-free boundary, save checkpoint and SQLite backup, pin the new executable, run read-only doctor/progress, refresh skills with ownership checks, then continue the same run. New wave variables work immediately; consumers explicitly revise fixed report paths/hooks as needed. Rollback uses the preserved executable and original config; do not downgrade a newer ledger writer blindly.
