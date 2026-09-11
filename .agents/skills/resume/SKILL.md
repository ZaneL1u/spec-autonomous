---
name: resume
description: Resume or hand off a paused Spec Autonomous run while preserving its native SDD source, phase bounds, policy and verified progress.
---

Inspect `spec-autonomous status <run-id> --json`, then use `spec-autonomous resume <run-id>` for continued autonomous work. Preserve the selected range and existing authorization. The CLI reconciles source changes, Git intentions and old workers before retrying.

For native handoff, use `spec-autonomous resume <run-id> --mode native` and show the exact returned checkout and native next action. Do not send the user to an outdated original checkout or mark handoff as milestone completion. If source drift or an unknown hook result blocks recovery, report the specific issue; never reset user changes or rerun uncertain side effects manually.
