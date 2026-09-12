## 1. Locale core

- [x] 1.1 Add shared English / Simplified Chinese catalogs and Rust locale negotiation, fallback and localized Clap help/error rendering; verify locale unit tests and native help/error snapshots.
- [x] 1.2 Add Node locale negotiation, catalog loading and Commander/provider integration; verify explicit override, environment precedence and fallback tests.

## 2. Packaging and protocols

- [x] 2.1 Include locale resources in both npm packages and validate catalog key parity, JSON/MCP stable fields and no provider/project side effects.
- [x] 2.2 Add Node/Bun/native parity tests for root/nested help, human progress, JSON errors and install package output; update documentation.

## 3. Release acceptance

- [x] 3.1 Bump to alpha.6, build the macOS arm64 binary, run the full Rust/JS/E2E/OpenSpec gate and produce a verified package.
- [x] 3.2 Publish the private GitHub Release with the localized binary and verify Git SSH installation under Chinese and English environments.
