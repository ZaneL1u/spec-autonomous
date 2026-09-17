## Why

Dogfooding exposed four avoidable rough edges: audit contract failures do not identify extra or missing IDs, cleanup plan hashes do not explain scope changes, cancelled runs with issued work give a long recovery path, and terminal integration worktrees accumulate indefinitely.

## What Changes

- Report exact audit IDs that are missing, unexpected, duplicated, or missing evidence.
- Include cleanup scope and the requested options in stale plan hash errors, with a remediation command.
- Include outstanding request IDs and revoke guidance in source drift recovery errors.
- Add an explicit opt-in cleanup scope for terminal integration worktrees/branches after evidence has been retained and the user confirms it.
- Add an in-repository `playground/` dogfood project and automated end-to-end test starting with OpenSpec init and exercising the auto protocol.

## Capabilities

### New Capabilities
- `dogfood-rough-edges`: actionable diagnostics, explicit terminal integration cleanup, and a checked-in auto playground.

### Modified Capabilities
- `native-lifecycle`: more precise audit and recovery diagnostics.

## Impact

Rust provider/audit, recovery, cleanup and capability schemas; playground scripts and E2E fixtures; docs and alpha.10 release assets.
