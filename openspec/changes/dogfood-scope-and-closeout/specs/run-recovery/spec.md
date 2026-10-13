## ADDED Requirements

### Requirement: Supersede replaced submitted receipts
When a later attempt for the same kind, task and phase is `accepted` or `integrated`, the system SHALL mark earlier `submitted` attempts for that work unit `superseded`, retain their receipts and evidence, and SHALL NOT re-apply those receipts. `issued` and `claimed` attempts SHALL remain outstanding until the host confirms stop through `work.revoke`. Resume SHALL apply the same reconciliation to existing ledgers before preparing new work.

#### Scenario: Retry succeeds after an unprocessed roadmap receipt
- **WHEN** a later roadmap attempt is accepted while an earlier roadmap attempt is still `submitted`
- **THEN** the earlier attempt becomes `superseded` and does not block further phases or new runs as outstanding host work

#### Scenario: Resume an existing ledger with leftover submitted work
- **WHEN** prepare or resume loads a run that already has an accepted later attempt for the same work unit
- **THEN** leftover `submitted` attempts for that unit are superseded before any new packet is issued

#### Scenario: Live issued work after host exit
- **WHEN** an attempt remains `issued` or `claimed` after the host session ends
- **THEN** the CLI reports it stale or outstanding and does not auto-revoke or spawn a replacement
