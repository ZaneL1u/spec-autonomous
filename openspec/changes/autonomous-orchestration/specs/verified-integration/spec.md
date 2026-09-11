## Purpose

Provide observable and verifiable verified integration behavior for Spec Autonomous users while preserving clear execution and compatibility boundaries.

## ADDED Requirements

### Requirement: Validate actual code
The system SHALL use actual changes and host-run verification evidence tied to code revisions, rather than worker success claims, to decide integration eligibility.

#### Scenario: Worker claims success but tests fail
- **WHEN** the result claims completion while required host verification fails
- **THEN** the task is not completed and dependent work is not unlocked

### Requirement: Verify the combined revision
The system SHALL serialize integration and validate the combined code revision before treating dependency work as integrated.

#### Scenario: Individually passing patches conflict
- **WHEN** two individually verified results fail together after integration
- **THEN** the system retains evidence and schedules bounded repair instead of accepting both as complete

#### Scenario: Independent work continues during candidate failure
- **WHEN** a candidate integration fails and another independent task is eligible
- **THEN** that task starts from the last accepted revision and cannot consume the failing candidate as verified input

### Requirement: Protect user changes during delivery
The system SHALL deliver according to the selected policy and SHALL not overwrite an original checkout that changed since the recorded starting revision.

#### Scenario: Original checkout changes before final delivery
- **WHEN** the user edits files or moves its branch during a run with fast-forward delivery
- **THEN** the system keeps the verified integration branch, reports delivery pending, and preserves user changes

### Requirement: Provide an auditable milestone report
The system SHALL report source scope, completion state, output revision, verification evidence, retries, blockers and unperformed validations.

#### Scenario: A platform was not tested
- **WHEN** completion evidence includes only the supported local platform
- **THEN** the report identifies unperformed platform checks instead of claiming cross-platform validation

### Requirement: Distinguish range completion from milestone completion
The system SHALL report selected-scope completion separately from whole-milestone completion, identify remaining phases and a usable next action, and never derive whole-milestone success solely from completed tasks in one worktree.

#### Scenario: A single phase completes
- **WHEN** an only-phase run finishes its validation and checkpoint delivery
- **THEN** it reports scope_completed and does not automatically audit, archive or clean up the entire milestone
