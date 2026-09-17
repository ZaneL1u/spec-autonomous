# private-git-install Specification

## Purpose
Allow users with private GitHub repository access to install the CLI directly from a Git SSH URL and retrieve verified platform binaries using their existing GitHub CLI authentication, without compiling Rust locally.
## Requirements
### Requirement: Direct Git installation
The repository root SHALL expose a working spec-autonomous CLI when installed from its Git URL. The installed facade MUST NOT require Rust or Bun, and existing tarball distribution MUST remain usable.

#### Scenario: Install a private repository
- **WHEN** a user with SSH access and authenticated gh runs npm install -g with this repository URL
- **THEN** the installed CLI reports the pinned version and can use existing workflow capabilities

### Requirement: Verified private binary delivery
Git installations SHALL select the platform's published Release asset and verify its pinned checksum and version before use. Downloads MUST reuse gh authentication without storing tokens in project files.

#### Scenario: Incorrect asset bytes
- **WHEN** an asset has the wrong SHA256
- **THEN** installation fails without exposing a usable cached binary

### Requirement: Cache and lifecycle recovery
Concurrent requests SHALL converge on one verified cached binary. Source checkout dependency installation MUST NOT download it. With lifecycle scripts disabled, the CLI SHALL prepare the missing binary on first use.

#### Scenario: No install scripts
- **WHEN** a Git package was installed with --ignore-scripts
- **THEN** its first invocation retrieves and verifies the required binary using the same delivery configuration

#### Scenario: Missing authentication or platform
- **WHEN** gh is unavailable, authentication fails, offline mode forbids a download, or the platform has no published asset
- **THEN** the CLI returns an actionable error rather than invoking a compiler or claiming successful preparation
