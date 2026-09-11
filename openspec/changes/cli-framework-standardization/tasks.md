## 1. Community parser integration

- [x] 1.1 Add Clap metadata and parse-only interfaces; verify they use the native grammar and have no project side effects.
- [x] 1.2 Replace JS manual parsing with Commander and structured provider contexts; verify root/nested help, unknown options, conflicts, global placement, native argv and JSON errors.

## 2. Distribution and compatibility

- [x] 2.1 Pin Commander, update release dependency policy and alpha.4 metadata; verify package contracts and Bun frozen lockfile.
- [x] 2.2 Verify CLI/MCP/provider regression and signal propagation, and update documentation with actual Node/Bun evidence.
- [x] 2.3 Build the macOS arm64 npm tarball and test installed commands with scripts disabled; run the full repository gate and strict OpenSpec validation.
