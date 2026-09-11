---
name: progress
description: Inspect milestone, phase and worker progress across all worktrees of the current Git repository without modifying execution state.
---

Run `spec-autonomous progress --all-worktrees --json`. This enumerates the common Git repository, including the main checkout, managed workers/integration trees and external worktrees. Use `status <run-id> --json` for run details and `roadmap --milestone <id> --format toml` for phase structure.

Report verified progress separately from native checkbox counts. Keep unknown/stale/partial observations visible; do not label unregistered worktrees idle or complete. Count a logical task once across retries. Summarize active work, blockers and next actions with bounded output; do not read full worker logs unless needed for a specific issue. This skill is read-only and never repairs or cleans up worktrees implicitly.
