## 1. Write-set vs context files

- [x] 1.1 Add `plan::worker_write_forbidden` and cover write-set/context intersection, empty writes, and `.spec-autonomous/` in `plan_contracts`
- [x] 1.2 Use that helper in `integrate_task`; add a host-protocol or unit case where an assigned context file is modified and accepted

## 2. Receipt closeout

- [x] 2.1 Supersede earlier `submitted` attempts when a later same-unit attempt is accepted or integrated
- [x] 2.2 Reconcile existing leftover submitted receipts in `recover` before issuing new work
- [x] 2.3 Process an unreplaced submitted receipt in `invoke_host` even when `request_key` differs; verify with a host-protocol contract

## 3. Host protocol diagnostics

- [x] 3.1 Allow optional `host` on `work.heartbeat` and reject other unknown fields
- [x] 3.2 Accept optional `usage` on apply-result, persist it on the attempt, and aggregate it in progress
- [x] 3.3 Extend doctor with skills version drift, ledger `user_version`, and stale outstanding request IDs; add contracts

## 4. Skills and verification

- [x] 4.1 Update autonomous, auto and resume skills: not OpenGSD, doctor/inspect before prepare, heartbeat (host optional), session closeout, run.revise instead of cancel
- [x] 4.2 Run `bun run test:all` after the stated unit/contract/e2e checks for this change pass, then mark remaining boxes
