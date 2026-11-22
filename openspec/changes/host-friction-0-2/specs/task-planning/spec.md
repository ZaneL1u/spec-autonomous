## MODIFIED Requirements

### Requirement: Use dependency and file constraints for parallelism
The system SHALL dispatch concurrent tasks only when their dependencies are integrated and their read/write constraints are compatible; unknown write sets SHALL serialize. Task writes and reads MAY be directory prefixes or globs, and overlap between them SHALL be judged conservatively on the literal prefix. A glob or directory entry SHALL NOT grant edits to native specification context files, which SHALL be named exactly. The Spec Kit `[P]` marker SHALL apply only to Spec Kit; OpenSpec tasks SHALL run in parallel unless linked by `depends_on` or conflicting writes, and planning packets SHALL explain this.

#### Scenario: Parallel marker conflicts with files
- **WHEN** two Spec Kit tasks marked parallel write the same file
- **THEN** they execute serially and the reported plan explains the conflict

#### Scenario: Overlapping glob writes
- **WHEN** one task writes `server/*/migrations/*.py` and another writes `server/app/migrations/0002.py`
- **THEN** the plan is accepted and the two tasks execute serially

#### Scenario: Glob over native context
- **WHEN** a task's writes contain `openspec/changes/**`
- **THEN** the task is not permitted to edit the change's specification or tasks file unless that file is listed exactly

## ADDED Requirements

### Requirement: Plan verify-only tasks
A plan task MAY set `verify_only` when it only runs checks. Plan validation SHALL require such a task to have empty writes and its own verification checks and SHALL reject the plan otherwise. Planning instructions SHALL describe the field and ask every task to declare the files it reads, outside the request key.

#### Scenario: Verify-only task with writes
- **WHEN** a planner returns a verify_only task that also declares writes
- **THEN** the plan is rejected with invalid_plan and a corrected planning packet can be issued

### Requirement: Give planners native task details
Native task parsing SHALL keep the deeper-indented non-checkbox lines under a task, including fenced code, as task details until the next checkbox, heading or dedent, while the task description stays the checkbox line. Planning instructions SHALL ask planners to fold details into task descriptions and turn executable acceptance commands into verification.

#### Scenario: OpenSpec task with acceptance commands
- **WHEN** a tasks.md item lists an indented acceptance command below its checkbox
- **THEN** the planning snapshot carries the command in that task's details
