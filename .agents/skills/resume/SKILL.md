---
name: resume
description: Resume a host-driven Spec Autonomous run while preserving its native SDD source, bounds, ownership and verified work.
---

Read sa_next and sa_progress for the run, then use sa_prepare with run_id. The capability reconciles persisted receipts and Git intents before preparing further work. Retain the original scope and policy; explicit budget/config changes use prepare's resume options.

The host owns agent sessions. Do not restart claimed or stale packets: inspect the host's session state and acknowledge stopped work through work.revoke before requesting replacement work. Use work.heartbeat for live sessions. A receiving receipt needs the same result resubmitted; completed receipts are idempotent. Use sa_apply_result for new owned results, with the same host session identity.

Legacy runs are read-only and never reactivate their former runner. Inspect their native artifacts and import the desired scope into a new host-driven run. Unknown native hook effects require hook.resolve with explicit outcome evidence. Do not force-reset dirty worktrees or silently widen the selected phase range.
