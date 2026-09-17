## Purpose

Enable an installed Spec Autonomous npm CLI to prepare missing native SDD tools on another machine, preserving existing specifications and host-owned agent execution while exposing inspectable installation state.

## ADDED Requirements

### Requirement: Native tool readiness
The npm entry point SHALL install missing selected OpenSpec or Spec Kit tools, including missing Spec Kit runtime prerequisites, using JavaScript compatible with Bun and Node. It MUST preserve usable existing tools and avoid global project dependency or shell configuration changes.

#### Scenario: Missing Spec Kit and Python tooling
- **WHEN** a user ensures Spec Kit on a machine without specify, uv or suitable Python
- **THEN** the installation layer prepares the prerequisites in a user-owned location and reports ready only after executing the installed tool successfully

#### Scenario: Existing native installation
- **WHEN** a usable provider command already exists
- **THEN** readiness reuses it without downloading or upgrading it

### Requirement: Framework selection and initialization
Initialization SHALL install the repository's selected provider automatically. An uninitialized repository MUST require an explicit provider selection and use that provider's native initializer. Existing files MUST NOT be overwritten with different generated content.

#### Scenario: Empty repository
- **WHEN** init is called with an explicit provider and supported agent
- **THEN** native scaffold and Spec Autonomous bindings are installed without creating an agent session

#### Scenario: Ambiguous repository
- **WHEN** both frameworks are detected without selection
- **THEN** the operation returns selection_required without installing either framework

### Requirement: Durable bounded installation
The installation layer SHALL verify fixed uv download hashes, serialize concurrent installs, bound network/process waits, and expose only verified installations. Offline mode MUST reject missing dependencies without downloading.

#### Scenario: Failed or interrupted installation
- **WHEN** installation fails before readiness validation
- **THEN** no ready record is published and a subsequent request can retry safely without reusing partial files

#### Scenario: Concurrent requests
- **WHEN** two worktrees request the same missing provider concurrently
- **THEN** both converge on one verified installation without publishing partial state

### Requirement: CLI and host integration
The npm CLI and MCP SHALL expose structured provider status and ensure capabilities and prepare dependencies before operations that need native tools. Progress, doctor and capability discovery MUST NOT initiate installation. Provider subprocess output MUST NOT corrupt JSON or MCP stdout.

#### Scenario: MCP prepares on a fresh machine
- **WHEN** a host requests native planning without a usable provider CLI
- **THEN** the provider is prepared before the core operation and all responses remain valid MCP messages

#### Scenario: Inspect progress while offline
- **WHEN** the user requests progress or discovers tools without installed providers
- **THEN** no install or network request occurs
