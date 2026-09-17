# capability-tools Specification

## Purpose
Provide predictable capability tools capabilities for existing SDD hosts while preserving source authority and observable execution boundaries.
## Requirements
### Requirement: Versioned discoverable capabilities
Complete operations and granular tools SHALL share a versioned registry with argument schemas and explicit mutability.

#### Scenario: List capabilities
- **WHEN** a host requests the capability catalog
- **THEN** it receives stable IDs, schemas and read/write classification

### Requirement: Bounded structured queries
Queries SHALL provide JSON/TOML and compact projections with provenance, limits, pagination and explicit unknown or partial states.

#### Scenario: Inspect large progress
- **WHEN** a repository has more worktrees than the requested page limit
- **THEN** the response includes a bounded page and continuation information without discarding aggregate progress

### Requirement: Atomic structured document updates
Granular document writes SHALL require an expected source hash and preserve unrelated Markdown content and unknown metadata.

#### Scenario: Stale frontmatter patch
- **WHEN** the document changed since inspection
- **THEN** the write is rejected without modifying bytes

### Requirement: Shared roadmap task and state operations
Granular roadmap, task, state and history operations SHALL use the same source authority, dependency and verification rules as complete operations.

#### Scenario: Premature task completion
- **WHEN** a host attempts to complete an unverified native task through a granular tool
- **THEN** the operation refuses without changing its checkbox
