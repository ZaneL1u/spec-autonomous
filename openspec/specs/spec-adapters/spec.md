# spec-adapters Specification

## Purpose
Provide observable and verifiable spec adapters behavior for Spec Autonomous users while preserving clear execution and compatibility boundaries.
## Requirements
### Requirement: Preserve existing specification sources
The system SHALL import a selected OpenSpec change or Spec Kit feature without requiring migration of its specification format and SHALL distinguish framework detection, planning readiness and execution support.

#### Scenario: Inspect an incomplete project
- **WHEN** a supported framework is detected but required planning artifacts are missing
- **THEN** inspect reports missing artifacts and the next native planning action, while implementation dispatch waits for the required planning gates

#### Scenario: Explicitly skipped OpenSpec artifact
- **WHEN** an OpenSpec change legitimately declares skip_specs and reports its specs artifact as skipped
- **THEN** the skipped artifact satisfies its dependency without forced spec generation

### Requirement: Resolve source selection explicitly
The system SHALL require an unambiguous source selection and SHALL bind the run to canonical project and feature or change paths within its supported scope.

#### Scenario: Multiple candidates or an external root
- **WHEN** selection is ambiguous or the upstream resolves an unsupported external store
- **THEN** the system reports candidates or a capability error without guessing or writing outside the selected project

### Requirement: Respect framework contracts
The system SHALL preserve source-specific task semantics, planning constraints, mandatory hooks and checklist gates, and SHALL report unsupported required capabilities before execution.

#### Scenario: Unsupported mandatory hook
- **WHEN** the selected Spec Kit feature requires a hook the adapter cannot execute
- **THEN** execution is blocked with the specific missing capability and the checklist remains unchanged

### Requirement: Write back without overwriting edits
The system SHALL update a source task only after all mapped work is verified and integrated, using unique task identity and source revision checks, and SHALL preserve unrelated text and formatting.

#### Scenario: Source task changes during a run
- **WHEN** a task is edited or no longer maps uniquely before completion writeback
- **THEN** writeback is rejected and the run requests replanning while preserving the edit

### Requirement: Reuse native planning contracts
The system SHALL detect and depend on the user's selected repository SDD provider, orchestrate its resolved stages, templates, instructions and required gates, and retain the original specification files and workflows without requiring a replacement framework.

#### Scenario: Missing native planning artifact
- **WHEN** autonomous mode reaches a missing artifact with a supported native generation contract
- **THEN** a fresh planning agent produces it in the native location and the system validates the result before advancing

#### Scenario: No SDD provider is configured
- **WHEN** autonomous is requested but no supported repository SDD provider is available
- **THEN** the system reports setup or selection guidance and does not silently create a private replacement workflow

#### Scenario: Remove the orchestration tool
- **WHEN** the user uninstalls the tool's command and skill bindings
- **THEN** the original framework artifacts remain usable through its native workflows

### Requirement: Expose structured Markdown with provenance
The CLI SHALL expose supported Markdown frontmatter, headings, requirements, scenarios and task fields as structured data with source path, content hash, source span, parser profile and diagnostics, while preserving native Markdown as the source of truth.

#### Scenario: Read an existing native tasks document
- **WHEN** the user queries task metadata through the CLI
- **THEN** the response contains supported IDs, checkbox and phase/story fields with source references, without rewriting the document or executing repository scripts

#### Scenario: Ambiguous Markdown structure
- **WHEN** parsing encounters duplicate identities or unsupported structural metadata
- **THEN** the CLI reports the ambiguity instead of inventing task state or converting the specification into a new format
