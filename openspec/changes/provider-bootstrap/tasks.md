## 1. JS provider installation

- [x] 1.1 Implement pinned provider and uv installation, readiness detection and offline behavior; verify unit tests for reuse, failure, hashes and platform selection.
- [x] 1.2 Implement installation locking and verified receipts; verify concurrent requests, interrupted installs and retry tests.

## 2. Integration

- [x] 2.1 Add providers status/ensure/exec and init bootstrap with collision-safe native scaffolding; verify CLI mock repository e2e and argument forwarding.
- [x] 2.2 Connect CLI/MCP readiness and minimal Rust argv bridge; verify MCP protocol, read-only no-download and provider failure tests.
- [x] 2.3 Update Skills, alpha.3 package metadata and documentation; verify packed assets and installation with lifecycle scripts disabled.
- [x] 2.4 Fix the pause mailbox acknowledgement race exposed by full regression; verify a deterministic coordinator-lock contract test and the concurrent progress/pause e2e without discarding submitted receipts.

## 3. Acceptance

- [x] 3.1 Run real isolated OpenSpec and Spec Kit install/init smoke tests, including missing uv and managed Python; record exact commands and actual platform evidence.
- [x] 3.2 Run full repository regression and strict OpenSpec validation, test JS under Bun, and produce the macOS arm64 npm tarball.
