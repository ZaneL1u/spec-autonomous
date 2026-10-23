## MODIFIED Requirements

### Requirement: Resume phase and mode boundaries
The system SHALL persist roadmap revision, stable range endpoints, native workflow stage and handoff checkout so resumption does not widen scope or recreate already valid artifacts. Resumption SHALL keep completed phases and recorded plans: an original checkout that only absorbed accepted integration history SHALL be recognized without merging or invalidating progress; foreign code changes SHALL mark completed phases stale so their command gates run again before delivery; native source changes SHALL reopen only the phases whose source changed, while phases depending on them become stale rather than reopened.

#### Scenario: Resume after native edits
- **WHEN** the user continues work through native commands and then resumes autonomous execution
- **THEN** the system reconciles the new source revision, preserves valid progress, invalidates affected evidence and retains the selected phase boundaries

#### Scenario: Original checkout fast-forwarded to the integration branch
- **WHEN** the original checkout's HEAD is an ancestor of the accepted integration revision at resume
- **THEN** the run records the new original revision and continues without a reconcile merge, replanning or phase re-verification

#### Scenario: Foreign commit on the original checkout
- **WHEN** the original checkout gains a commit touching files outside every phase source directory
- **THEN** completed phases remain completed, their plans are retained, and each completed phase's command gate runs once more before delivery

#### Scenario: Native source of a completed phase changes
- **WHEN** the original checkout changes the tracking file of a completed phase that other completed phases depend on
- **THEN** only that phase is reopened and replanned while dependent phases stay completed and are re-verified before delivery

## ADDED Requirements

### Requirement: Defer source reconciliation while work is in flight
The system SHALL NOT revoke or discard issued, claimed, submitted or integrating work because the original source changed. It SHALL record the pending reconciliation, keep accepting and integrating receipts against the current integration revision, stop issuing new work, and merge the changed source once no work is in flight. Only when nothing is in flight SHALL a source change pause the run immediately.

#### Scenario: Specification edited while a worker runs
- **WHEN** the original checkout commits a specification change while an implement packet is outstanding
- **THEN** the packet stays valid, its receipt is integrated, the source is merged afterwards and no attempt is revoked

### Requirement: Reclaim closed worktrees automatically
After an attempt is integrated, superseded, revoked, accepted as non-implement work, or is an older failure beyond the retained count, the coordinator SHALL store the attempt's patch and its hash, remove the attempt's worktree, and keep the branch ref, packet and evidence. Accepted or abandoned candidate and reconcile worktrees SHALL be removed the same way. The integration checkout, the original checkout and work with a live lease or pending receipt SHALL never be reclaimed. Reclamation SHALL be disableable per project, and retry fingerprints SHALL use the stored patch hash.

#### Scenario: Worker integrated
- **WHEN** a worker candidate is integrated
- **THEN** its worktree directory is removed, its branch and patch remain, and the attempt records worktree_state reclaimed

### Requirement: Store attempts and evidence as ledger rows
The ledger SHALL store run attempts and evidence in their own tables so that saving a run writes only changed rows, SHALL migrate schema 2 and 3 payloads in one transaction after a consistent backup, SHALL refuse writes from older CLIs, and SHALL keep the JSON and MCP run representation unchanged.

#### Scenario: Heartbeat on a large run
- **WHEN** a heartbeat updates one lease on a run with thousands of evidence entries
- **THEN** the run core row is rewritten but no attempt or evidence row changes

### Requirement: Read-only run forensics
`run.forensics` SHALL report, without modifying state, repeated failing checks, tasks with many attempts, idle outstanding requests, repeated phase completions, frequent source reconciliations, repeated handoffs, pending reconciliation, stale phases and worktree backlog, with the evidence that supports each finding.

#### Scenario: Diagnose a stuck run
- **WHEN** the host asks for forensics on a run whose phase gate failed twice with the same command
- **THEN** the report lists repeated_failing_check with the evidence indexes and a hint, and nothing in the run changes
