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
const SERIALIZED_ENV_PARALLEL_TAGS = Object.freeze([...SERIALIZED_TAGS, TAGS.env, TAGS.envParallel]);
const CONTINUATION_TAGS = Object.freeze([
  TAGS.stress,
  TAGS.stressSkip,
  TAGS.stressContinuation,
  TAGS.serial,
  TAGS.quarantine,
]);

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
        id: "root-worker-count",
        filter: "main_internal_tests::runtime_broker_tuning::runtime_proxy_worker_count_",
        label: "worker-count",
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
        id: "root-claude-caveman",
        filter: "main_internal_tests::claude_launch::prepare_runtime_proxy_claude_",
        label: "claude-caveman-root",
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
    suite: "admission",
    label: "admission and request paths",
    filters: [
      {
        id: "admission",
        filter: "main_internal_tests::runtime_proxy_selection_and_pressure::admission::",
        label: "admission",
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
        id: "continuation-http-session-quota",
        filter:
          "main_internal_tests::runtime_proxy_continuations::http_followups::runtime_proxy_http_message_followup_with_session_quota_does_not_rotate_or_fresh_fallback",
        label: "session-quota",
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
    weightSeconds: 4,
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
    weightSeconds: 7,
  },
  {
    name: "runtime_doctor_json_value_includes_selection_markers",
    weightSeconds: 7,
  },
  {
    name: "runtime_doctor_summary_counts_recent_runtime_markers",
    weightSeconds: 7,
  },
  {
    name: "runtime_doctor_state_collects_persisted_degradation_and_orphans",
    weightSeconds: 6,
  },
  {
    name: "remove_all_profiles_clears_state_and_continuation_sidecars",
    weightSeconds: 6,
  },
  {
    name: "previous_response_release_preserves_session_and_compact_session_lineage_for_compact_followups",
    weightSeconds: 6,
  },
  {
    name: "runtime_state_save_scheduler_persists_latest_snapshot",
    weightSeconds: 5,
  },
  {
    name: "translate_runtime_anthropic_messages_request_maps_tools_and_tool_results",
    weightSeconds: 5,
  },
  {
    name: "translate_runtime_anthropic_messages_request_keeps_versioned_builtin_client_tools",
    weightSeconds: 5,
  },
  {
    name: "perform_prodex_cleanup_removes_safe_local_artifacts",
    weightSeconds: 5,
  },
  {
    name: "runtime_affinity_touch_lookups_do_not_requeue_persistence_before_interval",
    weightSeconds: 5,
  },
  {
    name: "perform_prodex_cleanup_deduplicates_profiles_by_email",
    weightSeconds: 5,
  },
  {
    name: "runtime_previous_response_not_found_decision_matrix_stays_consistent",
    weightSeconds: 5,
  },
  {
    name: "auto_runtime_housekeeping_removes_runtime_garbage_without_touching_user_state",
    weightSeconds: 5,
  },
  {
    name: "runtime_proxy_anthropic_messages_retries_tool_result_transcript_on_another_profile",
    weightSeconds: 5,
  },
  {
    name: "runtime_proxy_continues_anthropic_web_search_server_tool_responses",
    weightSeconds: 5,
  },
  {
    name: "next_runtime_response_candidate_sync_probes_cold_start_when_existing_candidate_is_auth_failed",
    weightSeconds: 5,
  },
  {
    name: "previous_response_negative_cache_boundary_matrix_respects_threshold_and_expiry",
    weightSeconds: 5,
  },
  {
    name: "runtime_state_save_accepts_legacy_backoffs_without_last_good_backup",
    weightSeconds: 5,
  },
]);

export const RUNTIME_CI_TEST_CASES = [
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
    name: "runtime_proxy_waits_for_responses_inflight_relief_then_succeeds",
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
    name: "responses_and_websocket_sync_probe_cold_start_under_background_probe_queue_pressure",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "compact_and_standard_defer_sync_probe_cold_start_under_background_probe_queue_pressure",
    tags: SERIALIZED_TAGS,
  },
  {
    name: "next_runtime_response_candidate_skips_sync_cold_start_probe_during_pressure_mode",
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
    name: "runtime_proxy_worker_count_env_override_beats_policy_file",
    label: "env-sensitive-worker-count",
    tags: SERIALIZED_ENV_PARALLEL_TAGS,
  },
  {
    name: "runtime_proxy_websocket_previous_response_not_found_after_commit_surfaces_stale_continuation",
    tags: CONTINUATION_TAGS,
  },
  {
    name: "runtime_proxy_websocket_previous_response_not_found_after_prelude_surfaces_stale_continuation",
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

function runnableCaseFor(testCase) {
  return {
    label: testCase.label ?? testCase.name ?? testCase.id,
    filter: cargoFilterFor(testCase),
  };
}

function runnableCasesWithTag(tag) {
  return casesWithTag(tag).map(runnableCaseFor);
}

export const RUNTIME_STRESS_SKIP_TESTS = testNamesWithTag(TAGS.stressSkip);
export const RUNTIME_STRESS_SERIALIZED_TESTS = testNamesWithTag(TAGS.stressSerialized);
export const RUNTIME_STRESS_CONTINUATION_TESTS = testNamesWithTag(TAGS.stressContinuation);

export const RUNTIME_PARALLEL_SAFE_CASES = runnableCasesWithTag(TAGS.parallelSafe);
export const RUNTIME_SERIAL_CASES = runnableCasesWithTag(TAGS.serial);
export const RUNTIME_STRESS_CASES = runnableCasesWithTag(TAGS.stress);
export const RUNTIME_ENV_CASES = runnableCasesWithTag(TAGS.env);
export const RUNTIME_QUARANTINE_CASES = runnableCasesWithTag(TAGS.quarantine);

export const RUNTIME_ENV_PARALLEL_CASES = casesWithTag(TAGS.envParallel).map((testCase) => ({
  label: testCase.label ?? testCase.name,
  filter: cargoFilterFor(testCase),
}));
