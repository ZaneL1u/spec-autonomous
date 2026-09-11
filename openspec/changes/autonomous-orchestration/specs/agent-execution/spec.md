## Purpose

Provide observable and verifiable agent execution behavior for Spec Autonomous users while preserving clear execution and compatibility boundaries.

## ADDED Requirements

### Requirement: Start every attempt with a fresh session
The system SHALL start each worker attempt in a fresh agent session without the full coordinator history or implicit resume identifier and SHALL reject runners that cannot provide this capability.

#### Scenario: Retry a task
- **WHEN** a new attempt is scheduled after failure
- **THEN** it receives a new session with only task context and relevant failure evidence

### Requirement: Isolate writing workers
The system SHALL give each writing worker an independent managed Git worktree and SHALL report the actual runner sandbox capabilities separately from worktree isolation.

#### Scenario: Concurrent writers
- **WHEN** two independent writing tasks run concurrently
- **THEN** they use separate worktrees and neither owns shared source-task completion writes

### Requirement: Bound context and validate results
The system SHALL bound coordinator summaries and worker input/results, preserve mandatory constraints, and verify result identity, paths and actual changes before accepting a candidate.

#### Scenario: Oversized or mismatched result
- **WHEN** a worker returns an oversized result or the wrong attempt identity
- **THEN** the result is rejected without marking the task complete

### Requirement: Cancel the full process tree
The system SHALL enforce attempt timeouts and cancellation across the runner process tree on each platform it claims to support.

#### Scenario: Worker exceeds timeout
- **WHEN** the configured attempt timeout expires
- **THEN** dispatch stops for that attempt, its process tree is terminated, and durable failure evidence is recorded

### Requirement: Provide skill entry points over the CLI
The npm distribution SHALL include milestone, autonomous, progress and resume skill assets with autonomous as the primary short command and auto as its exact alias. Host-specific entry points SHALL invoke the same CLI contracts as terminal users rather than maintain a second orchestration state machine.

#### Scenario: Invoke autonomous through a skill
- **WHEN** a supported host invokes the autonomous skill with milestone and phase bounds
- **THEN** the CLI receives equivalent selection and policy fields and owns subsequent runtime state

#### Scenario: Invoke the short alias
- **WHEN** a supported host invokes auto instead of autonomous with the same arguments
- **THEN** provider resolution, phase bounds, policy, run identity and resumption semantics are equivalent

#### Scenario: Host does not support slash aliases
- **WHEN** installation targets a host that only supports native skills
- **THEN** init installs and reports that host's equivalent skill invocation rather than claiming unavailable slash syntax works

### Requirement: Install skills without overwriting user work
The CLI SHALL install skills only into the selected scope and host profile, track owned versions and hashes, and preserve user-modified or unrelated skills during updates and removal.

#### Scenario: Existing custom skill has the same name
- **WHEN** installation encounters a conflicting user-owned skill
- **THEN** it reports the conflict and leaves the existing skill unchanged
