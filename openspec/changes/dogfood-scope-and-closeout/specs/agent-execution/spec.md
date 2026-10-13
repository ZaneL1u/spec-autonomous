## MODIFIED Requirements

### Requirement: Bound context and validate results
The system SHALL bound coordinator summaries and worker input/results, preserve mandatory constraints, and verify result identity, paths and actual changes before accepting a candidate. A file listed in the assigned task write set SHALL be accepted even when that same path also appears in the snapshot context files. Paths under `.spec-autonomous/` SHALL remain forbidden. Context files that are not in the write set SHALL remain read-only. An empty write set SHALL continue to serialize unknown writers and SHALL NOT treat context files as writable.

#### Scenario: Oversized or mismatched result
- **WHEN** a worker returns an oversized result or the wrong attempt identity
- **THEN** the result is rejected without marking the task complete

#### Scenario: Worker updates an assigned context file
- **WHEN** a worker candidate changes a path that is both a snapshot context file and a member of `task.writes`
- **THEN** the change is in scope and is verified and integrated like any other allowed write

#### Scenario: Worker updates a context file outside the write set
- **WHEN** a worker candidate changes a snapshot context file that is not listed in `task.writes`
- **THEN** the result is rejected as `scope_violation` without integrating the candidate
