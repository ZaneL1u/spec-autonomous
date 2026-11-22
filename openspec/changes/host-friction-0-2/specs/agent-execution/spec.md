## MODIFIED Requirements

### Requirement: Isolate writing workers
The system SHALL assign each writing request an independent managed Git worktree and SHALL report the host sandbox capabilities separately from worktree isolation. Worker worktrees SHALL be placed under the configured `worktree_root` (the Git runtime directory by default, a sibling directory or an absolute directory outside the checkout), while coordinator-only checkouts stay in the runtime directory and existing runs keep their frozen location. Before issuing a writing packet the system SHALL initialize submodules when `init_submodules` is enabled and run the configured idempotent `worker_prepare` hook; when either fails no packet SHALL be issued, the attempt SHALL NOT count as a task failure and the run SHALL stop with `worker_prepare_failed`.

#### Scenario: Concurrent writers
- **WHEN** a host runs two independent writing requests concurrently
- **THEN** they use separate assigned worktrees and neither owns shared source-task completion writes

#### Scenario: Project with submodules and a Python environment
- **WHEN** init_submodules is true and worker_prepare creates the virtual environment
- **THEN** the issued worktree contains the submodule contents and the environment before the host receives the packet

#### Scenario: Sibling worktree root
- **WHEN** a new run uses worktree_root = "sibling"
- **THEN** worker checkouts are created next to the main checkout outside `.git`, and an earlier run keeps its checkouts under the runtime directory

## ADDED Requirements

### Requirement: Prepare read-only checkouts
When `prepare_read_only` is enabled, which SHALL be the default, planning and audit checkouts SHALL receive the same submodule initialization and `worker_prepare` hook as writing checkouts, with the work kind exposed to the hook. A hook that leaves tracked changes SHALL fail preparation with `worker_prepare_failed`. Ignored entries left by a successful hook SHALL be recorded so the checkout stays reusable, and the hook SHALL run again when the checkout is reused.

#### Scenario: Auditor runs the test suite
- **WHEN** an audit packet is issued for a project whose worker_prepare creates `server/.venv`
- **THEN** the audit checkout holds its own environment and the auditor does not use the main checkout

#### Scenario: Hook modifies a tracked file
- **WHEN** worker_prepare rewrites a tracked lockfile in a read-only checkout
- **THEN** no packet is issued and the blocker names the tracked changes

### Requirement: Keep worker prompts to a summary
Worker prompts SHALL hold the worker rules, the absolute input path and its hash, and a per-kind summary instead of the full input, and packet input SHALL be trimmed by work kind. Request keys SHALL be computed from the untrimmed snapshot so packets issued before an upgrade still match their receipts. Native task sub-items SHALL reach planners as task details.

#### Scenario: Implement packet on a large project
- **WHEN** an implement packet is issued for a project with large context documents
- **THEN** prompt.md contains a summary of the task and the input path rather than the embedded request

### Requirement: Continue from the previous candidate
A re-issued implement packet after a failed or blocked attempt SHALL carry that attempt's patch, base commit and changed files as `previous_candidate`, taken from its worktree or the stored patch of a reclaimed worktree, and SHALL be omitted when no such attempt exists.

#### Scenario: Blocked worker nearly finished
- **WHEN** an implement attempt blocked after editing several files and its task is issued again
- **THEN** the new packet carries the previous patch and instructs the worker to apply it and re-run checks
