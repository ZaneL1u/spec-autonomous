## Purpose

Provide predictable host work protocol capabilities for existing SDD hosts while preserving source authority and observable execution boundaries.

## ADDED Requirements

### Requirement: CLI never launches an agent
Every public operation SHALL avoid starting an agent executable or model session; semantic work SHALL be returned to the calling host.

#### Scenario: Prepare with a trap runner
- **WHEN** a configured legacy runner executable would create a marker file
- **THEN** prepare returns a work request and the marker does not exist

### Requirement: Durable host work requests
Requests SHALL preserve immutable context, unique identities, worktree isolation, ownership tokens and host session declarations across calls.

#### Scenario: Repeat prepare
- **WHEN** a request remains outstanding
- **THEN** the same request is returned without allocating a duplicate worktree

### Requirement: Idempotent verified result application
Applying results SHALL validate request identity and hashes, independently verify changed code and integrate only accepted revisions; identical receipt replay SHALL be idempotent.

#### Scenario: Replay a completed receipt
- **WHEN** the same result and ownership token are submitted twice
- **THEN** the second response identifies a replay and no task or source update occurs twice

### Requirement: Bounded native progression
The passive workflow SHALL preserve native planning, full task coverage, phase ranges, conservative parallelism, audits and bounded repair across host round trips.

#### Scenario: Host drives two phases
- **WHEN** the host answers each native planning, implementation and audit request
- **THEN** the selected scope reaches verified delivery without any CLI agent launch

### Requirement: Explicit host cancellation
Cancellation and stale leases SHALL report host actions and SHALL not assume an external agent is stopped.

#### Scenario: Stale host heartbeat
- **WHEN** a host request has no recent heartbeat
- **THEN** progress reports stale and no duplicate worker is automatically allocated

