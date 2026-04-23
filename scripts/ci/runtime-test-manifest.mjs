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
    name: "runtime_proxy_active_request_wait_recovers_after_short_burst",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_anthropic_admission_wait_recovers_after_short_burst",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_sheds_long_lived_queue_overload_fast",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_retries_after_websocket_reuse_precommit_hold_timeout",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_absorbs_brief_anthropic_queue_burst",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_absorbs_brief_long_lived_queue_burst",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_does_not_rotate_after_first_sse_chunk_reset",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_retries_same_compact_owner_after_websocket_reuse_watchdog",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_bound_previous_response_without_turn_state_replays_after_websocket_reuse_watchdog",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_stale_websocket_previous_response_reuse_replays_with_stored_turn_state",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_logs_local_writer_disconnect_after_first_chunk",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_surfaces_mid_turn_close_without_post_commit_rotate",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_passes_through_unauthorized_response_and_quarantines_profile_for_next_fresh_request",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_broker_health_endpoint_reports_registered_metadata",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_uses_current_profile_without_extra_runtime_quota_probe",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_reuses_rotated_profile_without_reprobing_quota",
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
    name: "runtime_proxy_keeps_previous_response_affinity_for_http_requests",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_keeps_previous_response_affinity_for_websocket_requests",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_restores_compact_followup_owner_across_restart",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_retries_overloaded_compact_on_another_profile",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_retries_quota_blocked_response_on_another_profile",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_reuses_compact_owner_for_followup_until_response_commits",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_retries_usage_limited_response_on_another_profile",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_releases_quota_blocked_previous_response_affinity_before_fresh_fallback",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_reuse_rotates_on_delayed_quota_before_commit",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_rotates_on_upstream_websocket_quota_error",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_standard_request_preserves_plain_429_when_not_explicit_quota",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_standard_request_retries_usage_limited_response_on_another_profile",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_masks_soft_quota_failure_when_no_ready_http_fallback_remains",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_masks_soft_quota_failure_when_no_ready_websocket_fallback_remains",
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
    name: "runtime_proxy_surfaces_service_unavailable_for_stale_websocket_previous_response_when_owner_snapshot_is_exhausted",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_preserves_function_call_output_affinity_when_previous_response_missing",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_preserves_quota_blocked_function_call_output_previous_response_affinity",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_stale_critical_floor_snapshot_skips_current_profile_on_fresh_websocket_requests",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_stale_critical_floor_snapshot_skips_current_profile_on_fresh_http_requests",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_preserves_quota_blocked_previous_response_affinity_without_turn_state",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_reuse_rotates_on_delayed_overload_before_commit",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_rotates_on_upstream_websocket_overload_error",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_session_affinity_rotates_on_delayed_overload_before_commit",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_websocket_session_id_without_owner_promotes_rotated_profile",
    tags: [TAGS.stressSkip, TAGS.stressSerialized],
  },
  {
    name: "runtime_proxy_worker_count_env_override_beats_policy_file",
    label: "env-sensitive-worker-count",
    tags: [TAGS.stressSkip, TAGS.stressSerialized, TAGS.envParallel],
  },
  {
    name: "runtime_proxy_persists_previous_response_affinity_across_restart",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
  },
  {
    name: "runtime_proxy_persists_session_affinity_across_restart_for_compact",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
  },
  {
    name: "runtime_proxy_retries_after_websocket_reuse_silent_hang",
    tags: [TAGS.stressSkip, TAGS.stressContinuation],
  },
  {
    name: "runtime_proxy_does_not_rotate_after_multi_chunk_sse_stall",
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
  return `main_internal_tests::${testCase.name}`;
}

export const RUNTIME_STRESS_SKIP_TESTS = testNamesWithTag(TAGS.stressSkip);
export const RUNTIME_STRESS_SERIALIZED_TESTS = testNamesWithTag(TAGS.stressSerialized);
export const RUNTIME_STRESS_CONTINUATION_TESTS = testNamesWithTag(TAGS.stressContinuation);

export const RUNTIME_ENV_PARALLEL_CASES = casesWithTag(TAGS.envParallel).map((testCase) => ({
  label: testCase.label ?? testCase.name,
  filter: cargoFilterFor(testCase),
}));
