---
name: autonomous
description: Run a milestone or roadmap phase range autonomously using the repository's existing OpenSpec or Spec Kit workflow. Use for autonomous SDD implementation, including resuming native planning.
---

Use the installed `spec-autonomous` CLI as the state owner. Start with `spec-autonomous detect --json` and `spec-autonomous progress --all-worktrees --json`; select the user's milestone or source without guessing among ambiguous candidates.

For a goal without a milestone, run `spec-autonomous milestone new "<goal>" --mode autonomous`. For an existing milestone, run `spec-autonomous run --milestone <id> --mode autonomous`, preserving requested `--from`, `--to`, `--only` and worker limits. Existing sources can use `--change <id>` or `--feature <path>` with `--framework` when needed.

`from/to` is an inclusive roadmap phase range, not a task number. Keep scope_completed distinct from milestone completed. Do not implement a second loop in chat, manually edit runtime state, or bypass native gates. If the CLI reports needs_input, summarize that specific decision. Preserve existing authorization on resume.

If the runner is missing, inspect `spec-autonomous doctor --json` and help configure the user's existing agent in `.spec-autonomous/config.toml`. Do not silently substitute an SDD framework. Full logs stay on disk; show bounded progress and evidence references. `/auto` is an exact alias of this workflow.
