# dogfood-rough-edges Specification

## Purpose
TBD - created by archiving change dogfood-rough-edges. Update Purpose after archive.
## Requirements
### Requirement: Explain audit contract differences
The audit contract error SHALL identify missing, unexpected, duplicate and evidence-less requirement IDs when validation fails.

#### Scenario: Extra audit item
- **WHEN** an audit includes an ID absent from metadata.acceptance
- **THEN** the error names that unexpected ID and any missing IDs.

### Requirement: Explain cleanup scope conflicts
Cleanup SHALL include the requested deletion scope in stale plan hash errors and identify the need to preview the same scope again.

#### Scenario: Scope changed
- **WHEN** a preview without branch or integration deletion is applied with either option enabled
- **THEN** the error identifies the changed scope and instructs a same-scope preview.

### Requirement: Guide cancelled run recovery
Source drift caused by outstanding host work SHALL identify the run and outstanding request IDs with the revoke action needed before retrying.

#### Scenario: Issued packet after cancellation
- **WHEN** a new source baseline is blocked by an issued packet
- **THEN** the error lists the request and instructs the host to stop and revoke it.

### Requirement: Dogfood the autonomous workflow
The repository SHALL include a runnable OpenSpec playground that initializes, plans, executes host receipts, reports progress and previews cleanup in an isolated copy.

#### Scenario: Local dogfood
- **WHEN** `node scripts/dogfood.mjs` runs
- **THEN** it reports a completed run without modifying the checked-in playground.
