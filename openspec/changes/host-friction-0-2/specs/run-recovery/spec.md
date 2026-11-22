## ADDED Requirements

### Requirement: Refuse older CLIs after the schema 5 upgrade
The ledger SHALL use schema version 5 with the same tables as schema 4. The first writable open of a schema 4 ledger SHALL back it up before stamping version 5 in one transaction without rewriting any run, attempt or evidence row, and older schemas SHALL be backed up and migrated as before. Read-only opens and pause or cancel control writes SHALL NOT migrate. After migration, CLIs that only understand schema 4 or older SHALL be refused with `schema_unsupported`, and JSON/MCP fields, status values and error codes SHALL remain unchanged.

#### Scenario: Upgrade a schema 4 ledger
- **WHEN** the new CLI first writes to a ledger at schema 4 that holds attempts and evidence
- **THEN** a backup is written, the version becomes 5 and every attempt and evidence row is still readable

#### Scenario: Older CLI meets the upgraded ledger
- **WHEN** an alpha.16 CLI opens the ledger after the upgrade
- **THEN** it reports schema_unsupported instead of reading or writing run payloads
