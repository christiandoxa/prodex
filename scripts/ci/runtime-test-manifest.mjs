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

export const RUNTIME_CI_BROAD_SHARD_FILTERS = [
  {
    id: "root-broker",
    filter: "main_internal_tests::runtime_proxy_broker_",
    label: "broker",
  },
  {
    id: "root-log-paths",
    filter: "main_internal_tests::runtime_proxy_log_paths_",
    label: "log-paths",
  },
  {
    id: "root-worker-count",
    filter: "main_internal_tests::runtime_proxy_worker_count_",
    label: "worker-count",
  },
  {
    id: "root-endpoint-child",
    filter: "main_internal_tests::runtime_proxy_endpoint_child_",
    label: "endpoint-child",
  },
  {
    id: "root-claude-launch",
    filter: "main_internal_tests::runtime_proxy_claude_launch_",
    label: "claude-launch-root",
  },
  {
    id: "root-claude-caveman",
    filter: "main_internal_tests::prepare_runtime_proxy_claude_",
    label: "claude-caveman-root",
  },
  {
    id: "selection",
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::selection::",
    label: "selection",
  },
  {
    id: "rotation",
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::rotation::",
    label: "rotation",
  },
  {
    id: "state",
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::state::",
    label: "state",
  },
  {
    id: "admission",
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::admission::",
    label: "admission",
  },
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
  {
    id: "persistence",
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::persistence::",
    label: "persistence",
  },
  {
    id: "doctor",
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::doctor::",
    label: "doctor",
  },
  {
    id: "incidents",
    filter: "main_internal_tests::runtime_proxy_selection_and_pressure::incidents::",
    label: "incidents",
  },
  {
    id: "continuations",
    filter: "main_internal_tests::runtime_proxy_continuations::",
    label: "continuations",
  },
  {
    id: "anthropic-lane-and-launch",
    filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::lane_and_launch::",
    label: "lane-and-launch",
  },
  {
    id: "anthropic-request-translation",
    filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::request_translation::",
    label: "request-translation",
  },
  {
    id: "anthropic-response-translation",
    filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::response_translation::",
    label: "response-translation",
  },
  {
    id: "anthropic-runtime-behavior",
    filter: "main_internal_tests::runtime_proxy_claude_and_anthropic::runtime_proxy_behavior::",
    label: "runtime-behavior",
  },
];

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
