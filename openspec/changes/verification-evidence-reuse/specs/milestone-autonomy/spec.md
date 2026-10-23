## ADDED Requirements

### Requirement: Stop on a repeated identical failure
The system SHALL identify each repair round by the phase and a failure signature: the failing command, cwd and exit code for host verification, or the failed requirement ids for audits. When the same signature recurs after a repair round, the run SHALL stop with needs_input instead of spending further repair budget, and the signature count SHALL reset when the phase completes.

#### Scenario: Phase gate fails identically after a repair
- **WHEN** a phase check fails with the same command and exit code after one convergence repair was integrated
- **THEN** the run reports needs_input with the repeated failure and issues no second repair

### Requirement: Cap retry, repair and deadline budgets
Configuration SHALL reject `max_attempts` above 10, `max_repair_rounds` above 4 or `run_timeout_seconds` above 72 hours unless `unsafe_budgets` is explicitly true, and doctor SHALL warn when any of those budgets or `no_progress_limit` exceeds twice its default or when `unsafe_budgets` is enabled.

#### Scenario: Oversized budgets without opt-in
- **WHEN** a project sets max_attempts to 20 and a ten-day run timeout
- **THEN** the configuration is rejected with budget_cap and the message names unsafe_budgets

#### Scenario: Large but allowed budget
- **WHEN** max_attempts is 8
- **THEN** the configuration loads and doctor reports a budget_warning for it
