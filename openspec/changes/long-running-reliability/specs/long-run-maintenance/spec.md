## Purpose

Keep long-running host-driven milestones recoverable and observable while bounding disposable resources, isolating verification output, and retaining independent evidence and ownership.

## ADDED Requirements

### Requirement: Explicit active-run cleanup
The system SHALL support reviewed cleanup of closed work during a nonterminal run only with explicit active-run scope. It SHALL keep the integration, live owners, outstanding work, accepted semantic work needed for recovery, dirty or locked worktrees, unknown resources, and durable evidence. Applying cleanup SHALL require the coordinator lock and the exact current preview hash; active integration deletion SHALL be rejected.

#### Scenario: Cleanup between work packets
- **WHEN** a running milestone has closed integrated attempts and a live claimed worker
- **THEN** a reviewed cleanup removes only eligible closed work and preserves the live worker and all accepted task state

#### Scenario: Coordinator or preview changed
- **WHEN** another coordinator owns the lock or the run changes after preview
- **THEN** application is refused before removing any resource

### Requirement: Isolated verification waves
Each verification invocation SHALL allocate a unique output directory outside source worktrees and expose its identity, source revision, checkout and stage to checks. An optional preparation hook SHALL run once per wave before checks with the same environment. Preparation failure SHALL prevent tests from running. Existing checks SHALL keep their meaning and successful evidence from another wave SHALL NOT substitute for a fresh result. Exact duplicate combined checks SHALL run once per wave.

#### Scenario: Worker and combined verification
- **WHEN** a candidate is verified in worker and combined worktrees
- **THEN** both stages receive different output directories and independent preparation while previous reports remain intact

### Requirement: Durable verification diagnostics
Verification SHALL persist a result manifest containing check identity, stage, duration, outcome and stdout/stderr references and hashes. Timeout, cancellation, preparation, launch and assertion failures SHALL be distinguishable. Public errors SHALL identify diagnostic paths without copying arbitrary command output.

#### Scenario: Failed external command
- **WHEN** an external check exits unsuccessfully or cannot start
- **THEN** a durable diagnostic points to the original logs and failed stage without rerunning the suite

### Requirement: Bounded project progress
Progress SHALL show declared milestones without execution as unverified native work, include retry and verification overhead and workspace pressure, and support an explicit bounded list of related repository paths. Related repositories SHALL be read-only, deduplicated by Git common directory, nonrecursive, and reported separately; missing repositories SHALL remain visible as errors.

#### Scenario: Native development without a run
- **WHEN** a declared milestone has native checked tasks but no execution ledger
- **THEN** progress shows native counts and no verified completion

#### Scenario: Separate executor clone
- **WHEN** the caller supplies a related executor repository and a duplicate worktree path
- **THEN** that repository appears once without changing aggregate counts for the primary repository

### Requirement: Safe consumer upgrade
Doctor SHALL report installed skill content drift even when versions match. Documentation SHALL require retaining the old executable through active commands, preserving the run ID and accepted work, and switching writers only at a quiescent handoff. New behavior SHALL NOT rewrite existing consumer checks or restart their work automatically.

#### Scenario: Same-version changed skill
- **WHEN** installed skill bytes differ from the running CLI bundled skill
- **THEN** doctor reports content drift without modifying the skill
