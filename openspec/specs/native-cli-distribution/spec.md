# native-cli-distribution Specification

## Purpose
Provide observable and verifiable native cli distribution behavior for Spec Autonomous users while preserving clear execution and compatibility boundaries.

## Requirements

### Requirement: Provide a native CLI through npm
The package SHALL expose a spec-autonomous executable using a thin Node launcher and a matching native Rust binary without requiring Rust or Bun at installed runtime.

#### Scenario: Install local platform tarball
- **WHEN** a matching-host tarball is installed with lifecycle scripts disabled
- **THEN** the executable can report its version and detect a repository

### Requirement: Preserve process arguments and results
The launcher SHALL forward argument boundaries and standard streams without shell interpretation and SHALL propagate child exit status and supported termination signals.

#### Scenario: Argument contains shell syntax
- **WHEN** a literal argument contains spaces or shell metacharacters
- **THEN** the native process receives the unchanged argument

### Requirement: Report unsupported or missing binaries
The launcher SHALL report a clear error for unsupported platforms, Linux musl or a missing native binary.

#### Scenario: Missing platform dependency
- **WHEN** no local binary or matching optional dependency is installed
- **THEN** the command fails with installation or build guidance

### Requirement: Assemble explicit platform packages
Release assembly SHALL require the complete declared target artifact set, pin platform dependencies to the wrapper version and keep reference clones and runtime state out of published files.

#### Scenario: Incomplete artifact set
- **WHEN** one target binary is missing
- **THEN** assembly fails before creating a partial release output
