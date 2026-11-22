## MODIFIED Requirements

### Requirement: Idempotent verified result application
Applying results SHALL validate request identity and hashes, independently verify changed code and integrate only accepted revisions; identical receipt replay SHALL be idempotent. When a check is cancelled while it runs, or a pause or cancel request, a process interrupt or the run deadline is noticed before integration starts or between two checks, the receipt SHALL return to `submitted` without an error, SHALL NOT count toward `max_attempts` or no-progress detection, SHALL NOT start a repair round, and SHALL be verified again as the same candidate when the run continues.

#### Scenario: Replay a completed receipt
- **WHEN** the same result and ownership token are submitted twice
- **THEN** the second response identifies a replay and no task or source update occurs twice

#### Scenario: Host process interrupted during the combined check
- **WHEN** the combined check ends as cancelled because the host process was interrupted
- **THEN** the implement receipt stays submitted with no failure recorded and prepare --run-id verifies the same candidate again

#### Scenario: Pause noticed between two checks
- **WHEN** a pause request is noticed after the first of two integration checks passed
- **THEN** the second check does not start, the attempt is not recorded as failed and its retry budget is unchanged

## ADDED Requirements

### Requirement: Grant blocked workers only literal safe paths
A blocked implement receipt MAY request additional writes. The system SHALL grant them automatically only when every path is a normalized literal file path outside `.git/`, `.spec-autonomous/` and the phase's native context, is not a parent directory of native context or a pattern matching it, and does not conflict with other outstanding implement work. Patterns, trailing slashes and existing directories SHALL stop with `write_scope_not_literal`, native context and its parents with `write_scope_forbidden`, and each refusal SHALL carry a ready `run.revise` input for review. An automatic grant SHALL start a new retry epoch for the task and re-issue it with the blocked changes, and a task SHALL receive at most ten paths automatically over a run, beyond which it stops with `write_scope_limit`.

#### Scenario: Worker needs an existing test file
- **WHEN** a blocked implement requests one existing test file outside its writes
- **THEN** the path is appended, the task is re-issued with the previous candidate and the blocked attempt does not consume max_attempts

#### Scenario: Worker requests the change directory
- **WHEN** a blocked implement requests `openspec/changes/<id>`
- **THEN** nothing is granted, the run stops with write_scope_forbidden and the specification and tasks file stay outside the task's writes

#### Scenario: Worker requests a pattern
- **WHEN** a blocked implement requests `tests/**`
- **THEN** the run stops with write_scope_not_literal and a run.revise input the operator can review

### Requirement: Report configuration drift and resume overrides
The run SHALL record a hash of the loaded project configuration at start and on reload, and `prepare`, `next` and `apply-result` SHALL add a `config_drift` warning when the source configuration no longer matches, without reloading it automatically. The full view SHALL list the configured hooks with environment values redacted. The `prepare` command SHALL accept reload, extension and attempt overrides only together with a run id. Packet limits SHALL state that the CLI does not enforce host worker time and that heartbeats are informational.

#### Scenario: Hook restored after the run started
- **WHEN** the project configuration adds a verification_prepare hook after a run froze its configuration
- **THEN** the next prepare response warns with config_drift and the run keeps its frozen configuration until reload_config
