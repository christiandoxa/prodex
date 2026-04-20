#!/usr/bin/env node
import { spawn } from "node:child_process";

const stressSkips = [
  "runtime_proxy_persists_previous_response_affinity_across_restart",
  "runtime_proxy_persists_session_affinity_across_restart_for_compact",
  "runtime_proxy_claude_launch_env_uses_foundry_compat_with_profile_config_dir",
  "runtime_proxy_claude_launch_env_honors_model_override",
  "runtime_proxy_claude_launch_env_keeps_custom_picker_entry_for_unknown_override",
  "runtime_proxy_claude_launch_env_uses_codex_config_model_by_default",
  "runtime_proxy_claude_launch_env_maps_alias_backed_override_to_builtin_picker_value",
  "runtime_proxy_claude_target_model_maps_builtin_aliases_to_pinned_gpt_models",
  "runtime_proxy_claude_reasoning_effort_override_normalizes_env",
  "runtime_proxy_claude_reasoning_effort_override_ignores_invalid_env",
  "runtime_proxy_active_request_wait_recovers_after_short_burst",
  "runtime_proxy_anthropic_admission_wait_recovers_after_short_burst",
  "runtime_proxy_retries_after_websocket_reuse_silent_hang",
  "runtime_proxy_retries_after_websocket_reuse_precommit_hold_timeout",
  "runtime_proxy_does_not_rotate_after_multi_chunk_sse_stall",
  "runtime_proxy_sheds_long_lived_queue_overload_fast",
  "runtime_proxy_absorbs_brief_anthropic_queue_burst",
  "runtime_proxy_absorbs_brief_long_lived_queue_burst",
  "runtime_proxy_does_not_rotate_after_first_sse_chunk_reset",
  "runtime_proxy_retries_same_compact_owner_after_websocket_reuse_watchdog",
  "runtime_proxy_bound_previous_response_without_turn_state_replays_after_websocket_reuse_watchdog",
  "runtime_proxy_stale_websocket_previous_response_reuse_replays_with_stored_turn_state",
  "runtime_proxy_logs_local_writer_disconnect_after_first_chunk",
  "runtime_proxy_websocket_surfaces_mid_turn_close_without_post_commit_rotate",
  "runtime_proxy_passes_through_unauthorized_response_and_quarantines_profile_for_next_fresh_request",
  "runtime_proxy_broker_health_endpoint_reports_registered_metadata",
  "runtime_proxy_uses_current_profile_without_extra_runtime_quota_probe",
  "runtime_proxy_reuses_rotated_profile_without_reprobing_quota",
  "runtime_proxy_waits_for_anthropic_inflight_relief_then_succeeds",
  "runtime_proxy_waits_for_responses_inflight_relief_then_succeeds",
  "runtime_proxy_responses_inflight_relief_times_out_without_relief",
  "runtime_proxy_wait_scopes_to_session_owner_relief",
  "runtime_proxy_keeps_previous_response_affinity_for_http_requests",
  "runtime_proxy_keeps_previous_response_affinity_for_websocket_requests",
  "runtime_proxy_restores_compact_followup_owner_across_restart",
  "runtime_proxy_retries_overloaded_compact_on_another_profile",
  "runtime_proxy_retries_quota_blocked_response_on_another_profile",
  "runtime_proxy_reuses_compact_owner_for_followup_until_response_commits",
  "runtime_proxy_retries_usage_limited_response_on_another_profile",
  "runtime_proxy_websocket_releases_quota_blocked_previous_response_affinity_before_fresh_fallback",
  "runtime_proxy_websocket_reuse_rotates_on_delayed_quota_before_commit",
  "runtime_proxy_websocket_rotates_on_upstream_websocket_quota_error",
  "runtime_proxy_standard_request_preserves_plain_429_when_not_explicit_quota",
  "runtime_proxy_standard_request_retries_usage_limited_response_on_another_profile",
  "runtime_proxy_masks_soft_quota_failure_when_no_ready_http_fallback_remains",
  "runtime_proxy_masks_soft_quota_failure_when_no_ready_websocket_fallback_remains",
  "runtime_proxy_streams_anthropic_mcp_messages_without_buffering",
  "runtime_proxy_translates_anthropic_messages_to_responses_and_back",
  "runtime_proxy_streams_anthropic_messages_from_buffered_responses",
  "runtime_proxy_surfaces_service_unavailable_for_stale_websocket_previous_response_when_owner_snapshot_is_exhausted",
  "runtime_proxy_websocket_preserves_function_call_output_affinity_when_previous_response_missing",
  "runtime_proxy_websocket_preserves_quota_blocked_function_call_output_previous_response_affinity",
  "runtime_proxy_stale_critical_floor_snapshot_skips_current_profile_on_fresh_websocket_requests",
  "runtime_proxy_stale_critical_floor_snapshot_skips_current_profile_on_fresh_http_requests",
  "runtime_proxy_websocket_preserves_quota_blocked_previous_response_affinity_without_turn_state",
  "runtime_proxy_websocket_reuse_rotates_on_delayed_overload_before_commit",
  "runtime_proxy_websocket_rotates_on_upstream_websocket_overload_error",
  "runtime_proxy_websocket_session_affinity_rotates_on_delayed_overload_before_commit",
  "runtime_proxy_websocket_session_id_without_owner_promotes_rotated_profile",
  "runtime_proxy_worker_count_env_override_beats_policy_file",
];

const serializedTests = [
  "runtime_proxy_claude_launch_env_uses_foundry_compat_with_profile_config_dir",
  "runtime_proxy_claude_launch_env_honors_model_override",
  "runtime_proxy_claude_launch_env_keeps_custom_picker_entry_for_unknown_override",
  "runtime_proxy_claude_launch_env_uses_codex_config_model_by_default",
  "runtime_proxy_claude_launch_env_maps_alias_backed_override_to_builtin_picker_value",
  "runtime_proxy_claude_target_model_maps_builtin_aliases_to_pinned_gpt_models",
  "runtime_proxy_claude_reasoning_effort_override_normalizes_env",
  "runtime_proxy_claude_reasoning_effort_override_ignores_invalid_env",
  "runtime_proxy_active_request_wait_recovers_after_short_burst",
  "runtime_proxy_anthropic_admission_wait_recovers_after_short_burst",
  "runtime_proxy_sheds_long_lived_queue_overload_fast",
  "runtime_proxy_retries_after_websocket_reuse_precommit_hold_timeout",
  "runtime_proxy_absorbs_brief_anthropic_queue_burst",
  "runtime_proxy_absorbs_brief_long_lived_queue_burst",
  "runtime_proxy_does_not_rotate_after_first_sse_chunk_reset",
  "runtime_proxy_retries_same_compact_owner_after_websocket_reuse_watchdog",
  "runtime_proxy_bound_previous_response_without_turn_state_replays_after_websocket_reuse_watchdog",
  "runtime_proxy_stale_websocket_previous_response_reuse_replays_with_stored_turn_state",
  "runtime_proxy_logs_local_writer_disconnect_after_first_chunk",
  "runtime_proxy_websocket_surfaces_mid_turn_close_without_post_commit_rotate",
  "runtime_proxy_passes_through_unauthorized_response_and_quarantines_profile_for_next_fresh_request",
  "runtime_proxy_broker_health_endpoint_reports_registered_metadata",
  "runtime_proxy_uses_current_profile_without_extra_runtime_quota_probe",
  "runtime_proxy_reuses_rotated_profile_without_reprobing_quota",
  "runtime_proxy_waits_for_anthropic_inflight_relief_then_succeeds",
  "runtime_proxy_waits_for_responses_inflight_relief_then_succeeds",
  "runtime_proxy_responses_inflight_relief_times_out_without_relief",
  "runtime_proxy_wait_scopes_to_session_owner_relief",
  "runtime_proxy_keeps_previous_response_affinity_for_http_requests",
  "runtime_proxy_keeps_previous_response_affinity_for_websocket_requests",
  "runtime_proxy_restores_compact_followup_owner_across_restart",
  "runtime_proxy_retries_overloaded_compact_on_another_profile",
  "runtime_proxy_retries_quota_blocked_response_on_another_profile",
  "runtime_proxy_reuses_compact_owner_for_followup_until_response_commits",
  "runtime_proxy_retries_usage_limited_response_on_another_profile",
  "runtime_proxy_websocket_releases_quota_blocked_previous_response_affinity_before_fresh_fallback",
  "runtime_proxy_websocket_reuse_rotates_on_delayed_quota_before_commit",
  "runtime_proxy_websocket_rotates_on_upstream_websocket_quota_error",
  "runtime_proxy_standard_request_preserves_plain_429_when_not_explicit_quota",
  "runtime_proxy_standard_request_retries_usage_limited_response_on_another_profile",
  "runtime_proxy_masks_soft_quota_failure_when_no_ready_http_fallback_remains",
  "runtime_proxy_masks_soft_quota_failure_when_no_ready_websocket_fallback_remains",
  "runtime_proxy_streams_anthropic_mcp_messages_without_buffering",
  "runtime_proxy_translates_anthropic_messages_to_responses_and_back",
  "runtime_proxy_streams_anthropic_messages_from_buffered_responses",
  "runtime_proxy_surfaces_service_unavailable_for_stale_websocket_previous_response_when_owner_snapshot_is_exhausted",
  "runtime_proxy_websocket_preserves_function_call_output_affinity_when_previous_response_missing",
  "runtime_proxy_websocket_preserves_quota_blocked_function_call_output_previous_response_affinity",
  "runtime_proxy_stale_critical_floor_snapshot_skips_current_profile_on_fresh_websocket_requests",
  "runtime_proxy_stale_critical_floor_snapshot_skips_current_profile_on_fresh_http_requests",
  "runtime_proxy_websocket_preserves_quota_blocked_previous_response_affinity_without_turn_state",
  "runtime_proxy_websocket_reuse_rotates_on_delayed_overload_before_commit",
  "runtime_proxy_websocket_rotates_on_upstream_websocket_overload_error",
  "runtime_proxy_websocket_session_affinity_rotates_on_delayed_overload_before_commit",
  "runtime_proxy_websocket_session_id_without_owner_promotes_rotated_profile",
  "runtime_proxy_worker_count_env_override_beats_policy_file",
];

const continuationTests = [
  "runtime_proxy_persists_previous_response_affinity_across_restart",
  "runtime_proxy_persists_session_affinity_across_restart_for_compact",
  "runtime_proxy_retries_after_websocket_reuse_silent_hang",
  "runtime_proxy_does_not_rotate_after_multi_chunk_sse_stall",
];

function parseArgs(argv) {
  const args = { suite: "all" };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--suite") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--suite requires a value");
      }
      args.suite = argv[index];
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function run(command, args, label) {
  return new Promise((resolve, reject) => {
    process.stdout.write(`${label}: ${command} ${args.join(" ")}\n`);
    const child = spawn(command, args, { stdio: "inherit" });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${label} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(new Error(`${label} exited with code ${code}`));
        return;
      }
      resolve();
    });
  });
}

async function retry(label, attemptCount, action) {
  for (let attempt = 1; attempt <= attemptCount; attempt += 1) {
    try {
      await action(attempt);
      return;
    } catch (error) {
      if (attempt === attemptCount) {
        throw error;
      }
      process.stdout.write(`${label}: retrying after transient failure\n`);
      await new Promise((resolve) => setTimeout(resolve, 5000));
    }
  }
}

async function runStressSuite() {
  const args = [
    "test",
    "--lib",
    "main_internal_tests::runtime_proxy_",
    "--",
    "--test-threads=1",
    ...stressSkips.flatMap((testName) => ["--skip", testName]),
  ];
  await run("cargo", args, "runtime-stress");
}

async function runSerializedSuite() {
  await retry("serialized runtime stress", 2, async (attempt) => {
    process.stdout.write(`serialized runtime stress attempt ${attempt}\n`);
    for (const testName of serializedTests) {
      await run("cargo", ["test", "--lib", `main_internal_tests::${testName}`, "--", "--test-threads=1"], testName);
    }
  });
}

async function runContinuationSuite() {
  for (let iteration = 1; iteration <= 2; iteration += 1) {
    process.stdout.write(`continuation-heavy iteration ${iteration}\n`);
    for (const testName of continuationTests) {
      await run("cargo", ["test", "--lib", `main_internal_tests::${testName}`, "--", "--test-threads=1"], testName);
    }
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/ci/runtime-stress.mjs [--suite stress|serialized|continuation|all]",
        "",
        "Runs runtime proxy stress shards with the repository's fixed skip lists.",
      ].join("\n") + "\n",
    );
    return;
  }

  if (args.suite === "stress" || args.suite === "all") {
    await runStressSuite();
  }
  if (args.suite === "serialized" || args.suite === "all") {
    await runSerializedSuite();
  }
  if (args.suite === "continuation" || args.suite === "all") {
    await runContinuationSuite();
  }
}

await main();
