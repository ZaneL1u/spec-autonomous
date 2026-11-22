## MODIFIED Requirements

### Requirement: Bound retries and detect lack of progress
The system SHALL enforce configured concurrency, attempt, run-time and repair-round limits and SHALL stop repeated failures without material progress. The run-time budget SHALL count active time only: coordinator work and time the run waits in `awaiting_host` with issued packets, but not time in `needs_input`, `paused` or a terminal state. Runs recorded without an activity anchor SHALL resume from their recorded elapsed time. Reloading configuration SHALL never shorten a granted deadline, and a budget stop SHALL state the smallest extension that reopens the budget and, without `unsafe_budgets`, how much the 72-hour cap still allows. A no-progress blocker SHALL list only the latest failure of each unfinished task.

#### Scenario: Repeated identical failure
- **WHEN** the configured no-progress threshold is reached
- **THEN** the system stops new repair attempts and reports a precise resumable blocker

#### Scenario: Host away overnight
- **WHEN** a run waits in needs_input for longer than its remaining budget and the host then answers
- **THEN** the waiting time is not charged and the run continues without budget_exhausted

#### Scenario: Reload after an extension
- **WHEN** a run extended to 72000 seconds is resumed with reload_config and a configured run_timeout_seconds of 28800
- **THEN** the deadline stays at least 72000 seconds plus the new extension

## ADDED Requirements

### Requirement: Re-audit only acceptance items whose inputs changed
Audit items MAY name the files their conclusion rests on. On a repeated audit of the same phase and scope, the system SHALL carry over a passed item when its reference text is unchanged and the diff since its audit commit touches neither those files, the write sets of the tasks that wrote them, native documents, global inputs nor files outside every declared write set, recording the source attempt, commit and tree. Only the remaining items SHALL be sent, a fully carried batch SHALL issue no packet, items without files SHALL always be audited again, and a worker receipt claiming carry-over SHALL be rejected.

#### Scenario: Repair touches one module
- **WHEN** a phase is audited again after a repair that changed files named by only one acceptance item
- **THEN** the audit packet lists only that item and the others are reported as carried over

#### Scenario: Item without files
- **WHEN** an earlier passed item named no files
- **THEN** it is audited again

### Requirement: Let the milestone audit stand on phase audits
For a milestone acceptance item without a carriable milestone-scope conclusion that belongs to exactly one phase, the milestone audit SHALL take that phase audit's passed conclusion under the same carry-over rules, judging native documents by the owning phase's own documents. Items shared by several phases SHALL always be audited again, and milestone-only items SHALL always be audited.

#### Scenario: Single-phase milestone
- **WHEN** the only phase of a milestone passed its audit and nothing changed afterwards
- **THEN** the milestone audit packet asks only for milestone-only items such as the goal

### Requirement: Judge extensions and gaps precisely
Audit packets SHALL instruct auditors that an implementation covering everything an item requires and going beyond an example list without contradicting it passes, with evidence marked `extension:`. The coordinator SHALL read such items as passed and SHALL NOT schedule repairs for them, and converge packets SHALL list only failed items as gaps.

#### Scenario: Specification lists examples
- **WHEN** an auditor finds that the implementation supports one more format than the specification's example list
- **THEN** the item passes with extension evidence and no converge gap is created for it

### Requirement: Hand real-environment evidence to the host
Implement and audit receipts MAY carry deferred evidence of kind live, manual or external for proof the checkout cannot produce. Deferred evidence SHALL NOT count as failure. The coordinator SHALL list the latest accepted items in run views, `next` and progress, and a completed run SHALL keep them with a `deferred_evidence_pending` warning without executing or dismissing them.

#### Scenario: Deployment check left to the host
- **WHEN** an audit item passes with a deferred live check against a deployed service
- **THEN** the milestone completes and its warnings list the pending item for the host
