## MODIFIED Requirements

### Requirement: Shared CLI and MCP service
The CLI and stdio MCP transport SHALL invoke the same deterministic capability implementations; default tool discovery SHALL present the complete operations and provide access to advanced granular tools. The default capability list SHALL name the hidden capabilities and how to list them. MCP registration SHALL write a command the host can start: the bare command when it is on PATH, otherwise the absolute Node and launcher script reported by the npm layer, and doctor SHALL report registered MCP commands that cannot start.

#### Scenario: MCP progress
- **WHEN** a host calls the progress tool
- **THEN** it receives the same semantic data as CLI progress without an agent process

#### Scenario: CLI installed outside PATH
- **WHEN** init --mcp runs through the npm launcher while spec-autonomous is not on PATH
- **THEN** the written MCP entry uses the absolute launcher so the host does not fail with ENOENT

## ADDED Requirements

### Requirement: Name the exact continuation command
Blockers and skills that ask the host to continue a run SHALL name `prepare --run-id <id>` without source selectors. `dirty_checkout` errors SHALL list the modified, untracked and submodule paths that make a checkout dirty. A discussion stop SHALL be resolved by `discussion_auto` regardless of the `needs_input:` prefix on the blocker.

#### Scenario: Source drift stop
- **WHEN** a run stops because the original source changed
- **THEN** the blocker names `spec-autonomous prepare --run-id <id>` and does not suggest repeating --change

#### Scenario: Dirty checkout at start
- **WHEN** prepare refuses to start because the original checkout has untracked files
- **THEN** the error lists those paths by category
