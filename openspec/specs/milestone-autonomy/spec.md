# milestone-autonomy Specification

## Purpose
Provide observable and verifiable milestone autonomy behavior for Spec Autonomous users while preserving clear execution and compatibility boundaries.
## Requirements
### Requirement: Automatically plan and complete a milestone
The system SHALL accept a milestone goal or existing native artifacts, produce or reconcile a roadmap, and advance selected phases through native planning, implementation, verification, bounded repair and integration without repeated continuation prompts.

#### Scenario: Start with only a milestone goal
- **WHEN** the user supplies a sufficiently bounded goal and selects autonomous mode
- **THEN** the system generates a roadmap and follows the selected framework's native planning contract before dispatching implementation tasks

#### Scenario: Recoverable intermediate failure
- **WHEN** a task fails verification with a repairable in-scope error and sufficient budget remains
- **THEN** a fresh repair attempt is scheduled and subsequent eligible tasks continue automatically

### Requirement: Stay within planned intent
The system SHALL permit execution decomposition and technical repairs within the existing scope and SHALL pause for decisions that introduce new product requirements or require new authorization.

#### Scenario: Proposed scope expansion
- **WHEN** an agent proposes a requirement absent from the selected milestone
- **THEN** the system records the proposal as needing input rather than silently implementing it

### Requirement: Bound retries and detect lack of progress
The system SHALL enforce configured concurrency, attempt, wall-clock and repair-round limits and SHALL stop repeated failures without material progress.

#### Scenario: Repeated identical failure
- **WHEN** the configured no-progress threshold is reached
- **THEN** the system stops new repair attempts and reports a precise resumable blocker

### Requirement: Prove whole-milestone completion
The system SHALL report completed only after all mandatory source scope, task integration, source writeback, milestone acceptance and configured delivery conditions pass.

#### Scenario: Checkboxes are already checked
- **WHEN** all source tasks are checked but verified evidence is absent
- **THEN** the system audits the actual implementation and acceptance conditions before reporting completion

### Requirement: Select an inclusive roadmap range
The system SHALL support from/to inclusive roadmap phase bounds and an exclusive only-phase selector, preserving stable endpoint identities and dependency gates across roadmap refreshes.

#### Scenario: Execute a bounded range
- **WHEN** the user selects phases 2 through 4 with satisfied prerequisites
- **THEN** the system runs those phases, stops at the upper bound and reports scope completion without automatic whole-milestone lifecycle actions

#### Scenario: Prerequisite is outside the selected range
- **WHEN** a selected phase depends on an incomplete phase outside the requested bounds
- **THEN** the system reports the prerequisite and neither bypasses it nor silently expands the range

### Requirement: Preserve native and autonomous paths
The system SHALL let users continue the same native artifacts through original framework workflows or autonomous orchestration and SHALL provide a safe, explicit handoff between them.

#### Scenario: Continue native planning autonomously
- **WHEN** a user has completed some native planning artifacts and switches to autonomous mode
- **THEN** existing valid artifacts are retained and progression starts at the next required native action

#### Scenario: Return control to the native workflow
- **WHEN** the user selects native mode or hands off an autonomous run
- **THEN** the system stops autonomous dispatch and returns the current usable checkout and original next action without reporting milestone completion
