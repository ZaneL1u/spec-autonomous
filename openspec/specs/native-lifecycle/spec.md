# native-lifecycle Specification

## Purpose
Provide predictable native lifecycle capabilities for existing SDD hosts while preserving source authority and observable execution boundaries.
## Requirements
### Requirement: Preview and verify native archives
Archives SHALL provide a dry-run plan bound to source revisions and SHALL preserve the provider native document structure and completion gates.

#### Scenario: Archive changed since preview
- **WHEN** a source changes after its archive preview
- **THEN** execution refuses without moving or deleting native files

### Requirement: Protect concurrent work during lifecycle changes
Archive and worktree removal SHALL refuse active, dirty, locked or ambiguous affected work unless a specifically supported safe resolution is supplied.

#### Scenario: Remove active worktree
- **WHEN** a worktree belongs to an outstanding host request
- **THEN** removal is refused and evidence remains

### Requirement: Explicit health and repair
Doctor SHALL inspect without mutation; repair SHALL expose a bounded plan and perform only supported repairs after source revision checks.

#### Scenario: Unreadable state
- **WHEN** the runtime cannot be parsed
- **THEN** doctor reports partial diagnostics without silently rebuilding or deleting the ledger
