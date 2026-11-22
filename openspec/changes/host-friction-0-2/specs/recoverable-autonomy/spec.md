## MODIFIED Requirements

### Requirement: Reduce avoidable host and resource overhead
The system SHALL support atomic batch claims, guidance for coherent task granularity, and previewed cleanup of terminal owned resources. Cleanup and automatic maintenance SHALL be able to remove a clean worktree whose submodules were initialized after re-checking at mutation time that the superproject and every submodule are clean, and SHALL keep locked worktrees and worktrees with submodule changes.

#### Scenario: Batch ownership failure
- **WHEN** any request in a batch has an invalid token or reused context identity
- **THEN** no request in the batch is claimed

#### Scenario: Terminal cleanup
- **WHEN** cleanup is applied using a current preview
- **THEN** safe owned resources can be removed while dirty, active, unknown, integration and evidence resources are retained with explicit reasons

#### Scenario: Clean worker checkout with initialized submodules
- **WHEN** init_submodules is enabled and a closed worker worktree and its submodules are clean
- **THEN** cleanup removes the worktree instead of retaining it after Git refuses the plain removal

## ADDED Requirements

### Requirement: Revise a task's write set
`run.revise` SHALL accept `task_writes` additions for pending tasks. Additions SHALL be normalized, append only, stay outside `.git/`, `.spec-autonomous/` and the phase's native context including its parent directories and matching patterns, and not overlap other outstanding implement work. Applied additions SHALL be recorded as revisions, reset the task's retry epoch and survive replanning.

#### Scenario: Operator grants a migration directory
- **WHEN** the operator revises a task's writes to add `server/app/migrations`
- **THEN** the revision is recorded, the re-issued packet carries the larger write set and a later replan keeps it

#### Scenario: Revision reaches native context
- **WHEN** a task_writes addition names the parent directory of the phase's tasks file
- **THEN** the revision is refused with write_scope_forbidden without partial modification

### Requirement: Keep the plan when only the environment is missing
A planning candidate whose preflight errors are all project-level missing executables or working directories SHALL remain a candidate, and the run SHALL stop with `verification_environment_unavailable`. Continuing the run SHALL re-run only the preflight on the kept plan. Task-level gaps SHALL still reject the plan.

#### Scenario: Tool installed after the stop
- **WHEN** a plan was kept because `uv` was missing and the operator installs it and continues the run
- **THEN** the preflight runs again on the same plan and no planning packet is issued
