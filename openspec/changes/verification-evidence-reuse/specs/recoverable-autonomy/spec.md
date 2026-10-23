## MODIFIED Requirements

### Requirement: Revise pending verification without losing accepted work
The system SHALL preview and apply reasoned verification changes to pending work with a compare-and-swap hash. Accepted tasks, native requirements, run identity, scope and Git history SHALL remain unchanged. Revised checks SHALL still execute before completion. Only in-flight work whose verification the revision changes SHALL block application: implement attempts of revised tasks, and repair or audit attempts of revised phases or of any phase for milestone-wide changes. Other in-flight work SHALL continue and SHALL be verified against the revised checks when its receipt is integrated.

#### Scenario: Late command correction
- **WHEN** a later task fails due to incorrect verification argv after earlier tasks were accepted
- **THEN** the host can revise the pending checks and continue the same run without repeating accepted tasks or erasing failure evidence

#### Scenario: Stale or active revision
- **WHEN** the preview becomes stale or work on the revised scope is still owned or submitted
- **THEN** apply is refused without partial modification

#### Scenario: Obsolete verification repair
- **WHEN** a phase command correction supersedes a pending host-verification repair
- **THEN** the obsolete repair is recorded and cleared and corrected checks run before any new completion claim

#### Scenario: Unrelated work is outstanding
- **WHEN** a pending task's checks are revised while another task's implement packet is outstanding
- **THEN** the revision applies, the outstanding packet stays valid and the revised checks reach the revised task's worker
