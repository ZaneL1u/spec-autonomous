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
