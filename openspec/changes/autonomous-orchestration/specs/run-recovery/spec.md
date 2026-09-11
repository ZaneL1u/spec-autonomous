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
