# cli-command-interface Specification

## Purpose
Provide a predictable community-framework CLI interface that keeps native workflow commands and JavaScript provider setup discoverable while validating syntax before any installation or project mutation.
## Requirements
### Requirement: Consistent command discovery
The CLI SHALL generate root and nested command help from community CLI framework definitions and expose provider setup alongside the primary native commands.

#### Scenario: Nested provider help
- **WHEN** the user runs providers ensure --help or help providers ensure
- **THEN** only the selected command's syntax, options and arguments are shown with exit zero and no installation

### Requirement: Validate before side effects
The CLI SHALL reject unknown options, invalid values, conflicting selections and excess arguments before installing providers or initializing files. Native syntax MUST be checked by the same parser used for native execution.

#### Scenario: Invalid init host
- **WHEN** init receives an unsupported agent or unknown option
- **THEN** it exits with a syntax error without installation or project mutation

### Requirement: Preserve structured and native protocols
The CLI SHALL preserve global option placement, native command argument boundaries, forwarded exit codes and termination signals. JSON errors MUST remain parseable and MCP stdout MUST remain JSON-RPC only.

#### Scenario: Provider exec forwards option-like arguments
- **WHEN** provider exec forwards arguments after -- that include --help, --json and shell metacharacters
- **THEN** the provider receives each literal argument unchanged and its exit status is propagated

#### Scenario: JSON syntax error
- **WHEN** a command has invalid syntax and selects JSON output
- **THEN** stdout contains one structured error envelope and exit code is nonzero
