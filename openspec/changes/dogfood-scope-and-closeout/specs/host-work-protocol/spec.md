## ADDED Requirements

### Requirement: Accept compatible heartbeat envelopes
`work.heartbeat` SHALL accept the required `run_id`, `request_id` and `token` fields and MAY ignore a well-formed optional `host` object copied from a claim envelope. Extra unknown fields other than `host` SHALL still be rejected.

#### Scenario: Heartbeat reuses the claim host object
- **WHEN** a host sends `work.heartbeat` with `run_id`, `request_id`, `token` and the same `host` object used for `work.claim`
- **THEN** liveness is recorded and the response is `continue`, `pause` or `stop` rather than `unknown field arguments.host`

### Requirement: Diagnose installation drift and stranded work
`doctor` SHALL report whether installed skill manifests match the running CLI version, the readable ledger `user_version`, and any outstanding issued, claimed or submitted requests whose leases are stale. It SHALL NOT start agents, revoke ownership, or delete worktrees.

#### Scenario: Skills lag the CLI
- **WHEN** `.spec-autonomous/skills-installed.toml` records a version different from the running CLI
- **THEN** doctor includes a `skills_version_drift` diagnostic naming both versions

#### Scenario: Stale outstanding work
- **WHEN** a non-terminal run has issued, claimed or submitted work whose heartbeat is older than the configured lease
- **THEN** doctor lists those request IDs as stale outstanding work and does not allocate a replacement worker

### Requirement: Record optional host usage without inventing zeros
`apply-result` MAY accept an optional `usage` object. When present, the ledger SHALL retain it on the attempt. Progress SHALL aggregate reported token counts when at least one attempt recorded usage, and SHALL continue to report `unavailable` when no host has supplied usage.

#### Scenario: Host omits usage
- **WHEN** apply-result succeeds without a usage object
- **THEN** progress still reports `usage` as `unavailable`

#### Scenario: Host reports token usage
- **WHEN** apply-result includes `usage.input_tokens` and `usage.output_tokens`
- **THEN** a later progress snapshot aggregates those counts instead of claiming zero cost
