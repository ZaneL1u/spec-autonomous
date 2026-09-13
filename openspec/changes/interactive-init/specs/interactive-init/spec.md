## ADDED Requirements

### Requirement: Select initialization options interactively or by arguments
The npm CLI SHALL offer localized framework and host choices for missing init options in an interactive terminal. Explicit options SHALL take precedence. Complete parameter invocations SHALL remain non-interactive.

#### Scenario: Empty interactive directory
- **WHEN** a user runs init in an empty directory with terminal input and output
- **THEN** the CLI offers OpenSpec or Spec Kit, Codex or Claude Code, and optional MCP setup before initialization

#### Scenario: Automation
- **WHEN** init runs with --non-interactive, CI, or structured output
- **THEN** it never waits for prompts and returns an actionable stable error for missing mandatory selections

#### Scenario: Defaults and ambiguity
- **WHEN** --yes runs in a directory without a framework
- **THEN** OpenSpec, Codex and MCP are selected by default, while an existing ambiguous or incomplete framework setup is never replaced

### Requirement: Complete the project foundation
Initialization SHALL install missing selected tools, run their native initializer and create the project's Spec Autonomous configuration, Skills, declaration directories and optional MCP entry. A directory outside Git SHALL become a Git repository without an automatic commit.

#### Scenario: Parameter initialization
- **WHEN** init --provider speckit --agent claude --non-interactive succeeds in an empty directory
- **THEN** native Spec Kit files, Git metadata, Spec Autonomous config and declaration directories, and Claude commands are present

#### Scenario: Repeat initialization
- **WHEN** init runs again for an existing provider and host
- **THEN** native files and user configuration remain unchanged and owned Skills can be refreshed safely

### Requirement: Validate before side effects and preserve user files
The CLI SHALL collect all interactive choices and validate options and owned configuration before provider installation or project writes. It SHALL preserve existing conflicting files and report failure.

#### Scenario: Cancel selection
- **WHEN** the user cancels a prompt with Ctrl-C or EOF
- **THEN** initialization exits with code 130 and a localized cancellation message, without installing providers or changing the project

#### Scenario: Owned file conflict
- **WHEN** an existing Skill or MCP entry conflicts with initialization
- **THEN** the CLI reports the conflict before downloading tools or creating framework files
