## Purpose

Provide a predictable bilingual command line interface that follows the user's locale while keeping command identifiers and machine-readable protocols stable for Skills, LLM hosts and MCP clients.

## ADDED Requirements

### Requirement: Resolve the interface locale
The CLI SHALL resolve a supported locale with explicit `--lang` taking precedence over the documented environment variables, and SHALL fall back to English for unknown, C or POSIX locales.

#### Scenario: Chinese environment
- **WHEN** `LC_ALL=zh_CN.UTF-8` is set and no explicit language is supplied
- **THEN** help, human status labels and localized error messages are shown in Simplified Chinese

#### Scenario: Explicit override
- **WHEN** `--lang en-US` is supplied in a Chinese environment
- **THEN** the interface is shown in English while command identifiers and output schema remain unchanged

### Requirement: Localize human interface resources
The CLI SHALL load reviewed English and Simplified Chinese catalogs for root and nested help, option descriptions, provider setup messages, common errors and human progress labels. Missing translations SHALL fall back to English without a network request.

#### Scenario: Nested help
- **WHEN** a user requests `providers ensure --help` under a Chinese locale
- **THEN** nested help is Chinese, while `providers`, `ensure`, option names and provider enum values remain stable

### Requirement: Preserve structured protocols
JSON and MCP output SHALL keep field names, status values, error codes, tool names and command identifiers in their existing English forms. Only human text fields MAY be localized.

#### Scenario: JSON error
- **WHEN** an invalid command is run with `--lang zh-CN --json`
- **THEN** the response is parseable with the same `schema_version` and `error.code` as English output, while `error.message` is Chinese

### Requirement: Runtime parity
The Rust native binary, Node npm facade and Bun execution path SHALL negotiate the same locale and render equivalent help and errors without starting agents or changing project state.

#### Scenario: Node and native parity
- **WHEN** the same help command runs under `LANG=zh_CN.UTF-8` through npm and the native binary
- **THEN** both include the same localized command descriptions and neither installs providers or modifies the repository
