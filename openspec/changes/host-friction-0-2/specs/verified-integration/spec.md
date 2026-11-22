## MODIFIED Requirements

### Requirement: Protect user changes during delivery
The system SHALL deliver according to the selected policy and SHALL not overwrite an original checkout that changed since the recorded starting revision. For fast-forward delivery and the final archive fast-forward, tracked changes, staged changes, submodule changes and untracked files that collide with a path the delivered diff changes SHALL block delivery, while untracked files that do not collide SHALL stay in place without blocking. A collision SHALL mean the same path, a path below a delivered path, or a path where the delivery needs a directory, compared case-insensitively. A blocked delivery SHALL keep the verified branch, report delivery pending and list the blocking paths by category together with the number of untracked files that did not block.

#### Scenario: Original checkout changes before final delivery
- **WHEN** the user edits files or moves its branch during a run with fast-forward delivery
- **THEN** the system keeps the verified integration branch, reports delivery pending, and preserves user changes

#### Scenario: Unrelated untracked note
- **WHEN** the original checkout holds an untracked note whose path the delivered diff does not touch
- **THEN** delivery fast-forwards the checkout and the note stays unchanged in place

#### Scenario: Colliding untracked file
- **WHEN** an untracked file sits at a path the delivered diff creates
- **THEN** the run reports delivery pending, keeps the verified branch and lists that file under the blocking untracked paths

## ADDED Requirements

### Requirement: Narrow gate verification to checks a change reaches
With `verification_scope = "affected"` (the default), combined and phase gates SHALL always execute configured project, stage and phase checks and the current task's own checks, and SHALL let a check declared by other tasks stand on its latest executed pass of the same command identity only when that pass was on an ancestor commit and the files changed since reach none of its owners' declared writes or reads, the check's path operands resolved under its cwd, the preparation hook, global inputs, native documents or files outside every declared write set. Owners with empty writes or reads SHALL always re-run. Only executed passes SHALL be a basis for reuse, and reused evidence SHALL record its source, owners and changed files. `verification_scope = "full"` SHALL execute every check, and the milestone gate SHALL follow `milestone_verification_scope`, which defaults to `full`.

#### Scenario: Three-line repair in one module
- **WHEN** a repair changes only files inside one task's declared writes after other tasks were accepted
- **THEN** that task's checks and the configured checks execute while the other tasks' checks stand on their last pass with reused_from scope "affected"

#### Scenario: Owner did not declare reads
- **WHEN** a check belongs to a task whose reads are empty
- **THEN** the check executes again at every gate

#### Scenario: Global input changed
- **WHEN** a dependency lockfile or a file outside every declared write set changed since the last pass
- **THEN** every task check executes again

#### Scenario: Check runs in a subdirectory
- **WHEN** a check runs with cwd `server` and a change touches an unrelated file under `server`
- **THEN** the check is not treated as depending on the whole directory and may stand on its last pass

### Requirement: Run verify-only tasks in the coordinator
A ready plan task marked `verify_only` SHALL NOT be issued to a worker. The coordinator SHALL run its task checks in the integration checkout as a verify attempt that does not count toward `max_attempts`. A pass SHALL accept the task through an integration that only checks off its native tasks; a failure SHALL issue an implement repair packet whose failure carries the failed command and its output tail; a cancellation SHALL record no failure and run the checks again on resume.

#### Scenario: Verification task passes
- **WHEN** a verify-only task's checks pass on the integration checkout
- **THEN** the task is accepted and its native task checked off without any packet being issued

#### Scenario: Verification task fails
- **WHEN** a verify-only task's check fails
- **THEN** the next packet for that task is an implement repair whose failure names the failed argv and output

### Requirement: Explain which check failed
A failed verification check SHALL report the executed argv, the declared cwd, the exit code and the last non-empty lines of its stderr and stdout before the diagnostics paths, with configured environment values, credential-like pairs, bearer or basic credentials and URL passwords redacted and status words in child output neutralized. A failed preparation hook SHALL report only its exit code.

#### Scenario: Test suite fails during the combined wave
- **WHEN** a combined check exits non-zero
- **THEN** the error names the command, cwd, exit code and redacted output tail, and the re-issued packet and no_progress blocker carry the same text

### Requirement: Apply declared non-secret check environment
Checks in plans, configuration and `run.revise` SHALL accept an optional `env` map applied over the project environment for that check only. Names that override `PATH`, `HOME`, shell variables, `SA_*`, `GIT_*`, `LD_*` or `DYLD_*`, and credential-like names, SHALL be refused. Values SHALL join the verification reuse identity, and evidence SHALL record only the variable names.

#### Scenario: Reserved name in a plan
- **WHEN** a planned check declares `env` with `GIT_DIR`
- **THEN** the plan is rejected with a check_env diagnostic and no worker starts

### Requirement: Run bare openspec checks through the managed provider
A check argv that starts with a bare `openspec` SHALL execute through the managed provider command when `openspec` is not on PATH, in preflight, verification waves and lifecycle checks alike. Planning environments and work packets for such projects SHALL state the absolute argv the coordinator executes and the `providers exec` form, so planners and workers can run the same check.

#### Scenario: OpenSpec only managed by the CLI
- **WHEN** a plan declares `openspec validate <change> --strict` and PATH has no openspec
- **THEN** preflight reports it ready with the resolved argv and worker packets list the mapping under TOOLS

### Requirement: Report checks that fail on the accepted baseline
When `baseline_checks` is enabled, the coordinator SHALL run a newly accepted phase plan's already runnable checks once on the integration checkout at the accepted revision before issuing implement work, skipping deferred checks and checks naming files a task will write. Failures SHALL be reported as `baseline_failure:` warnings until the phase completes and handed to the phase's implement and converge packets, and SHALL NOT block, count as failures or start repairs. The integration checkout SHALL be restored afterwards. The option SHALL default to off.

#### Scenario: Stale assertion already fails on main
- **WHEN** baseline_checks is on and the full suite already fails before any task is implemented
- **THEN** prepare warns with baseline_failure and the phase's implement packets carry the failure as metadata
