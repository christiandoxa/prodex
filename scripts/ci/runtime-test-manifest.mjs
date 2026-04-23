const TAGS = Object.freeze({
  stressSkip: "runtime-stress:skip",
  stressSerialized: "runtime-stress:serialized",
  stressContinuation: "runtime-stress:continuation",
  envParallel: "runtime-env:parallel",
});

export const RUNTIME_TEST_TAGS = TAGS;

export const RUNTIME_CI_TEST_CASES = [
  {
    id: "claude-env-filter",
    filter: "main_internal_tests::runtime_proxy_claude_",
    label: "env-sensitive-claude",
    tags: [TAGS.envParallel],
  },
  {
    name: "runtime_proxy_claude_launch_env_uses_foundry_compat_with_profile_config_dir",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_claude_launch_env_honors_model_override",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_claude_launch_env_keeps_custom_picker_entry_for_unknown_override",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_claude_launch_env_uses_codex_config_model_by_default",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_claude_launch_env_maps_alias_backed_override_to_builtin_picker_value",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_claude_target_model_maps_builtin_aliases_to_pinned_gpt_models",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_claude_reasoning_effort_override_normalizes_env",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_claude_reasoning_effort_override_ignores_invalid_env",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_broker_health_endpoint_reports_registered_metadata",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_waits_for_anthropic_inflight_relief_then_succeeds",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_waits_for_responses_inflight_relief_then_succeeds",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_responses_inflight_relief_times_out_without_relief",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_wait_scopes_to_session_owner_relief",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_returns_anthropic_overloaded_error_when_interactive_capacity_is_full",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_pressure_mode_sheds_fresh_compact_requests_before_upstream",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "compact_final_failure_logs_inflight_saturation_terminal_reason",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "compact_final_failure_logs_local_selection_terminal_reason",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "compact_final_failure_logs_overload_terminal_reason",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "compact_final_failure_logs_quota_terminal_reason",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "responses_and_websocket_sync_probe_cold_start_under_background_probe_queue_pressure",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "compact_and_standard_defer_sync_probe_cold_start_under_background_probe_queue_pressure",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "next_runtime_response_candidate_skips_sync_cold_start_probe_during_pressure_mode",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_http_message_followup_with_session_quota_does_not_rotate_or_fresh_fallback",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_http_quota_does_not_fresh_fallback_tool_output_only_requests",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "websocket_previous_response_not_found_requires_stale_continuation_without_turn_state",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "websocket_reuse_watchdog_fresh_fallback_stays_blocked_for_locked_affinity",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_streams_anthropic_mcp_messages_without_buffering",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_translates_anthropic_messages_to_responses_and_back",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_streams_anthropic_messages_from_buffered_responses",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_worker_count_env_override_beats_policy_file",
    label: "env-sensitive-worker-count",
    tags: [TAGS.stressSkip, TAGS.stressSerialized, TAGS.envParallel],
  },
  {
    name: "runtime_proxy_websocket_previous_response_not_found_after_commit_surfaces_stale_continuation",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
  },
  {
    name: "runtime_proxy_websocket_previous_response_not_found_after_prelude_surfaces_stale_continuation",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
  },
  {
    name: "runtime_proxy_http_compact_previous_response_not_found_surfaces_stale_continuation",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
  },
  {
    name: "runtime_proxy_http_tool_output_with_session_surfaces_stale_continuation_without_fresh_retry",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
  },
  {
    name: "runtime_state_snapshot_save_retries_stale_continuation_generation",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
  },
  {
    name: "runtime_continuation_journal_save_retries_stale_generation",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
  },
  {
    name: "runtime_state_snapshot_retry_does_not_resurrect_released_response_binding",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
  },
  {
    name: "runtime_continuation_journal_retry_does_not_resurrect_released_response_binding",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
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
