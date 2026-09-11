---
name: milestone
description: Plan a milestone roadmap through the repository's existing SDD framework, or import existing OpenSpec and Spec Kit work into a milestone.
---

Resolve the user's goal, constraints and native framework with `spec-autonomous detect --json`. Run `spec-autonomous milestone new "<goal>" --mode native` to create a reviewable roadmap and native next action. Use `--mode autonomous` only when the user requested continued autonomous implementation. The CLI will preserve native templates, artifacts and gates.

Use `spec-autonomous roadmap --milestone <id> --format toml` for phase identities and dependencies. Do not replace the user's specifications with TOML: TOML describes orchestration; normative specs and tasks remain native Markdown. Ask only for missing product decisions required to establish scope. Do not guess a framework when detection is ambiguous.
