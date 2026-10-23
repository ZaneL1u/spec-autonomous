## ADDED Requirements

### Requirement: Claim prepared work for the calling host
When `prepare` or `apply-result` carries a host identity, the system SHALL claim every unowned issued request for that host under a distinct derived session identity and SHALL return each packet with its token and owner. Requests owned by another host SHALL NOT be taken over. Follow-up work issued by a receipt SHALL be claimed for the receipt's host the same way when the receipt asks for it with claim_next.

#### Scenario: Host prepares with its identity
- **WHEN** a host calls prepare with host_id and session_id
- **THEN** the returned packets are claimed under sessions derived from that identity and carry their tokens without a separate claim call

### Requirement: Recover ownership without the token
`next` and `request.get` SHALL return a request's token only to the host whose host_id owns it, and `work.revoke` SHALL accept the owning host identity in place of the token together with the stop acknowledgement. Anonymous previews SHALL never expose tokens.

#### Scenario: Host lost its context
- **WHEN** the owning host calls next with its identity after losing the prepare response
- **THEN** it receives the tokens of the requests it owns and can submit or revoke them

### Requirement: Liveness is informational and the lock waits
A lapsed heartbeat SHALL mark a request stale without refusing its owned receipt. The coordinator lock SHALL wait a bounded time before reporting another active coordinator. Claim, batch-claim and heartbeat responses SHALL omit the verification readiness computation.

#### Scenario: Heartbeat lapsed before submission
- **WHEN** an owned request's heartbeat is older than the lease and its receipt arrives
- **THEN** the receipt is verified and integrated normally

#### Scenario: Concurrent host calls
- **WHEN** a receipt arrives while another coordinator call still holds the lock for a moment
- **THEN** the receipt waits for the lock instead of failing with run_already_active

### Requirement: One-step reviewed operations
`run.revise` SHALL apply without a preview hash when no outstanding work is in the revised scope, and `run.cleanup` SHALL apply without a preview hash when neither branches nor the integration are deleted. Destructive scopes SHALL keep the preview and matching plan_hash requirement.

#### Scenario: Reclaim closed worktrees directly
- **WHEN** the host applies run.cleanup with active:true and no delete flags
- **THEN** closed worktrees are removed in that call while refs, ledger and evidence are retained

### Requirement: Discussion cards travel with prepare
When a phase pauses on gray areas, `prepare` SHALL return the discussion cards in its response, and `prepare` with discussion_auto SHALL apply the recommended defaults and continue in the same call.

#### Scenario: Host accepted defaults up front
- **WHEN** prepare is called with discussion_auto and the phase has cards marked requires_user
- **THEN** the recommended options are recorded as decisions and planning continues without a second call

### Requirement: Declare host capacity to the host
Responses that carry work SHALL include host_capacity with the effective concurrency, lease seconds and the host capabilities declared in configuration, so hosts do not self-assess dispatch capacity.

#### Scenario: Configured isolation capability
- **WHEN** the project declares [host.capabilities] isolation = "worktree"
- **THEN** prepare returns host_capacity.capabilities.isolation = "worktree" alongside max_concurrency
