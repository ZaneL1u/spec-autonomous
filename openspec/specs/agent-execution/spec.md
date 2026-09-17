# agent-execution Specification

## Purpose
Provide observable and verifiable agent execution behavior for Spec Autonomous users while preserving clear execution and compatibility boundaries.
## Requirements
### Requirement: Require a fresh host session for every attempt
The system SHALL issue each worker attempt as an immutable host work request that requires a fresh agent context without the full coordinator history or an implicit resume identifier. The CLI SHALL validate the host-declared session identity and SHALL NOT start an agent or model session itself.

#### Scenario: Retry a task
- **WHEN** a new attempt is scheduled after failure
- **THEN** the host receives a new work request containing only task context and relevant failure evidence and claims it with a unique fresh session identity

### Requirement: Isolate writing workers
The system SHALL assign each writing request an independent managed Git worktree and SHALL report the host sandbox capabilities separately from worktree isolation.

#### Scenario: Concurrent writers
- **WHEN** a host runs two independent writing requests concurrently
- **THEN** they use separate assigned worktrees and neither owns shared source-task completion writes

### Requirement: Bound context and validate results
The system SHALL bound coordinator summaries and worker input/results, preserve mandatory constraints, and verify result identity, paths and actual changes before accepting a candidate.

#### Scenario: Oversized or mismatched result
- **WHEN** a worker returns an oversized result or the wrong attempt identity
- **THEN** the result is rejected without marking the task complete

### Requirement: Coordinate cancellation across the ownership boundary
The CLI SHALL stop and clean up complete process trees only for deterministic subprocesses that it started. For host-owned agent sessions, pause, cancel, timeout and stale leases SHALL return explicit host stop actions and SHALL require host confirmation before ownership is revoked or work is reassigned.

#### Scenario: Host request exceeds its lease
- **WHEN** a host-owned request has no recent heartbeat
- **THEN** the CLI reports its state as stale or unknown, does not start a replacement agent, and returns the stop-and-revoke action required from the host

#### Scenario: Verification subprocess exceeds timeout
- **WHEN** a CLI-owned verification process exceeds its configured timeout
- **THEN** the CLI terminates its complete process tree and records durable failure evidence on every platform currently claimed as supported

### Requirement: Provide skill entry points over the CLI
The npm distribution SHALL include milestone, autonomous, progress and resume skill assets with autonomous as the primary short command and auto as its exact alias. Host-specific entry points SHALL invoke the same CLI contracts as terminal users rather than maintain a second orchestration state machine.

#### Scenario: Invoke autonomous through a skill
- **WHEN** a supported host invokes the autonomous skill with milestone and phase bounds
- **THEN** the CLI receives equivalent selection and policy fields, owns deterministic runtime state, and returns semantic work to the host

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
