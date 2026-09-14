---
name: milestone
description: Prepare a milestone roadmap or import existing OpenSpec and Spec Kit work using the host-driven Spec Autonomous capabilities.
---

Use sa_inspect to identify the existing SDD provider and sa_prepare with the user's goal and mode. Native mode produces a roadmap work packet for the host, then hands off to the native flow. Autonomous mode continues returning native planning, implementation and audit packets. Existing work can be selected by milestone_id/change/feature; do not invent a replacement spec framework.

Missing tools are prepared by the npm layer. Inspect or retry readiness through sa_providers status/ensure. If no framework is initialized, use the user's chosen provider with `spec-autonomous init --provider openspec|speckit --agent codex|claude`; do not select a framework silently. Native commands use `spec-autonomous providers exec <provider> -- <arguments>` without requiring global installation.

The host owns fresh agent contexts. Claim each issued packet via work.claim, dispatch through the existing host facility, and pass its structured WorkerResult to sa_apply_result. No CLI agent launcher is available or required. The capability handles roadmap persistence, validation and worktree mechanics.

Use roadmap.get/select/add/insert/import through sa_tools when the user requests structural operations. Supply source hashes for updates and preserve stable phase IDs and dependencies. Do not infer phase ranges before the roadmap exists. Missing material decisions should be surfaced through the host; routine file/Git commands belong to the capability implementation.

Milestone design follows a two-level pipeline inspired by GSD's research/plan phase workflow: the roadmap worker decomposes a goal into stable phase/spec selectors and dependency edges; each phase is then researched and planned from its own native artifacts before execution packets are issued. Treat a phase as the unit of research, context, plan, verification and delivery. Do not collapse unrelated specs into one plan or create a product version for every phase.

Version semantics: the user-facing milestone `id` is a stable project identity and is not a release version. `milestone.toml.revision` is an internal optimistic-concurrency revision incremented when phase membership, dependencies, source selectors or roadmap decisions change. Native source hashes and the run's accepted head bind a prepared plan to the exact source snapshot. A new product release/version is independent and belongs to the repository's package/release process.

When a milestone goal is broad, use the phase split as the first design decision: each phase should map to one OpenSpec change or one Spec Kit feature directory, have explicit `depends_on`, and state its verification boundary. Research/context decisions belong to the phase's derived artifacts; native Markdown remains the requirement authority. Planning then turns each phase's native tasks plus research into executable host packets.
