Each item records a commit already on `release/0.2.0` (base `main` = b5d9317). Contract tests live in `crates/core/tests/`; the release gate for every group is `bun run test:all`.

## 1. Scope-narrowed audits and verification

- [x] 1.1 Re-audit only acceptance items whose inputs changed: audit items may carry `files`; passed items with unchanged reference text and an unaffected diff are carried over with `carried_over`, fully carried batches issue no packet (34b1b5a). Verify with `cargo test -p spec-autonomous-core --locked --test scope_contracts` (`a_repeated_phase_audit_asks_only_for_items_whose_inputs_changed`, `an_item_without_files_or_with_changed_files_is_audited_again`, `bookkeeping_is_only_plans_and_checkbox_or_append_changes_to_tasks_md`).
- [x] 1.2 Let the milestone audit stand on passed phase audit items owned by exactly one phase (1c60316). Verify with `cargo test -p spec-autonomous-core --locked --test scope_contracts a_single_phase_milestone_audits_only_its_own_items`.
- [x] 1.3 Add `[execution] verification_scope` / `milestone_verification_scope` and re-run only the task checks a change reaches, recording `reused_from.scope = "affected"`; ask planners for `reads` (ded1f6d). Verify with `cargo test -p spec-autonomous-core --locked --test scope_contracts` (`checks_of_tasks_the_change_does_not_reach_stand_on_their_last_pass`, `full_scope_reruns_every_task_check_and_affected_milestone_skips_them`, `a_check_whose_owner_declared_no_reads_always_runs_again`).
- [x] 1.4 Run `verify_only` tasks in the coordinator and issue an implement packet only to repair a failure (dc11ff9). Verify with `cargo test -p spec-autonomous-core --locked --test scope_contracts` (`a_passing_verify_only_task_is_accepted_without_a_worker`, `a_failing_verify_only_task_issues_a_repair_with_the_failure_output`).
- [x] 1.5 Reconcile the scope and host-fix structures after merging both branches (70d7097). Verify with `cargo test -p spec-autonomous-core --locked --test scope_contracts` and `cargo test -p spec-autonomous-core --locked --test v02_host_contracts`.
- [x] 1.6 Resolve check path operands under the check's own cwd instead of depending on the whole directory (065af3e). Verify with `cargo test -p spec-autonomous-core --locked --test scope_contracts checks_running_in_a_subdirectory_still_stand_on_their_last_pass`.

## 2. Audit results

- [x] 2.1 Send only failed audit items as converge gaps while the request key still hashes the full audit (95dfacf). Verify with `cargo test -p spec-autonomous-core --locked --test host_friction_contracts converge_instruction_lists_only_failed_audit_items`.
- [x] 2.2 Treat non-contradicting extensions of open-ended specs as passed (`extension:`) and keep them out of converge gaps (524b3f3). Verify with `cargo test -p spec-autonomous-core --locked --test v02_host_contracts` (`extension_evidence_marks_an_item_that_is_never_a_gap`, `an_audit_failing_only_on_extensions_passes_and_mixed_gaps_omit_them`, `converge_lists_real_gaps_and_leaves_extension_items_out`).
- [x] 2.3 Let implement and audit receipts carry `deferred_evidence` and complete runs with `deferred_evidence_pending` (19745cf). Verify with `cargo test -p spec-autonomous-core --locked --test v02_host_contracts` (`implement_receipts_carry_deferred_evidence_into_views_and_progress`, `audit_items_may_defer_live_evidence_and_the_run_completes_with_follow_ups`, `receipts_without_deferred_evidence_keep_their_bytes`).
- [x] 2.4 Give audit packets a per-check digest of execution evidence (46b4c2a). Verify with `cargo test -p spec-autonomous-core --locked --test field_report_contracts audit_packets_list_each_check_with_command_revision_stage_and_reuse`.

## 3. Worker checkouts and packets

- [x] 3.1 Add `[execution] init_submodules` and `[hooks.worker_prepare]` for writing checkouts, with `worker_prepare_failed` and doctor hints for unbound prepare scripts (70db4db). Verify with `cargo test -p spec-autonomous-core --locked --test host_friction_contracts` (`init_submodules_fills_worker_and_integration_checkouts`, `worker_prepare_runs_in_each_writing_checkout_before_issue`, `failed_worker_preparation_issues_nothing_and_recovers_after_a_fix`, `doctor_warns_about_unbound_prepare_scripts_and_uninitialized_submodules`).
- [x] 3.2 Prepare read-only planning and audit checkouts too, controlled by `[execution] prepare_read_only` (default true) (97c247a). Verify with `cargo test -p spec-autonomous-core --locked --test field_report_contracts` (`read_only_planning_and_audit_checkouts_run_worker_prepare_and_stay_reusable`, `a_hook_that_changes_tracked_files_fails_preparation_of_a_read_only_checkout`, `prepare_read_only_can_be_turned_off_and_is_omitted_while_on`).
- [x] 3.3 Warn about submodules until worker checkouts receive them (e079991). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts doctor_warns_about_submodules_until_workers_get_them`.
- [x] 3.4 Remove clean worktrees whose submodules were initialized (93a55a3). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts cleanup_removes_clean_checkouts_with_initialized_submodules`.
- [x] 3.5 Add `[execution] worktree_root` (`git` default, `sibling`, absolute) with `worktree_root_notice` / `worktree_root_unavailable` (447aa79). Verify with `cargo test -p spec-autonomous-core --locked --test v02_host_contracts` (`worktree_root_config_is_validated_and_omitted_while_default`, `sibling_worktree_root_puts_worker_checkouts_outside_git_and_reclaims_them`, `existing_runs_keep_their_worktree_root_and_doctor_explains_the_default`, `absolute_worktree_roots_are_per_repository_and_never_inside_the_checkout`).
- [x] 3.6 Keep worker prompts to a summary and trim `input.json` by work kind without changing request keys (a71242f). Verify with `cargo test -p spec-autonomous-core --locked --test host_friction_contracts` (`implement_prompt_is_a_small_summary_of_a_trimmed_request`, `trimming_unwraps_native_apply_json_and_dedupes_planning_failures`).
- [x] 3.7 Keep native task sub-items as `SourceTask.details` (ea25886). Verify with `cargo test -p spec-autonomous-core --locked --test provider_contracts` (`task_details_keep_indented_sub_items_code_and_stop_at_the_next_task`, `a_task_inside_a_detail_fence_still_counts_for_openspec`) and `cargo test -p spec-autonomous-core --locked --test host_protocol_contracts planner_packets_carry_native_sub_items_as_task_details`.
- [x] 3.8 Hand a blocked or failed attempt's changes to the re-dispatched implement as `previous_candidate` (05bd088). Verify with `cargo test -p spec-autonomous-core --locked --test ledger_reclaim_contracts redispatched_implement_inherits_the_blocked_attempts_changes`.

## 4. Checks and verification environment

- [x] 4.1 Run bare `openspec` checks through the managed provider command (384f1d9). Verify with `cargo test -p spec-autonomous-core --locked --test verification_preflight_contracts managed_openspec_command_backs_bare_openspec_checks`.
- [x] 4.2 Tell planners and workers how a bare `openspec` check runs (`managed_commands`, `TOOLS:`) and probe `pnpm`, `uv`, `openspec` (58f7e35). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts planners_and_workers_learn_how_a_mapped_openspec_runs`.
- [x] 4.3 Keep the planning candidate when only the verification environment is missing (2cda119). Verify with `cargo test -p spec-autonomous-core --locked --test host_protocol_contracts environment_only_preflight_failure_keeps_the_planning_candidate`.
- [x] 4.4 Let checks declare non-secret `env` variables (1a37c33). Verify with `cargo test -p spec-autonomous-core --locked --test v02_host_contracts` (`check_env_names_are_validated_and_secrets_refused`, `a_task_check_runs_with_its_env_and_logs_only_the_names`, `a_plan_with_a_reserved_check_env_is_rejected`, `planning_instruction_documents_check_env`).
- [x] 4.5 Say which check failed and why, with redacted output tails (bfd29f0). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts a_failed_check_names_its_command_exit_and_redacted_output`.
- [x] 4.6 Add opt-in `[execution] baseline_checks` and report checks that already fail on the accepted baseline (4a4fd94). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts baseline_checks_report_failures_that_predate_the_plan`.

## 5. Interruptions, budgets and progress

- [x] 5.1 Keep a candidate whose verification was cancelled (99d6a23). Verify with `cargo test -p spec-autonomous-core --locked --test host_friction_contracts a_cancelled_integration_check_keeps_the_candidate_and_counts_no_failure`.
- [x] 5.2 Keep a candidate interrupted before or between integration checks (2a4848a). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts an_interruption_between_integration_checks_keeps_the_candidate`.
- [x] 5.3 Keep run extensions across `reload_config` and size `budget_exhausted` stops (59a9833). Verify with `cargo test -p spec-autonomous-core --locked --test host_friction_contracts reload_config_keeps_earlier_extensions_and_budget_stops_say_how_much_to_add`.
- [x] 5.4 Spend the run budget on active time only (65628b2). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts` (`the_run_budget_counts_active_time_only`, `legacy_runs_resume_from_recorded_time_and_cap_hints_name_unsafe_budgets`).
- [x] 5.5 Keep `no_progress` blockers to the latest failure per task (edee054). Verify with `cargo test -p spec-autonomous-core --locked --test host_friction_contracts no_progress_blocker_names_only_the_latest_failure_per_task`.

## 6. Write-set grants

- [x] 6.1 Let `run.revise` (`task_writes`) and blocked workers (`requested_writes`) append to a task's write set (8e71062). Verify with `cargo test -p spec-autonomous-core --locked --test host_friction_contracts` (`run_revise_appends_writes_and_the_redispatched_packet_carries_them`, `reconcile_plan_keeps_revised_writes_after_replanning`, `blocked_receipt_with_requested_writes_extends_the_scope_and_redispatches`, `unsafe_requested_writes_stop_for_review_with_a_revise_example`).
- [x] 6.2 Grant only literal files automatically and never a parent or pattern of native context (bb6b079). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts` (`a_parent_directory_or_pattern_of_native_context_is_never_granted`, `blocked_workers_get_only_literal_file_paths_without_review`).
- [x] 6.3 Start a new retry epoch on an automatic grant and bound automatic grants to ten paths per task (3c1cb81). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts an_automatic_grant_starts_a_new_retry_epoch_with_a_bounded_total`.
- [x] 6.4 Document glob and directory write sets and keep native context literal in planning (0bc356a). Verify with `cargo test -p spec-autonomous-core --locked --test v02_host_contracts` (`planning_guidance_allows_glob_and_directory_writes`, `glob_writes_cover_their_matches_and_conflict_conservatively`, `a_planned_glob_write_is_accepted_and_serializes_overlapping_tasks`).
- [x] 6.5 Tell planners how OpenSpec tasks parallelize, outside the request key (9b94dc8). Verify with `cargo test -p spec-autonomous-core --locked --test field_report_contracts planning_guidance_explains_openspec_parallelism_outside_the_keyed_instruction`.

## 7. Delivery

- [x] 7.1 List what makes a checkout dirty in `dirty_checkout` errors (6669ebb). Verify with `cargo test -p spec-autonomous-core --locked --test runtime_contracts dirty_checkout_errors_name_modified_untracked_and_submodule_paths`.
- [x] 7.2 Deliver past untracked files that do not collide with the delivered diff, for `ff-original` and the archive fast-forward (1755c56). Verify with `cargo test -p spec-autonomous-core --locked --test field_report_contracts` (`delivery_passes_untracked_files_outside_the_delivered_diff`, `delivery_names_untracked_collisions_and_tracked_changes_that_block_it`, `fast_forward_blockers_compare_untracked_files_with_the_target_diff`).

## 8. Host integration

- [x] 8.1 Warn about project configuration drift and show hooks in full views (2dd60a9). Verify with `cargo test -p spec-autonomous-core --locked --test host_friction_contracts source_config_drift_is_reported_and_full_views_show_hooks`.
- [x] 8.2 Accept `--reload-config`, `--extend-seconds` and `--max-attempts` on `prepare --run-id` (bd89b8f). Verify with `cargo test -p spec-autonomous-cli --locked prepare_exposes_run_resume_options_only_with_a_run_id`.
- [x] 8.3 Name `prepare --run-id <id>` in blockers that ask to continue a run (a03ab48). No dedicated contract; verify with `bun run test:all`.
- [x] 8.4 Resume with `prepare --run-id` in the skills and explain environment stops (8dbe6cf). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts skills_name_run_id_resumes_and_environment_blockers`.
- [x] 8.5 Name hidden capabilities and explain packet limits (af58d7a). Verify with `cargo test -p spec-autonomous-core --locked --test mcp_contracts default_catalog_names_hidden_capabilities_and_revise_items_have_a_schema` and `cargo test -p spec-autonomous-core --locked --test host_protocol_contracts packet_limits_say_the_cli_does_not_enforce_host_worker_time`.
- [x] 8.6 Apply `discussion_auto` when the blocker carries the `needs_input:` prefix (5516907). No dedicated contract; verify with `bun run test:all`.
- [x] 8.7 Write an MCP command hosts can start and let doctor check it (c5b364f). Verify with `cargo test -p spec-autonomous-core --locked --test v02_host_contracts` (`mcp_launch_command_prefers_path_then_an_absolute_launcher`, `doctor_reports_mcp_commands_that_cannot_start`), `node --test packages/cli/test/launcher.test.mts` and `bun run test:e2e` (`init --mcp writes an absolute launcher when spec-autonomous is not on PATH`).

## 9. Ledger

- [x] 9.1 Move the ledger to schema 5 with a backup on the first writable open, without rewriting rows (4e60149). Verify with `cargo test -p spec-autonomous-core --locked --test release_hardening_contracts ledger_schema_5_upgrades_a_schema_4_ledger_without_losing_rows`.

## 10. Record

- [x] 10.1 Record these behavior changes in this OpenSpec change. Verify with `openspec validate host-friction-0-2 --strict --no-interactive` and `openspec validate --all --strict --no-interactive`.
