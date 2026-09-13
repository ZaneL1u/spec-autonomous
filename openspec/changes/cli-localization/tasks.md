## 1. Locale core

- [x] 1.1 Add shared English / Simplified Chinese catalogs and Rust locale negotiation, fallback and localized Clap help/error rendering; verify locale unit tests and native help/error snapshots.
- [x] 1.2 Add Node locale negotiation, catalog loading and Commander/provider integration; verify explicit override, environment precedence and fallback tests.

## 2. Packaging and protocols

- [x] 2.1 Include locale resources in both npm packages and validate catalog key parity, JSON/MCP stable fields and no provider/project side effects.
- [x] 2.2 Add Node/Bun/native parity tests for root/nested help, human progress, JSON errors and install package output; update documentation.

## 3. Release acceptance

- [x] 3.1 Bump to alpha.6, build the macOS arm64 binary, run the full Rust/JS/E2E/OpenSpec gate and produce a verified package.
- [x] 3.2 Publish the private GitHub Release with the localized binary and verify Git SSH installation under Chinese and English environments.

## 4. Complete the user-visible localization (alpha.7)

- [x] 4.1 Read the OS preferred UI language before LANG defaults while honoring explicit overrides; verify Mac Chinese preferences with English LANG, C locale, missing preferences and CLI payload boundaries.
- [x] 4.2 Translate all help headings, command/option descriptions, setup errors and installation messages; remove agent-launch disclaimers from current product text; verify catalog coverage and concrete init/help/error cases.
- [x] 4.3 Render human fields without changing paths, IDs or user content; preserve stable machine protocols and normal Clap parsing; verify regression tests for metadata --help payload and translated status paths.
- [ ] 4.4 Validate installed Node/Bun CLI and full repository checks, publish a new private alpha.7 with matching binaries, and verify Git installation.
