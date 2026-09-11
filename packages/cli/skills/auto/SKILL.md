---
name: auto
description: Alias of autonomous for running an OpenSpec or Spec Kit milestone or phase range with the Spec Autonomous CLI.
---

This is exactly the `autonomous` entry point, with the same selection, budgets and run state. Use `spec-autonomous detect --json` and `spec-autonomous progress --all-worktrees --json`, then `spec-autonomous run --milestone <id> --mode autonomous` with the user's from/to/only flags. A new goal uses `spec-autonomous milestone new "<goal>" --mode autonomous`.

The CLI owns planning, dispatch, verification, repair and recovery. Do not create a separate shortcut loop or change scope for this alias. Surface concrete blockers; resume existing work with `spec-autonomous resume <run-id>`. Keep the user's native SDD files intact and do not infer whole-milestone completion from a completed range.
