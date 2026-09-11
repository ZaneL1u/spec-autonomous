---
name: milestone
description: Prepare a milestone roadmap or import existing OpenSpec and Spec Kit work using the host-driven Spec Autonomous capabilities.
---

Use sa_inspect to identify the existing SDD provider and sa_prepare with the user's goal and mode. Native mode produces a roadmap work packet for the host, then hands off to the native flow. Autonomous mode continues returning native planning, implementation and audit packets. Existing work can be selected by milestone_id/change/feature; do not invent a replacement spec framework.

The host owns fresh agent contexts. Claim each issued packet via work.claim, dispatch through the existing host facility, and pass its structured WorkerResult to sa_apply_result. No CLI agent launcher is available or required. The capability handles roadmap persistence, validation and worktree mechanics.

Use roadmap.get/select/add/insert/import through sa_tools when the user requests structural operations. Supply source hashes for updates and preserve stable phase IDs and dependencies. Do not infer phase ranges before the roadmap exists. Missing material decisions should be surfaced through the host; routine file/Git commands belong to the capability implementation.
