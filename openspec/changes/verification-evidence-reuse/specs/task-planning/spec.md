## ADDED Requirements

### Requirement: Extend an accepted plan for appended native tasks
When native tasks are appended to a phase whose plan is already recorded, the system SHALL plan only the uncovered tasks, SHALL append the resulting tasks to the recorded plan after its remaining leaves, and SHALL retain completed tasks. A full replan SHALL happen only when a source task covered by the recorded plan no longer exists.

#### Scenario: Convergence appends a repair task
- **WHEN** a repair round appends one native task to the tracking file
- **THEN** the planner receives only that task plus a summary of the existing plan, and the extended plan keeps every accepted task and its completion state
