## ADDED Requirements

### Requirement: Stop on a repeated identical failure
The system SHALL identify each repair round by the phase and a failure signature: the failing command, cwd and exit code for host verification, or the failed requirement ids for audits. When the same signature recurs after a repair round, the run SHALL stop with needs_input instead of spending further repair budget, and the signature count SHALL reset when the phase completes.

#### Scenario: Phase gate fails identically after a repair
- **WHEN** a phase check fails with the same command and exit code after one convergence repair was integrated
- **THEN** the run reports needs_input with the repeated failure and issues no second repair
