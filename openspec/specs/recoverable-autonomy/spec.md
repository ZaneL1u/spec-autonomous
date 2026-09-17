# recoverable-autonomy Specification

## Purpose
定义自治执行在验证修订、进程中断与持久化意图场景下的安全恢复行为，确保恢复过程核对实际 Git 和进程状态，避免丢失已验收工作、重复语义劳动或盲目重放副作用。
## Requirements
### Requirement: Revise pending verification without losing accepted work
The system SHALL preview and apply reasoned verification changes to pending work with a compare-and-swap hash. Accepted tasks, native requirements, run identity, scope and Git history SHALL remain unchanged. Revised checks SHALL still execute before completion.

#### Scenario: Late command correction
- **WHEN** a later task fails due to incorrect verification argv after earlier tasks were accepted
- **THEN** the host can revise the pending checks and continue the same run without repeating accepted tasks or erasing failure evidence

#### Scenario: Stale or active revision
- **WHEN** the preview becomes stale or work is still owned or submitted
- **THEN** apply is refused without partial modification

#### Scenario: Obsolete verification repair
- **WHEN** a phase command correction supersedes a pending host-verification repair
- **THEN** the obsolete repair is recorded and cleared and corrected checks run before any new completion claim

### Requirement: Expose honest verification readiness
Planning SHALL inspect executable and path readiness, report deferred future files separately, and expose command portability diagnostics. Rejected planning results SHALL be correctable through fresh work packets.

#### Scenario: Future test file
- **WHEN** verification references a test file produced by planned writes
- **THEN** readiness reports deferred rather than claiming test success or rejecting its absence as implementation failure

#### Scenario: Invalid planner command
- **WHEN** a proposed verifier is unavailable
- **THEN** the candidate is durably rejected and a subsequent planner receives its diagnostics in a fresh packet

### Requirement: Reduce avoidable host and resource overhead
The system SHALL support atomic batch claims, guidance for coherent task granularity, and previewed cleanup of terminal owned resources.

#### Scenario: Batch ownership failure
- **WHEN** any request in a batch has an invalid token or reused context identity
- **THEN** no request in the batch is claimed

#### Scenario: Terminal cleanup
- **WHEN** cleanup is applied using a current preview
- **THEN** safe owned resources can be removed while dirty, active, unknown, integration and evidence resources are retained with explicit reasons
