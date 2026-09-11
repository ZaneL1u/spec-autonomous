## Purpose

Provide predictable host interface capabilities for existing SDD hosts while preserving source authority and observable execution boundaries.

## ADDED Requirements

### Requirement: Shared CLI and MCP service
The CLI and stdio MCP transport SHALL invoke the same deterministic capability implementations; default tool discovery SHALL present the complete operations and provide access to advanced granular tools.

#### Scenario: MCP progress
- **WHEN** a host calls the progress tool
- **THEN** it receives the same semantic data as CLI progress without an agent process

### Requirement: Host owned semantic work
Skills SHALL use native host agent capabilities for semantic work and submit structured results; they SHALL not configure or invoke a CLI agent runner.

#### Scenario: Autonomous skill
- **WHEN** a user invokes the autonomous entry
- **THEN** the skill uses prepared work packets and the host existing subagent facility

### Requirement: Historical run compatibility
Historical runs SHALL remain readable and SHALL never reactivate an old agent launcher; migration SHALL provide clear next actions.

#### Scenario: Resume legacy run
- **WHEN** a pre-host-protocol run is resumed
- **THEN** the CLI gives a migration result instead of spawning its former runner
