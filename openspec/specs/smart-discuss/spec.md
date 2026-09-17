# smart-discuss Specification

## Purpose
定义阶段规划前灰区问题的确定性呈现、选择持久化与受限自动决策协议，使宿主能够通过稳定卡片明确验证和排序策略，并在来源变化或选择冲突时安全停止。
## Requirements
### Requirement: Offer clickable discussion cards
The system SHALL expose deterministic phase gray areas as cards with one recommended option, alternatives, rationale and impact, using stable IDs.

#### Scenario: Phase has gray areas
- **WHEN** discussion.next is requested before planning a phase
- **THEN** it returns cards and marks each as unresolved or decided without starting work or changing files.

### Requirement: Apply selected discussion decisions
The system SHALL accept selected card options with source CAS, persist them idempotently, and expose them to subsequent planning.

#### Scenario: Click recommended option
- **WHEN** the host applies a recommended option with the current source hash
- **THEN** the decision is stored with provenance and the next plan view includes it.

#### Scenario: Stale or conflicting choice
- **WHEN** source hash changed or a locked card receives a different option
- **THEN** application fails with an actionable conflict and leaves prior decisions unchanged.

### Requirement: Auto mode is bounded
The system SHALL apply only recommended options in auto mode and report unresolved cards without pretending they were decided.

#### Scenario: Unresolved card
- **WHEN** auto is requested and a card lacks a recommendation
- **THEN** it remains unresolved and the result lists it as requiring user choice.
