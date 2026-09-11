## Purpose

Provide observable and verifiable run recovery behavior for Spec Autonomous users while preserving clear execution and compatibility boundaries.

## ADDED Requirements

### Requirement: Persist execution progress
The system SHALL durably record milestone selection, policy, source revisions, task attempts, evidence and integration intentions so that process interruption does not erase progress.

#### Scenario: Supervisor interruption
- **WHEN** the supervisor exits unexpectedly during a run
- **THEN** a later resume reads the same run and reconciles its persisted state before dispatch

### Requirement: Prevent duplicate coordinators
The system SHALL allow only one writing coordinator per managed Git repository, including linked worktrees.

#### Scenario: Second run starts from another worktree
- **WHEN** a coordinator already owns the repository lock
- **THEN** the second process reports the active run and does not dispatch competing writers

### Requirement: Reconcile non-atomic side effects
The system SHALL reconcile recorded intentions with actual Git history, file revisions and worker identities before retrying interrupted operations.

#### Scenario: Crash after Git integration
- **WHEN** a commit is integrated but the completion transaction did not finish
- **THEN** resume recognizes the actual integration and does not apply it twice

#### Scenario: Hook outcome is unknown after interruption
- **WHEN** a lifecycle hook may have performed a side effect but its outcome cannot be reconciled
- **THEN** the run reports hook_outcome_unknown instead of blindly retrying or marking the hook complete

### Requirement: Retain policy and distinguish unknown usage
The system SHALL preserve the effective run policy on resume and SHALL report unavailable cost or token measurements as unknown rather than zero.

#### Scenario: Resume an authorized run
- **WHEN** the existing run is resumed without policy changes
- **THEN** routine authorized work continues without repeated authorization and unavailable usage remains explicitly unknown

### Requirement: Report every worktree in the repository
The progress CLI SHALL enumerate all worktrees sharing the selected Git common directory and associate available milestone, phase, run, task and worker metadata, including externally created worktrees.

#### Scenario: Query from a worker worktree
- **WHEN** progress is invoked from one concurrent worker's directory
- **THEN** the result still lists the main checkout, other managed worktrees and external worktrees from that repository

#### Scenario: Missing runtime metadata
- **WHEN** a worktree exists in Git but no readable runtime metadata is available
- **THEN** it remains visible with external, unknown or unavailable diagnostics rather than being omitted or reported complete

### Requirement: Provide consistent read-only structured progress
The CLI SHALL offer human, JSON and TOML progress snapshots with a versioned schema, timestamp and consistency diagnostics, deduplicate logical task totals and distinguish native planning progress from verified execution progress.

#### Scenario: Concurrent inventory changes
- **WHEN** a worktree disappears or a worker heartbeat becomes stale during a query
- **THEN** the response marks affected observations partial or stale without blocking execution, changing source files or silently reporting zero progress

#### Scenario: Same task appears in several worktrees
- **WHEN** a task has an original checkout entry and multiple attempts
- **THEN** aggregate task completion counts that logical task once while worker and retry counts remain separate

### Requirement: Resume phase and mode boundaries
The system SHALL persist roadmap revision, stable range endpoints, native workflow stage and handoff checkout so resumption does not widen scope or recreate already valid artifacts.

#### Scenario: Resume after native edits
- **WHEN** the user continues work through native commands and then resumes autonomous execution
- **THEN** the system reconciles the new source revision, preserves valid progress, invalidates affected evidence and retains the selected phase boundaries
