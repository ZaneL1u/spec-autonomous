## ADDED Requirements

### Requirement: Reuse verified evidence for identical trees
The system SHALL treat a passing host-run check as evidence for any later check with the same command, relative cwd, runtime environment, preparation hook and checkout class executed against a Git tree whose hash was identical before and after the original run. Reused evidence SHALL reference the original wave, manifest and logs, SHALL record zero measured duration, and SHALL never be derived from a failed, timed-out, cancelled or tree-mutating run. A tree, environment, command or checkout-class difference SHALL always execute the check again. Reuse SHALL be disableable per project.

#### Scenario: Worker and combined verification share a tree
- **WHEN** a worker candidate is based on the current accepted revision so the candidate tree equals the worker tree
- **THEN** combined verification reuses the worker evidence instead of executing the same commands again

#### Scenario: Integration checkout differs from a worker checkout
- **WHEN** the phase gate runs in the integration checkout after a worker passed the same command on an identical tree
- **THEN** the phase gate executes the command because the checkout class differs

#### Scenario: Hooks did not change the accepted revision
- **WHEN** after-implement hooks leave the accepted revision unchanged
- **THEN** no post-hook verification wave runs and the phase gate evidence stands

### Requirement: Reuse a passed audit of an unchanged tree
The system SHALL key each audit batch by phase, scope, integration tree hash and the exact acceptance ids it covers, and SHALL reuse a fully passed audit with the same key instead of dispatching another audit worker. Audits with any failed item SHALL NOT be reused.

#### Scenario: Audit requested again for the same tree
- **WHEN** the phase loop reaches the audit step again while the integration tree and acceptance references are unchanged
- **THEN** the recorded passed audit is applied and no audit packet is issued
