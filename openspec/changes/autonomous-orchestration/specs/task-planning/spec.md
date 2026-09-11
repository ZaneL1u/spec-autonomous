## Purpose

Provide observable and verifiable task planning behavior for Spec Autonomous users while preserving clear execution and compatibility boundaries.

## ADDED Requirements

### Requirement: Cover the original task scope
The system SHALL produce a validated execution plan that covers every required source task and retains stable source mappings across internal decomposition.

#### Scenario: Decompose a large source task
- **WHEN** a source task is divided into internal tasks
- **THEN** the source task remains incomplete until every required child is integrated and its original acceptance passes

### Requirement: Reject invalid dependency graphs
The system SHALL reject duplicate identities, missing dependencies, cycles, out-of-scope tasks and ambiguous source mappings before dispatch.

#### Scenario: Cyclic plan
- **WHEN** a planner returns mutually dependent tasks
- **THEN** the plan is rejected with the cycle and no worker starts

### Requirement: Use dependency and file constraints for parallelism
The system SHALL dispatch concurrent tasks only when their dependencies are integrated and their read/write constraints are compatible; unknown write sets SHALL serialize.

#### Scenario: Parallel marker conflicts with files
- **WHEN** two Spec Kit tasks marked parallel write the same file
- **THEN** they execute serially and the reported plan explains the conflict

### Requirement: Consume integrated dependency results
The system SHALL prepare a dependent task against an integration revision containing its verified prerequisites.

#### Scenario: Predecessor finishes
- **WHEN** a predecessor is verified and integrated
- **THEN** the next task receives its integrated code and bounded result summary rather than only its success claim

### Requirement: Keep a structured roadmap linked to native specifications
The system SHALL persist versioned milestone and phase declarations in TOML with stable identities, ordered labels, dependencies and native source references, and SHALL retain normative specification content in its original Markdown files.

#### Scenario: Plan a multi-phase milestone
- **WHEN** a goal is decomposed into several roadmap phases
- **THEN** each phase has a unique native change or feature reference and explicit dependencies, while the readable roadmap reflects that topology

#### Scenario: Insert a roadmap phase during execution
- **WHEN** the roadmap changes after a bounded run has started
- **THEN** the system reconciles its revision, preserves stable range endpoints and validates scope and dependencies before scheduling any inserted phase
