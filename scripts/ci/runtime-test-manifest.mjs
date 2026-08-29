const TAGS = Object.freeze({
  parallelSafe: "runtime:parallel-safe",
  serial: "runtime:serial",
  stress: "runtime:stress",
  env: "runtime:env",
  quarantine: "runtime:quarantine",
  stressSkip: "runtime-stress:skip",
  stressSerialized: "runtime-stress:serialized",
  stressContinuation: "runtime-stress:continuation",
  envParallel: "runtime-env:parallel",
});

const ENV_PARALLEL_TAGS = Object.freeze([TAGS.env, TAGS.envParallel, TAGS.quarantine]);
const SERIALIZED_TAGS = Object.freeze([
  TAGS.stress,
  TAGS.stressSkip,
  TAGS.stressSerialized,
  TAGS.serial,
  TAGS.quarantine,
]);
const CONTINUATION_TAGS = Object.freeze([
  TAGS.stress,
  TAGS.stressSkip,
  TAGS.stressContinuation,
  TAGS.serial,
  TAGS.quarantine,
]);
const ADMISSION_PREFIX =
  "main_internal_tests::runtime_proxy_selection_and_pressure::admission::";

export const RUNTIME_TEST_TAGS = TAGS;

export const RUNTIME_SMOKE_TESTS = [
  {
    label: "log-json",
    filter: "runtime_proxy_log_to_path_preserves_valid_json_format",
  },
  {
    label: "doctor-markers-json",
    filter: "runtime_doctor_json_value_includes_selection_markers",
  },
  {
    label: "header-preservation",
    package: "prodex-runtime-proxy",
    filter: "request_header_skip_list_preserves_codex_metadata_headers",
  },
  {
    label: "selection-hard-affinity",
    filter: "response_selection_preserves_bound_previous_response_affinity_despite_quota",
  },
  {
    label: "stale-continuation-guard",
    filter: "websocket_previous_response_not_found_requires_stale_continuation_without_turn_state",
  },
  {
    label: "websocket-local-pressure",
    filter: "websocket_local_pressure_connect_error_does_not_mark_profile_transport_failure",
  },
  {
    label: "tuning-snapshot",
    filter: "runtime_tuning_snapshot_reports_effective_policy_and_env_values",
  },
];

export const RUNTIME_CI_WORKFLOW_SHARDS = [
  {
    suite: "root",
    label: "root proxy helpers",
    filters: [
      {
        id: "root-broker",
        filter: "main_internal_tests::info_and_broker::runtime_proxy_broker_",
        label: "broker",
      },
      {
        id: "root-log-paths",
        filter: "main_internal_tests::info_and_broker::runtime_proxy_log_paths_",
        label: "log-paths",
      },
      {
        id: "root-endpoint-child",
        filter: "main_internal_tests::runtime_broker_tuning::runtime_proxy_endpoint_child_",
        label: "endpoint-child",
      },
      {
        id: "root-claude-launch",
        filter: "main_internal_tests::claude_launch::runtime_proxy_claude_launch_",
        label: "claude-launch-root",
      },
      {
        id: "root-claude-optional-tool",
        filter: "main_internal_tests::claude_launch::missing_external_caveman_",
        label: "claude-optional-tool-root",
      },
    ],
  },
  {
    suite: "selection",
    label: "selection and quota",
    filters: [
      {
        id: "selection",
        filter: "main_internal_tests::runtime_proxy_selection_and_pressure::selection::",
        label: "selection",
      },
    ],
  },
  {
    suite: "rotation",
    label: "rotation and affinity",
    filters: [
      {
        id: "rotation",
        filter: "main_internal_tests::runtime_proxy_selection_and_pressure::rotation::",
        label: "rotation",
      },
    ],
  },
  {
    suite: "state",
    label: "state persistence",
    filters: [
      {
        id: "state",
        filter: "main_internal_tests::runtime_proxy_selection_and_pressure::state::",
        label: "state",
      },
    ],
  },
  {
    suite: "admission-core",
    label: "admission core",
    filters: [
      {
        id: "admission-compact",
        filter: `${ADMISSION_PREFIX}compact::`,
        label: "admission-compact",
      },
      {
        id: "admission-continuation-store",
        filter: `${ADMISSION_PREFIX}continuation_store::`,
        label: "admission-continuation-store",
      },
      {
        id: "admission-doctor-summary",
        filter: `${ADMISSION_PREFIX}doctor_summary::`,
        label: "admission-doctor-summary",
      },
      {
        id: "admission-pressure-budget",
        filter: `${ADMISSION_PREFIX}pressure_budget::`,
        label: "admission-pressure-budget",
      },
      {
        id: "admission-turn-state",
        filter: `${ADMISSION_PREFIX}turn_state::`,
        label: "admission-turn-state",
      },
      {
        id: "admission-rotation-matrix",
        filter: `${ADMISSION_PREFIX}rotation_matrix::`,
        label: "admission-rotation-matrix",
      },
    ],
  },
  {
    suite: "admission-affinity",
    label: "admission guards and affinity",
    filters: [
      {
        id: "admission-cli-mount",
        filter: `${ADMISSION_PREFIX}cli_mount::`,
        label: "admission-cli-mount",
      },
      {
        id: "admission-guards",
        filter: `${ADMISSION_PREFIX}guards::`,
        label: "admission-guards",
      },
      {
        id: "admission-pre-send",
        filter: `${ADMISSION_PREFIX}pre_send::`,
        label: "admission-pre-send",
      },
      {
        id: "admission-previous-response",
        filter: `${ADMISSION_PREFIX}previous_response::`,
        label: "admission-previous-response",
      },
      {
        id: "admission-response-affinity",
        filter: `${ADMISSION_PREFIX}response_affinity::`,
        label: "admission-response-affinity",
      },
      {
        id: "admission-sse-tap",
        filter: `${ADMISSION_PREFIX}sse_tap::`,
        label: "admission-sse-tap",
      },
    ],
  },
  {
    suite: "health",
    label: "health and pressure",
    filters: [
      {
        id: "health",
        filter: "main_internal_tests::runtime_proxy_selection_and_pressure::health::",
        label: "health",
      },
      {
        id: "pressure",
        filter: "main_internal_tests::runtime_proxy_selection_and_pressure::pressure::",
        label: "pressure",
      },
    ],
  },
  {
    suite: "persistence",
    label: "persisted backoff selection",
    filters: [
      {
        id: "persistence",
        filter: "main_internal_tests::runtime_proxy_selection_and_pressure::persistence::",
        label: "persistence",
      },
    ],
  },
  {
    suite: "doctor-summary-guidance",
    label: "doctor summary and guidance",
    filters: [
      {
        id: "doctor-summary-fields",
        filter: "main_internal_tests::runtime_proxy_selection_and_pressure::doctor::summary_fields::",
        label: "summary-fields",
      },
      {
        id: "doctor-finalize-guidance",
        filter: "main_internal_tests::runtime_proxy_selection_and_pressure::doctor::finalize_guidance::",
        label: "finalize-guidance",
      },
    ],
  },
  {
    suite: "doctor-state-runtime",
    label: "doctor state runtime",
    filters: [
      {
        id: "doctor-state-broker-binary",
        filter:
          "main_internal_tests::runtime_proxy_selection_and_pressure::doctor::state_collect::runtime_doctor_collect_state_flags_runtime_broker_binary_mismatch",
        label: "state-broker-binary",
      },
    ],
  },
  {
    suite: "doctor-state-registry",
    label: "doctor state registry",
    filters: [
      {
        id: "doctor-state-dead-registry",
        filter:
          "main_internal_tests::runtime_proxy_selection_and_pressure::doctor::state_collect::runtime_doctor_collect_state_surfaces_dead_broker_registry_and_stale_leases",
        label: "state-dead-registry",
      },
      {
        id: "doctor-state-unreachable-health",
        filter:
          "main_internal_tests::runtime_proxy_selection_and_pressure::doctor::state_collect::runtime_doctor_collect_state_surfaces_unreachable_live_broker_health",
        label: "state-unreachable-health",
      },
    ],
  },
  {
    suite: "doctor-state-persistence",
    label: "doctor state persistence",
    filters: [
      {
        id: "doctor-state-orphans",
        filter:
          "main_internal_tests::runtime_proxy_selection_and_pressure::doctor::state_collect::collect_orphan_managed_profile_dirs_ignores_tracked_and_fresh_dirs",
        label: "state-orphans",
      },
      {
        id: "doctor-state-persisted",
        filter:
          "main_internal_tests::runtime_proxy_selection_and_pressure::doctor::state_collect::runtime_doctor_state_collects_persisted_degradation_and_orphans",
        label: "state-persisted",
      },
    ],
  },
  {
    suite: "incidents",
    label: "incidents",
    filters: [
      {
        id: "incidents",
        filter: "main_internal_tests::runtime_proxy_selection_and_pressure::incidents::",
        label: "incidents",
      },
    ],
  },
  {
    suite: "runtime-backend-parser",
    label: "runtime backend parser",
    filters: [
      {
        id: "runtime-backend-parser",
        filter: "main_internal_tests::runtime_proxy_backend::request_parsing::",
        label: "request-parsing",
      },
    ],
  },
  {
    suite: "continuation-http-precommit-transport",
    label: "continuation http precommit transport",
    filters: [
      {
        id: "continuation-http-precommit-transport",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_precommit_transport_",
        label: "precommit-transport",
      },
    ],
  },
  {
    suite: "continuation-http-followups-affinity",
    label: "continuation http followups affinity",
    filters: [
      {
        id: "continuation-http-empty-session",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_empty_session_previous_response_does_not_fresh_fallback",
        label: "empty-session",
      },
      {
        id: "continuation-http-previous-response",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_message_followup_previous_response_does_not_fresh_fallback",
        label: "previous-response",
      },
      {
        id: "continuation-http-session-header",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_message_followup_with_session_header_does_not_fresh_fallback",
        label: "session-header",
      },
    ],
  },
  {
    suite: "continuation-http-followups-rotation",
    label: "continuation http followups rotation",
    filters: [
      {
        id: "continuation-http-fresh-after-usage",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_fresh_request_reaches_later_profile_after_usage_limit_chain",
        label: "fresh-after-usage",
      },
      {
        id: "continuation-http-quota-pool-exhaustion",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_quota_tries_every_profile_before_final_429",
        label: "quota-pool-exhaustion",
      },
      {
        id: "continuation-http-fresh-sse-preoutput-quota",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_fresh_sse_quota_after_output_item_added_rotates_before_model_output",
        label: "fresh-sse-preoutput-quota",
      },
      {
        id: "continuation-http-session-quota",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_message_followup_with_session_quota_does_not_rotate_or_fresh_fallback",
        label: "session-quota",
      },
      {
        id: "continuation-http-restarted-session-quota",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_restarted_session_only_affinity_rotates_after_quota",
        label: "restarted-session-quota",
      },
      {
        id: "continuation-http-stale-session",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_stale_session_binding_rotates_as_fresh",
        label: "stale-session",
      },
      {
        id: "continuation-http-transport-session-rotation",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_transport_backoff_rotation_rebinds_soft_session",
        label: "transport-session-rotation",
      },
      {
        id: "continuation-http-transient-recovery",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_waits_for_transient_profiles_then_starts_a_new_sweep",
        label: "transient-recovery",
      },
    ],
  },
  {
    suite: "continuation-http-followups-metadata",
    label: "continuation http followups metadata",
    filters: [
      {
        id: "continuation-http-turn-metadata",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_message_followup_with_turn_metadata_session_does_not_fresh_fallback",
        label: "turn-metadata",
      },
      {
        id: "continuation-http-resume-metadata",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_resume_continuation_preserves_metadata_headers_and_affinity",
        label: "resume-metadata",
      },
      {
        id: "continuation-http-memory-consolidation-metadata",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_memory_metadata::runtime_proxy_http_memory_consolidation_metadata_survives_precommit_rotation",
        label: "memory-consolidation-metadata",
      },
      {
        id: "continuation-http-journal-restart",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_restart_recovers_previous_response_affinity_from_journal",
        label: "journal-restart",
      },
    ],
  },
  {
    suite: "continuation-http-tool-compact",
    label: "continuation http tool and compact",
    filters: [
      {
        id: "continuation-http-tool-compact",
        filter: "main_internal_tests::runtime_proxy_continuations::http_tool_and_compact::",
        label: "http-tool-and-compact",
      },
      {
        id: "continuation-http-compact-transparency",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_compact_transparency::",
        label: "http-compact-transparency",
      },
    ],
  },
  {
    suite: "continuation-http-backend-passthrough",
    label: "continuation http backend passthrough",
    filters: [
      {
        id: "continuation-http-backend-passthrough",
        filter: "main_internal_tests::runtime_proxy_continuations::http_backend_passthrough::",
        label: "http-backend-passthrough",
      },
    ],
  },
  {
    suite: "continuation-websocket-precommit",
    label: "continuation websocket precommit",
    filters: [
      {
        id: "continuation-websocket-precommit",
        filter: "main_internal_tests::runtime_proxy_continuations::websocket_precommit::",
        label: "websocket-precommit",
      },
      {
        id: "continuation-websocket-pool-exhaustion",
        filter:
          "main_internal_tests::runtime_proxy_continuations::websocket_pool_exhaustion::",
        label: "websocket-pool-exhaustion",
      },
      {
        id: "continuation-websocket-recovery",
        filter: "main_internal_tests::runtime_proxy_continuations::websocket_recovery::",
        label: "websocket-recovery",
      },
    ],
  },
  {
    suite: "continuation-post-commit",
    label: "continuation post commit",
    filters: [
      {
        id: "continuation-post-commit",
        filter: "main_internal_tests::runtime_proxy_continuations::post_commit::",
        label: "post-commit",
      },
    ],
  },
  {
    suite: "anthropic-launch",
    label: "anthropic launch",
    filters: [
      {
        id: "anthropic-lane-and-launch",
        filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::lane_and_launch::",
        label: "lane-and-launch",
      },
      {
        id: "anthropic-launch-config",
        filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::launch_config::",
        label: "launch-config",
      },
    ],
  },
  {
    suite: "anthropic-request",
    label: "anthropic request translation",
    filters: [
      {
        id: "anthropic-request-translation",
        filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::request_translation::",
        label: "request-translation",
      },
    ],
  },
  {
    suite: "anthropic-response",
    label: "anthropic response translation",
    filters: [
      {
        id: "anthropic-response-translation",
        filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::response_translation::",
        label: "response-translation",
      },
    ],
  },
  {
    suite: "anthropic-runtime",
    label: "anthropic runtime behavior",
    filters: [
      {
        id: "anthropic-runtime-behavior",
        filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::runtime_proxy_behavior::",
        label: "runtime-behavior",
      },
    ],
  },
];

export const RUNTIME_CI_BROAD_SHARD_FILTERS = RUNTIME_CI_WORKFLOW_SHARDS.flatMap(
  (shard) => shard.filters,
);

export const RUNTIME_STRESS_DEFAULT_WEIGHT_SECONDS = 1;

// Static duration hints keep broad runtime-stress shards balanced without
// depending on external CI telemetry at run time. Unknown tests use the default.
// Update from saved CI duration telemetry with:
// node scripts/ci/github-job-durations.mjs --runtime-stress-calibration --write-runtime-stress-hints < ci-job-durations.json
export const RUNTIME_STRESS_WEIGHT_HINTS = Object.freeze([
  {
    filter: "main_internal_tests::runtime_proxy_continuations::",
    weightSeconds: 5,
  },
  {
    filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::runtime_proxy_behavior::",
    weightSeconds: 5,
  },
  {
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::admission::compact::",
    weightSeconds: 5,
  },
  {
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::rotation::continuation_cleanup::",
    weightSeconds: 4,
  },
  {
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::doctor::",
    weightSeconds: 3,
  },
  {
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::state::",
    weightSeconds: 3,
  },
  {
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::persistence::",
    weightSeconds: 2,
  },
  {
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::health::",
    weightSeconds: 2,
  },
  {
    filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::launch_config::",
    weightSeconds: 2,
  },
  {
    filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::request_translation::",
    weightSeconds: 2,
  },
  {
    filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::response_translation::",
    weightSeconds: 2,
  },
  {
    name: "runtime_doctor_fields_surface_queue_lag_and_failure_classes",
    weightSeconds: 6,
  },
  {
    name: "runtime_doctor_json_value_includes_selection_markers",
    weightSeconds: 6,
  },
  {
    name: "runtime_doctor_summary_counts_recent_runtime_markers",
    weightSeconds: 7,
  },
  {
    name: "runtime_doctor_state_collects_persisted_degradation_and_orphans",
    weightSeconds: 9,
  },
  {
    name: "remove_all_profiles_clears_state_and_continuation_sidecars",
    weightSeconds: 7,
  },
  {
    name: "previous_response_release_preserves_session_and_compact_session_lineage_for_compact_followups",
    weightSeconds: 6,
  },
  {
    name: "runtime_state_save_scheduler_persists_latest_snapshot",
    weightSeconds: 4,
  },
  {
    name: "translate_runtime_anthropic_messages_request_maps_tools_and_tool_results",
    weightSeconds: 4,
  },
  {
    name: "translate_runtime_anthropic_messages_request_keeps_versioned_builtin_client_tools",
    weightSeconds: 6,
  },
  {
    name: "perform_prodex_cleanup_removes_safe_local_artifacts",
    weightSeconds: 5,
  },
  {
    name: "runtime_affinity_touch_lookups_do_not_requeue_persistence_before_interval",
    weightSeconds: 3,
  },
  {
    name: "perform_prodex_cleanup_deduplicates_profiles_by_email",
    weightSeconds: 6,
  },
  {
    name: "runtime_previous_response_not_found_decision_matrix_stays_consistent",
    weightSeconds: 8,
  },
  {
    name: "auto_runtime_housekeeping_removes_runtime_garbage_without_touching_user_state",
    weightSeconds: 4,
  },
  {
    name: "runtime_proxy_anthropic_messages_retries_tool_result_transcript_on_another_profile",
    weightSeconds: 5,
  },
  {
    name: "runtime_proxy_continues_anthropic_web_search_server_tool_responses",
    weightSeconds: 8,
  },
  {
    name: "previous_response_negative_cache_boundary_matrix_respects_threshold_and_expiry",
    weightSeconds: 5,
  },
  {
    name: "runtime_state_save_accepts_legacy_backoffs_without_last_good_backup",
    weightSeconds: 6,
  },
]);

export const RUNTIME_CI_TEST_CASES = [
  {
    name: "weekly_exhausted_profile_is_not_a_ready_quota_fallback",
    tags: [TAGS.parallelSafe],
  },
  {
    id: "anthropic-request-translation",
    filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::request_translation::",
    label: "parallel-safe-anthropic-request-translation",
    tags: [TAGS.parallelSafe],
  },
  {
    id: "anthropic-response-translation",
    filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::response_translation::",
    label: "parallel-safe-anthropic-response-translation",
    tags: [TAGS.parallelSafe],
  },
  {
    id: "claude-env-filter",
    filter: "main_internal_tests::runtime_proxy_claude_",
    label: "env-sensitive-claude",
    tags: ENV_PARALLEL_TAGS,
  },
  {
    name: "runtime_proxy_claude_launch_env_uses_foundry_compat_with_profile_config_dir",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_claude_launch_env_honors_model_override",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_claude_launch_env_keeps_custom_picker_entry_for_unknown_override",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_claude_launch_env_uses_codex_config_model_by_default",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_claude_launch_env_maps_alias_backed_override_to_builtin_picker_value",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_claude_target_model_maps_builtin_aliases_to_pinned_gpt_models",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_claude_reasoning_effort_override_normalizes_env",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_claude_reasoning_effort_override_ignores_invalid_env",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_broker_health_endpoint_reports_registered_metadata",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_waits_for_anthropic_inflight_relief_then_succeeds",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_waits_for_one_responses_slot_then_succeeds_past_soft_limit",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "responses_wait_past_old_admission_window_for_healthy_saturated_profile",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "responses_wait_for_any_saturated_profile_and_reselect_after_release",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_responses_inflight_relief_times_out_without_relief",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_wait_scopes_to_session_owner_relief",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_returns_anthropic_overloaded_error_when_interactive_capacity_is_full",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_pressure_mode_sheds_fresh_compact_requests_before_upstream",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "compact_final_failure_logs_inflight_saturation_terminal_reason",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "compact_final_failure_logs_local_selection_terminal_reason",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "compact_final_failure_logs_overload_terminal_reason",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "compact_final_failure_logs_quota_terminal_reason",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "cold_start_candidate_probe_is_queued_without_blocking_selection",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "sync_probe_pressure_mode_is_route_aware_for_background_queue_pressure",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_http_message_followup_with_session_quota_does_not_rotate_or_fresh_fallback",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_http_quota_does_not_fresh_fallback_tool_output_only_requests",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "websocket_previous_response_not_found_requires_stale_continuation_without_turn_state",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "websocket_reuse_watchdog_fresh_fallback_stays_blocked_for_locked_affinity",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_streams_anthropic_mcp_messages_without_buffering",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_translates_anthropic_messages_to_responses_and_back",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_streams_anthropic_messages_from_buffered_responses",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "runtime_proxy_websocket_previous_response_not_found_after_commit_passes_through",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_websocket_previous_response_not_found_after_prelude_surfaces_stale_continuation",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_http_invalid_previous_response_id_recovers_on_same_profile_once",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_http_invalid_previous_response_id_stops_after_one_recovery",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_http_stale_overlay_resume_after_restart_recovers_chain_once",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_http_invalid_previous_response_id_requires_owned_binding",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_http_sse_invalid_previous_response_id_recovers_once_without_rotation",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_websocket_invalid_previous_response_triggers_codex_full_context_replay",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_websocket_0_147_reconnect_invalid_previous_response_replays_full_context",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_websocket_0_149_reconnect_invalid_previous_response_replays_full_context",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_websocket_future_reconnect_invalid_previous_response_replays_full_context",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_websocket_owned_quota_replays_on_ready_profile",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_http_compact_previous_response_not_found_surfaces_stale_continuation",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_http_tool_output_with_session_surfaces_stale_continuation_without_fresh_retry",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_state_snapshot_save_retries_stale_continuation_generation",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_continuation_journal_save_retries_stale_generation",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_state_snapshot_retry_does_not_resurrect_released_response_binding",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_continuation_journal_retry_does_not_resurrect_released_response_binding",
    tags: CONTINUATION_TAGS,
  },
];

function casesWithTag(tag) {
  return RUNTIME_CI_TEST_CASES.filter((testCase) => testCase.tags.includes(tag));
}

function testNamesWithTag(tag) {
  return casesWithTag(tag).map((testCase) => {
    if (!testCase.name) {
      throw new Error(`runtime CI manifest case ${testCase.id ?? testCase.label ?? testCase.filter} needs a test name for ${tag}`);
    }
    return testCase.name;
  });
}

function cargoFilterFor(testCase) {
  if (testCase.filter) {
    return testCase.filter;
  }
  return testCase.name;
}

export const RUNTIME_STRESS_SKIP_TESTS = testNamesWithTag(TAGS.stressSkip);
export const RUNTIME_STRESS_SERIALIZED_TESTS = testNamesWithTag(TAGS.stressSerialized);
export const RUNTIME_STRESS_CONTINUATION_TESTS = testNamesWithTag(TAGS.stressContinuation);

export const RUNTIME_ENV_PARALLEL_CASES = casesWithTag(TAGS.envParallel).map((testCase) => ({
  label: testCase.label ?? testCase.name,
  filter: cargoFilterFor(testCase),
}));
