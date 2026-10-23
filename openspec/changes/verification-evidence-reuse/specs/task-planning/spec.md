## ADDED Requirements

### Requirement: Extend an accepted plan for appended native tasks
When native tasks are appended to a phase whose plan is already recorded, the system SHALL plan only the uncovered tasks, SHALL append the resulting tasks to the recorded plan after its remaining leaves, and SHALL retain completed tasks. A full replan SHALL happen only when a source task covered by the recorded plan no longer exists.

#### Scenario: Convergence appends a repair task
- **WHEN** a repair round appends one native task to the tracking file
- **THEN** the planner receives only that task plus a summary of the existing plan, and the extended plan keeps every accepted task and its completion state

### Requirement: Reject inline scripts and defer declared future scripts
Check validation SHALL reject an argv element longer than 1024 bytes or one that embeds an encoded or compressed script payload, for configuration and plans alike. Preflight SHALL treat a script operand or executable that does not exist yet but is covered by the task's or prerequisites' writes as deferred, and SHALL report a missing undeclared script as an error.

#### Scenario: Planner embeds a compressed test suite in argv
- **WHEN** a plan check is `python3 -c` with a base85 and zlib payload
- **THEN** the plan is rejected with inline_script_argv and the planner is told to write the script to a declared file

#### Scenario: Test script written by the same task
- **WHEN** a check runs `python3 tests/check_value.py` and that file is in the task's writes
- **THEN** preflight reports the check as deferred rather than missing

### Requirement: Give planners real environment facts
Planning packets SHALL include actual runtime versions of available executables, package scripts, project manifests, a bounded list of existing test files with their total count, whether common cache directories are ignored, and every earlier rejected candidate for the same planning unit summarised by diagnostic code.

#### Scenario: Second planning attempt after rejections
- **WHEN** two earlier plan candidates for a phase were rejected for a missing executable
- **THEN** the next planner packet lists both rejections with their diagnostic codes and the detected runtime versions
