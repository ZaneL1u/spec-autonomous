## Purpose

Bound the resources consumed by a long host-driven milestone while preserving independent verification, immutable receipts, fresh semantic contexts and recoverable integration evidence.

## ADDED Requirements

### Requirement: Bounded read-only checkouts
The system SHALL reuse a managed checkout for read-only planning and audit only after all previous owners have completed and its state is clean, owned, unlocked and free of ignored artifacts. Each request MUST retain a distinct host session and immutable packet. Unsafe checkouts MUST be retained.

#### Scenario: Sequential reviews
- **WHEN** multiple planning and audit requests execute serially
- **THEN** their closed clean checkout is reused while receipt identities remain distinct

#### Scenario: Live or dirty owner
- **WHEN** a prior request is outstanding or its checkout contains unpreserved files
- **THEN** that checkout is not moved, overwritten or deleted

### Requirement: One authoritative candidate verification
The system SHALL verify each task on the actual integration candidate, including required prior checks. An unchanged baseline MUST permit using the completed worker as the candidate without an additional checkout. Changed baselines MUST retain isolated candidate merging.

#### Scenario: Serial implementation
- **WHEN** the worker baseline equals the accepted baseline
- **THEN** acceptance runs once on the worker candidate before native writeback and advancement

### Requirement: Evidence-bound verification continuation
The system SHALL reuse successful verification only for an identical checkout, committed source, check list, process environment, resolved executables and preparation configuration, with intact persisted evidence. Failures or incomplete waves MUST NOT be reused. Users MUST be able to disable reuse for external state checks. Actual lifecycle hooks MUST force post-hook verification.

#### Scenario: Audit wait continuation
- **WHEN** a coordinator resumes awaiting audit with unchanged verification inputs
- **THEN** previously completed phase verification is reused and reported as reused

#### Scenario: Changed input or evidence
- **WHEN** code, environment, executable, checks or evidence changes
- **THEN** verification executes again

### Requirement: Conservative affected phase review
The system SHALL invalidate affected phases and their transitive dependents using declared task read and write scopes. Unknown change coverage, missing scopes, hooks and global configuration MUST invalidate all completed phases.

#### Scenario: Known independent scope
- **WHEN** an external change belongs only to a declared phase scope
- **THEN** unrelated completed phases remain accepted

### Requirement: Safe maintenance and backlog reporting
The system SHALL reclaim eligible closed checkouts at coordinator handoffs using existing ownership, reference, dirty, lock, ignored-file and recovery protections. Evidence and branches MUST be retained. Resource pressure MUST scale with declared concurrency and distinguish retained backlog from active capacity.

#### Scenario: Handoff cleanup
- **WHEN** work has closed and a coordinator hands off
- **THEN** eligible resources are removed while live work, unpreserved data and recovery dependencies remain with explicit reasons
