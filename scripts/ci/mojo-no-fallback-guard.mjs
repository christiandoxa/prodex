#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const PRECOMMIT_BUDGET_FILE = "crates/prodex-runtime-proxy/src/failure_response.rs";
const PRECOMMIT_BUDGET_TEST_FILE = "crates/prodex-runtime-proxy/tests/src/failure_response.rs";
const RESPONSE_METADATA_FILE = "crates/prodex-runtime-proxy/src/payload_detection/response_metadata.rs";
const RESPONSE_METADATA_ADAPTER_FILE = "crates/prodex-mojo-core/src/json.rs";
const RESPONSE_METADATA_MOJO_FILE = "mojo/prodex_core/runtime_response_metadata.mojo";
const DOCTOR_MARKER_ABI_ADAPTER_FILE = "crates/prodex-mojo-core/src/rich/runtime_doctor_marker.rs";
const DOCTOR_MARKER_ABI_TEST_FILE = "crates/prodex-mojo-core/tests/runtime_doctor_markers.rs";
const DOCTOR_MARKER_ABI_MOJO_FILE = "mojo/prodex_core/runtime_doctor_marker.mojo";
const DOCTOR_MARKER_SUMMARY_COUNTS_MOJO_FILE = "mojo/prodex_core/runtime_doctor_marker_summary.mojo";
const DOCTOR_MARKER_SUMMARY_COUNTS_CONSUMER_FILE = "crates/prodex-runtime-doctor/src/diagnosis/final_summary/log_summary.rs";
const DOCTOR_MARKER_SUMMARY_COUNTS_SELECTION_FILE = "crates/prodex-runtime-doctor/src/parsing/selection.rs";
const DOCTOR_MARKER_SUMMARY_COUNTS_CALLER_TEST_FILE = "crates/prodex-runtime-doctor/tests/src/parsing.rs";
const DOCTOR_COMPACT_EXIT_COUNTS_CONSUMER_FILE = "crates/prodex-runtime-doctor/src/diagnosis/final_summary/compact.rs";
const DOCTOR_COMPACT_EXIT_COUNTS_CALLER_TEST_FILE = "crates/prodex-runtime-doctor/tests/src/diagnosis.rs";
const DOCTOR_TIMELINE_DETAIL_CONSUMER_FILE = "crates/prodex-runtime-doctor/src/parsing/request_timeline.rs";
const DOCTOR_LAST_MARKER_LINE_CONSUMER_FILE = "crates/prodex-runtime-doctor/src/parsing/log_line.rs";
const DOCTOR_RENDER_ADAPTER_FILE = "crates/prodex-mojo-core/src/rich/runtime_doctor_render.rs";
const DOCTOR_RENDER_MOJO_FILE = "mojo/prodex_core/runtime_doctor_render.mojo";
const RUNTIME_DOCTOR_PLAN_ADAPTER_FILE = "crates/prodex-mojo-core/src/rich/runtime_doctor_plan.rs";
const RUNTIME_DOCTOR_PLAN_MOJO_FILE = "mojo/prodex_core/runtime_doctor_plan.mojo";
const GOVERNANCE_INSPECTION_CONSUMER_FILE = "crates/prodex-domain/src/governance/inspection.rs";
const GOVERNANCE_INSPECTION_ADAPTER_FILE = "crates/prodex-mojo-core/src/policy.rs";
const GOVERNANCE_INSPECTION_MOJO_FILE = "mojo/prodex_core/governance_inspection.mojo";
const GOVERNANCE_INSPECTION_TEST_FILE = "crates/prodex-domain/tests/governance_inspection.rs";
const CLI_DEFAULT_RUN_CONSUMER_FILE = "crates/prodex-cli/src/lib.rs";
const CLI_DEFAULT_RUN_ABI_FILE = "crates/prodex-mojo-core/src/launch.rs";
const CLI_DEFAULT_RUN_MOJO_FILE = "mojo/prodex_core/launch_args.mojo";
const CLI_DEFAULT_RUN_ABI_TEST_FILE = "crates/prodex-mojo-core/tests/launch_args.rs";
const CLI_DEFAULT_RUN_CALLER_TEST_FILE = "crates/prodex-cli/tests/src/shortcuts.rs";
const SMART_CONTEXT_CAPSULE_ORDER_FILE = "crates/prodex-runtime-proxy/src/smart_context/normalization/token_budget.rs";
const SMART_CONTEXT_CAPSULE_ARTIFACTS_FILE = "crates/prodex-runtime-proxy/src/smart_context/normalization/artifacts.rs";
const SMART_CONTEXT_CAPSULE_ORDER_ADAPTER_FILE = "crates/prodex-mojo-core/src/rich/smart_context_capsule_order.rs";
const SMART_CONTEXT_CAPSULE_ORDER_MOJO_FILE = "mojo/prodex_core/smart_context_capsule_order.mojo";
const DEEPSEEK_INPUT_HISTORY_FILE = "crates/prodex-provider-core/src/deepseek_bridge/input_items/history.rs";
const PRESIDIO_LOCAL_REDACTION_FILE = "crates/prodex-app/src/runtime_proxy/presidio/local.rs";
const SMART_CONTEXT_SYMBOLS_CONSUMER_FILE = "crates/prodex-app/src/runtime_state_shared/semantic_index.rs";
const SMART_CONTEXT_SYMBOLS_RUST_FILE = "crates/prodex-app/src/runtime_state_shared/semantic_index/symbols.rs";
const SMART_CONTEXT_SYMBOLS_ADAPTER_FILE = "crates/prodex-mojo-core/src/smart_context_symbols.rs";
const SMART_CONTEXT_SYMBOLS_MOJO_FILE = "mojo/prodex_core/smart_context_symbols.mojo";
const SMART_CONTEXT_SYMBOLS_TEST_FILE = "crates/prodex-mojo-core/tests/smart_context_symbols.rs";
const SMART_CONTEXT_SYMBOLS_CALLER_TEST_FILE = "crates/prodex-app/src/runtime_state_shared/artifact_tests.rs";
const LIVE_LOG_RECORD_FILE = "crates/prodex-runtime-log/src/live.rs";
const LIVE_LOG_RECORD_ADAPTER_FILE = "crates/prodex-mojo-core/src/live_log_record.rs";
const LIVE_LOG_RECORD_MOJO_FILE = "mojo/prodex_core/live_log_record.mojo";
const LIVE_LOG_RECORD_DIRECT_TEST_FILE = "mojo/tests/live_log_record_test.mojo";
const RUNTIME_POLICY_PRESET_CONSUMER_FILE = "crates/prodex-runtime-policy/src/types/runtime_proxy_preset.rs";
const RUNTIME_POLICY_PRESET_CALLER_FILE = "crates/prodex-runtime-policy/src/lib.rs";
const RUNTIME_POLICY_PRESET_ADAPTER_FILE = "crates/prodex-mojo-core/src/runtime_decisions/preset.rs";
const RUNTIME_POLICY_PRESET_TEST_FILE = "crates/prodex-mojo-core/tests/runtime_policy_preset.rs";
const RUNTIME_POLICY_PRESET_MOJO_FILE = "mojo/prodex_core/runtime_tuning.mojo";
const OPERATIONAL_HISTOGRAM_CALLER_FILE = "crates/prodex-app/src/runtime_operational_metrics.rs";
const OPERATIONAL_HISTOGRAM_ADAPTER_FILE = "crates/prodex-mojo-core/src/operational_metrics.rs";
const OPERATIONAL_HISTOGRAM_MOJO_FILE = "mojo/prodex_core/operational_metrics.mojo";
const OPERATIONAL_HISTOGRAM_ABI_TEST_FILE = "crates/prodex-mojo-core/tests/operational_metrics.rs";
const UPDATE_NOTICE_VERSION_CALLER_FILE = "crates/prodex-update-notice/src/lib.rs";
const UPDATE_NOTICE_VERSION_UPDATER_FILE = "crates/prodex-update-notice/src/updater.rs";
const UPDATE_NOTICE_VERSION_ADAPTER_MODULE_FILE = "crates/prodex-update-notice/src/release_version.rs";
const UPDATE_NOTICE_VERSION_ADAPTER_FILE = "crates/prodex-mojo-core/src/update_notice_policy.rs";
const UPDATE_NOTICE_VERSION_MOJO_FILE = "mojo/prodex_core/update_notice_policy.mojo";
const SESSION_USAGE_LIMIT_CONSUMER_FILE = "crates/prodex-app/src/app_commands/runtime_launch/usage_limit_recovery.rs";
const SESSION_USAGE_LIMIT_ADAPTER_FILE = "crates/prodex-mojo-core/src/rich/fallback.rs";
const SESSION_USAGE_LIMIT_FACADE_FILE = "crates/prodex-mojo-core/src/rich.rs";
const SESSION_USAGE_LIMIT_MOJO_FILE = "mojo/prodex_core/rich_fallback.mojo";
const DOCTOR_SMART_CONTEXT_DECISION_CONSUMER_FILE = "crates/prodex-runtime-doctor/src/smart_context.rs";
const DOCTOR_SMART_CONTEXT_DECISION_ADAPTER_FILE = DOCTOR_MARKER_ABI_ADAPTER_FILE;
const DOCTOR_SMART_CONTEXT_DECISION_MOJO_FILE = DOCTOR_MARKER_ABI_MOJO_FILE;
const DOCTOR_LOG_FIELDS_CONSUMER_FILE = "crates/prodex-runtime-doctor/src/log_fields.rs";
const DOCTOR_SMART_CONTEXT_CALLER_TEST_FILE = "crates/prodex-runtime-doctor/tests/src/smart_context_autopilot.rs";
const GEMINI_COMPACT_SNIPPET_CONSUMER_FILE = "crates/prodex-provider-core/src/gemini_bridge/compact/local/snippet.rs";
const GEMINI_COMPACT_TEXT_CONSUMER_FILE = "crates/prodex-provider-core/src/gemini_bridge/compact/local/text.rs";
const GEMINI_COMPACT_SUMMARY_CONSUMER_FILE = "crates/prodex-provider-core/src/gemini_bridge/compact/local.rs";
const GEMINI_COMPACT_SEMANTIC_CONSUMER_FILE = "crates/prodex-provider-core/src/gemini_bridge/compact/local/semantic.rs";
const GEMINI_COMPACT_SNIPPET_REMOVED_RUST_FILE = "crates/prodex-provider-core/src/gemini_bridge/compact/local/snippet/tool.rs";
const GEMINI_COMPACT_SNIPPET_ADAPTER_FILE = "crates/prodex-mojo-core/src/rich/gemini_compact_snippet.rs";
const GEMINI_COMPACT_SNIPPET_MOJO_FILE = "mojo/prodex_core/gemini_compact_snippet.mojo";
const PROMOTED_FILES = [
  GEMINI_COMPACT_SNIPPET_CONSUMER_FILE,
  GEMINI_COMPACT_TEXT_CONSUMER_FILE,
  GEMINI_COMPACT_SUMMARY_CONSUMER_FILE,
  GEMINI_COMPACT_SEMANTIC_CONSUMER_FILE,
  GEMINI_COMPACT_SNIPPET_ADAPTER_FILE,
  GEMINI_COMPACT_SNIPPET_MOJO_FILE,
  OPERATIONAL_HISTOGRAM_CALLER_FILE,
  OPERATIONAL_HISTOGRAM_ADAPTER_FILE,
  OPERATIONAL_HISTOGRAM_MOJO_FILE,
  OPERATIONAL_HISTOGRAM_ABI_TEST_FILE,
  DOCTOR_SMART_CONTEXT_DECISION_CONSUMER_FILE,
  DOCTOR_LOG_FIELDS_CONSUMER_FILE,
  DOCTOR_SMART_CONTEXT_CALLER_TEST_FILE,
  LIVE_LOG_RECORD_FILE,
  LIVE_LOG_RECORD_ADAPTER_FILE,
  LIVE_LOG_RECORD_MOJO_FILE,
  LIVE_LOG_RECORD_DIRECT_TEST_FILE,
  SMART_CONTEXT_CAPSULE_ORDER_ADAPTER_FILE,
  SMART_CONTEXT_CAPSULE_ORDER_MOJO_FILE,
  SMART_CONTEXT_CAPSULE_ARTIFACTS_FILE,
  "crates/prodex-app/src/app_commands/log_throughput_state.rs",
  "crates/prodex-mojo-core/src/log_throughput_policy.rs",
  "crates/prodex-runtime-log/src/retention.rs",
  "crates/prodex-runtime-log/src/retention_selection.rs",
  "crates/prodex-runtime-broker/src/version_guard.rs",
  "crates/prodex-mojo-core/src/super_provider_config.rs",
  "crates/prodex-app/src/runtime_deepseek_config.rs",
  "crates/prodex-cli/src/runtime_args.rs",
  "crates/prodex-app/src/super_expose/protocol/dispatch.rs",
  "crates/prodex-app/src/app_commands/log_transcript.rs",
  "crates/prodex-mojo-core/src/sub_agent_policy.rs",
  "crates/prodex-mojo-core/src/sub_agent_policy/rendering.rs",
  "crates/prodex-cli/src/sub_agent.rs",
  "crates/prodex-cli/src/runtime_args/super_validation.rs",
  "crates/prodex-app/src/runtime_tools/sub_agents.rs",
  "crates/prodex-app/src/runtime_tools/sub_agent_rendering.rs",
  "crates/prodex-app/src/runtime_tools/sub_agent_catalog.rs",
  "crates/prodex-mojo-core/src/runtime_overlay_policy.rs",
  "crates/prodex-app/src/runtime_tools/overlay.rs",
  "crates/prodex-mojo-core/src/rich/super_expose.rs",
  "crates/prodex-app/src/super_expose/protocol/tool_contract.rs",
  "crates/prodex-app/src/super_expose/protocol/validation.rs",
  "crates/prodex-mojo-core/src/provider_usage.rs",
  "crates/prodex-provider-core/src/usage.rs",
  "crates/prodex-audit-log/src/lib.rs",
  "crates/prodex-core/src/lib.rs",
  "crates/prodex-mcp-stdio/src/lib.rs",
  "crates/prodex-shared-codex-fs/src/history.rs",
  "crates/prodex-mojo-core/src/shared_history_policy.rs",
  UPDATE_NOTICE_VERSION_CALLER_FILE,
  UPDATE_NOTICE_VERSION_UPDATER_FILE,
  UPDATE_NOTICE_VERSION_ADAPTER_MODULE_FILE,
  "crates/prodex-shared-codex-fs/src/image_attachments.rs",
  "crates/prodex-runtime-cookies/src/lib.rs",
  "crates/prodex-mojo-core/src/runtime_cookie_policy.rs",
  "crates/prodex-mojo-core/src/shared_attachment_policy.rs",
  UPDATE_NOTICE_VERSION_ADAPTER_FILE,
  UPDATE_NOTICE_VERSION_MOJO_FILE,
  "crates/prodex-mojo-core/src/mcp_stdio_policy.rs",
  "crates/prodex-mcp-stdio/src/lib.rs",
  "crates/prodex-mojo-core/src/mcp_stdio_policy.rs",
  "crates/prodex-mojo-core/src/core_file_policy.rs",
  "crates/prodex-core/src/lib.rs",
  "crates/prodex-mojo-core/src/core_file_policy.rs",
  "crates/prodex-mojo-core/src/audit_log_policy.rs",
  "crates/prodex-runtime-proxy/src/route_decision_trace.rs",
  "crates/prodex-mojo-core/src/runtime_route_reason.rs",
  "crates/prodex-runtime-proxy/src/route_decision_trace/reason.rs",
  "crates/prodex-app/src/runtime_proxy/lineage/remember.rs",
  "crates/prodex-app/src/runtime_proxy/health_circuit.rs",
  "crates/prodex-mojo-core/src/smart_context_markers.rs",
  "crates/prodex-app/src/runtime_state_shared/semantic_index/markers.rs",
  SMART_CONTEXT_SYMBOLS_CONSUMER_FILE,
  SMART_CONTEXT_SYMBOLS_ADAPTER_FILE,
  SMART_CONTEXT_SYMBOLS_MOJO_FILE,
  SMART_CONTEXT_SYMBOLS_TEST_FILE,
  SMART_CONTEXT_SYMBOLS_CALLER_TEST_FILE,
  "crates/prodex-mojo-core/src/lib.rs",
  "crates/prodex-mojo-core/build.rs",
  "crates/prodex-mojo-core/src/smart_context_artifact_ref.rs",
  "crates/prodex-app/src/runtime_proxy/smart_context/artifact_manifest.rs",
  "crates/prodex-app/src/runtime_proxy/smart_context/artifact_refs.rs",
  "crates/prodex-app/src/runtime_proxy/smart_context/rewrite_validation.rs",
  "crates/prodex-mojo-core/src/runtime_decisions/smart_context_policy.rs",
  "mojo/prodex_core/smart_context.mojo",
  "crates/prodex-mojo-core/src/json.rs",
  "mojo/prodex_core/gemini_response.mojo",
  "mojo/prodex_core/rich_abi.mojo",
  RESPONSE_METADATA_FILE,
  RESPONSE_METADATA_MOJO_FILE,
  DOCTOR_MARKER_ABI_ADAPTER_FILE,
  DOCTOR_MARKER_ABI_TEST_FILE,
  DOCTOR_MARKER_ABI_MOJO_FILE,
  DOCTOR_MARKER_SUMMARY_COUNTS_MOJO_FILE,
  DOCTOR_MARKER_SUMMARY_COUNTS_CONSUMER_FILE,
  DOCTOR_MARKER_SUMMARY_COUNTS_SELECTION_FILE,
  DOCTOR_MARKER_SUMMARY_COUNTS_CALLER_TEST_FILE,
  DOCTOR_COMPACT_EXIT_COUNTS_CONSUMER_FILE,
  DOCTOR_COMPACT_EXIT_COUNTS_CALLER_TEST_FILE,
  DOCTOR_TIMELINE_DETAIL_CONSUMER_FILE,
  DOCTOR_RENDER_ADAPTER_FILE,
  DOCTOR_RENDER_MOJO_FILE,
  CLI_DEFAULT_RUN_CONSUMER_FILE,
  CLI_DEFAULT_RUN_ABI_FILE,
  CLI_DEFAULT_RUN_MOJO_FILE,
  CLI_DEFAULT_RUN_ABI_TEST_FILE,
  CLI_DEFAULT_RUN_CALLER_TEST_FILE,
  SESSION_USAGE_LIMIT_CONSUMER_FILE,
  SESSION_USAGE_LIMIT_ADAPTER_FILE,
  SESSION_USAGE_LIMIT_FACADE_FILE,
  SESSION_USAGE_LIMIT_MOJO_FILE,
  "crates/prodex-session-store/src/session_selector.rs",
  "crates/prodex-session-store/src/report.rs",
  "crates/prodex-mojo-core/src/runtime_lineage.rs",
  "crates/prodex-runtime-state/src/lineage.rs",
  "crates/prodex-mojo-core/src/runtime_repo_map.rs",
  "crates/prodex-app/src/runtime_state_shared/line_index.rs",
  "crates/prodex-mojo-core/src/profile_export.rs",
  "crates/prodex-mojo-core/src/profile_export/active_profile.rs",
  "crates/prodex-mojo-core/src/profile_export/copilot.rs",
  "mojo/prodex_core/profile_export_policy.mojo",
  "crates/prodex-profile-export/src/envelope.rs",
  "crates/prodex-profile-export/src/data_model.rs",
  "crates/prodex-profile-export/src/selection.rs",
  "crates/prodex-profile-export/tests/src/lib.rs",
  "crates/prodex-runtime-state/src/quota.rs",
  "crates/prodex-runtime-proxy/src/lib.rs",
  "crates/prodex-mojo-core/src/runtime_broker_continuity.rs",
  "crates/prodex-runtime-broker/src/continuity.rs",
  "crates/prodex-runtime-broker-log/src/lib.rs",
  "crates/prodex-mojo-core/src/state_policy.rs",
  "crates/prodex-state/src/provider_capabilities.rs",
  "crates/prodex-state/src/lib.rs",
  "crates/prodex-mojo-core/src/codex_config.rs",
  "crates/prodex-codex-config/src/lib.rs",
  "crates/prodex-runtime-state/src/background.rs",
  "crates/prodex-mojo-core/src/runtime_state.rs",
  "crates/prodex-redaction/src/lib.rs",
  "crates/prodex-mojo-core/src/redaction.rs",
  PRESIDIO_LOCAL_REDACTION_FILE,
  "crates/prodex-quota/src/models.rs",
  "crates/prodex-quota/src/auth.rs",
  "crates/prodex-quota/src/render/time.rs",
  "crates/prodex-quota/src/render/copilot.rs",
  "crates/prodex-quota/src/render/windows.rs",
  "crates/prodex-quota/tests/src/render/quota_reset_message.rs",
  "crates/prodex-mojo-core/tests/quota_reset_epoch.rs",
  "mojo/prodex_core/quota.mojo",
  "crates/prodex-profile-identity/src/lib.rs",
  "crates/prodex-mojo-core/src/profile_identity.rs",
  "crates/prodex-domain/src/governance/inspection.rs",
  "crates/prodex-mojo-core/build.rs",
  "crates/prodex-mojo-core/src/lib.rs",
  "crates/prodex-mojo-core/src/quota.rs",
  "crates/prodex-mojo-core/src/quota/reset_epoch.rs",
  "crates/prodex-mojo-core/src/routing.rs",
  "crates/prodex-mojo-core/src/runtime.rs",
  "crates/prodex-mojo-core/src/runtime/candidate_plan.rs",
  "crates/prodex-mojo-core/src/runtime/auto_redeem.rs",
  "crates/prodex-mojo-core/src/runtime_decisions.rs",
  "crates/prodex-mojo-core/tests/profile_health.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/final_summary/log_summary.rs",
  "crates/prodex-mojo-core/src/provider_constraints.rs",
  "crates/prodex-mojo-core/src/provider_constraints/gemini_sse_tool_call_index.rs",
  "crates/prodex-provider-core/src/gemini_bridge/hardening/contents/tool_pairs/order.rs",
  "mojo/prodex_core/provider_constraints.mojo",
  "crates/prodex-mojo-core/src/websocket_proxy_policy.rs",
  "crates/prodex-mojo-core/src/transport_failure_policy.rs",
  "crates/prodex-runtime-proxy/src/transport_failure.rs",
  "crates/prodex-runtime-proxy/src/websocket_proxy.rs",
  "crates/prodex-runtime-proxy/src/websocket_tcp_connect_executor/local_pressure.rs",
  "crates/prodex-runtime-proxy/src/websocket_tcp_connect_executor/task_kind.rs",
  "crates/prodex-provider-core/src/surface.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline_dispatch/provider_precommit.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge_routing.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream/error_class.rs",
  "crates/prodex-mojo-core/src/policy.rs",
  "crates/prodex-mojo-core/src/context.rs",
  "crates/prodex-mojo-core/src/rich.rs",
  "crates/prodex-mojo-core/src/rich/catalog.rs",
  "crates/prodex-mojo-core/src/rich/catalog_planner.rs",
  "crates/prodex-mojo-core/src/rich/context_plan.rs",
  "crates/prodex-mojo-core/src/log.rs",
  "crates/prodex-mojo-core/src/log_load.rs",
  "crates/prodex-mojo-core/src/rich/routing.rs",
  "crates/prodex-context/src/critical_signal.rs",
  "crates/prodex-quota/src/render/gemini.rs",
  "crates/prodex-quota/src/render/reports.rs",
  "crates/prodex-quota/src/capacity.rs",
  "crates/prodex-quota/src/render/windows.rs",
  "crates/prodex-quota/src/render/model_capacity.rs",
  "crates/prodex-context/src/critical_signal.rs",
  "crates/prodex-app/src/app_commands/status.rs",
  "crates/prodex-terminal-ui/src/info.rs",
  "crates/prodex-terminal-ui/src/runtime_launch.rs",
  "crates/prodex-mojo-core/src/info_render.rs",
  "crates/prodex-app/src/runtime_external_provider_config.rs",
  "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs",
  "crates/prodex-app/src/super_expose/protocol.rs",
  "crates/prodex-app/src/super_expose/openai_tunnel.rs",
  "crates/prodex-app/src/app_commands/log_event_source.rs",
  "crates/prodex-app/src/app_commands/log_stream.rs",
  "crates/prodex-app/src/app_commands/log_load.rs",
  "crates/prodex-app/src/app_commands/log_command_tui.rs",
  "crates/prodex-mojo-core/src/observability.rs",
  "crates/prodex-app/src/app_commands/ping.rs",
  "crates/prodex-app/src/app_commands/super_main_catalog.rs",
  "crates/prodex-app/src/app_commands/super_main_prompt.rs",
  "crates/prodex-app/src/app_commands/doctor.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_tool_calls.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_deepseek_send.rs",
  "crates/prodex-quota/src/render.rs",
  "crates/prodex-quota/src/render/remaining_percent.rs",
  "crates/prodex-quota/src/render/quota_policy.rs",
  "crates/prodex-runtime-proxy/src/mojo.rs",
  "crates/prodex-runtime-proxy/src/quota.rs",
  "crates/prodex-runtime-proxy/src/previous_response_log.rs",
  "crates/prodex-runtime-proxy/src/route_affinity_log.rs",
  "crates/prodex-runtime-proxy/src/chain_log.rs",
  "crates/prodex-runtime-proxy/src/quota/mojo.rs",
  "crates/prodex-runtime-proxy/tests/src/quota.rs",
  "crates/prodex-runtime-proxy/src/selection_plan.rs",
  "crates/prodex-runtime-proxy/tests/src/selection_plan.rs",
  "crates/prodex-runtime-proxy/tests/src/selection_plan/large_pool.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/static_context.rs",
  "crates/prodex-runtime-proxy/src/smart_context/static_context.rs",
  "crates/prodex-runtime-proxy/src/smart_context/core.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting.rs",
  "crates/prodex-runtime-proxy/src/mojo.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/artifacts.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/rewrite_policy.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/budget.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/telemetry.rs",
  "crates/prodex-runtime-proxy/tests/src/smart_context/static_context.rs",
  PRECOMMIT_BUDGET_FILE,
  PRECOMMIT_BUDGET_TEST_FILE,
  "crates/prodex-runtime-proxy/src/selection_prompt_cache_mojo.rs",
  "crates/prodex-runtime-proxy/src/selection_policy.rs",
  "crates/prodex-runtime-proxy/src/attempt_outcome.rs",
  "crates/prodex-runtime-proxy/src/websocket_message.rs",
  "crates/prodex-runtime-proxy/src/websocket_response_tracking.rs",
  "crates/prodex-runtime-proxy/src/compatibility_surface.rs",
  "crates/prodex-runtime-proxy/src/error_policy.rs",
  "crates/prodex-runtime-proxy/src/error_policy/rate_limit_header.rs",
  "crates/prodex-runtime-proxy/src/error_policy/retry_after.rs",
  "crates/prodex-runtime-proxy/src/error_policy/signal.rs",
  "crates/prodex-runtime-proxy/src/error_policy/stream.rs",
  "crates/prodex-runtime-proxy/src/quota/mojo.rs",
  "crates/prodex-runtime-proxy/src/quota.rs",
  "crates/prodex-runtime-proxy/src/selection_policy/mojo.rs",
  "crates/prodex-runtime-proxy/tests/src/selection_policy.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/token_budget.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/estimation.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/adaptive.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/calibration.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/observed.rs",
  "crates/prodex-runtime-proxy/src/smart_context/safety.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rollout.rs",
  "crates/prodex-runtime-proxy/src/smart_context/regression.rs",
  "crates/prodex-runtime-quota/src/selection/scoring.rs",
  "crates/prodex-runtime-quota/src/selection/scoring/profile_order.rs",
  "crates/prodex-runtime-launch/src/args.rs",
  "crates/prodex-runtime-launch/src/lib.rs",
  RUNTIME_POLICY_PRESET_CONSUMER_FILE,
  RUNTIME_POLICY_PRESET_CALLER_FILE,
  RUNTIME_POLICY_PRESET_ADAPTER_FILE,
  RUNTIME_POLICY_PRESET_TEST_FILE,
  RUNTIME_POLICY_PRESET_MOJO_FILE,
  "crates/prodex-observability/src/lib.rs",
  "crates/prodex-observability/src/metric_label.rs",
  "crates/prodex-observability/src/mojo.rs",
  "crates/prodex-runtime-tuning/src/lib.rs",
  "crates/prodex-runtime-tuning/src/capacity.rs",
  "crates/prodex-runtime-tuning/src/mojo.rs",
  "crates/prodex-runtime-launch/src/args.rs",
  "crates/prodex-runtime-doctor/src/parsing/log_line.rs",
  "crates/prodex-runtime-doctor/src/parsing/request_timeline.rs",
  "crates/prodex-runtime-doctor/src/parsing/route_profile.rs",
  "crates/prodex-runtime-doctor/src/parsing/selection.rs",
  "crates/prodex-runtime-doctor/src/state_summary/profiles.rs",
  "crates/prodex-runtime-doctor/src/state_summary/quota.rs",
  "crates/prodex-runtime-doctor/src/state_summary/routes.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/next_steps.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/next_steps/mojo_render.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/final_summary.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/final_summary/mojo.rs",
  "crates/prodex-runtime-doctor/src/suggestions.rs",
  "crates/prodex-runtime-doctor/src/markers.rs",
  "crates/prodex-provider-core/src/fallback/chains.rs",
  "crates/prodex-provider-core/src/fallback/chains/gemini.rs",
  "crates/prodex-provider-core/src/errors.rs",
  "crates/prodex-provider-core/src/catalog.rs",
  "crates/prodex-provider-core/src/implementation_registry.rs",
  "crates/prodex-provider-core/src/implementation_registry/mojo.rs",
  "crates/prodex-provider-core/src/catalog/reasoning.rs",
  "crates/prodex-provider-core/src/catalog/serialization.rs",
  "crates/prodex-provider-core/src/catalog_tests.rs",
  "crates/prodex-provider-core/src/models.rs",
  "crates/prodex-provider-core/src/surface/models.rs",
  "crates/prodex-provider-core/src/translators/anthropic/messages.rs",
  "crates/prodex-provider-core/src/translators/anthropic/messages/response.rs",
  "crates/prodex-provider-core/src/translators/anthropic/messages/stream.rs",
  "crates/prodex-provider-core/src/translators/anthropic/messages/web_search.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_response.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_response/stream.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_params.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request_mojo.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request_mojo_tests.rs",
  "crates/prodex-provider-core/src/mojo_json.rs",
  "crates/prodex-cli/src/runtime_features.rs",
  "crates/prodex-runtime-quota/src/pressure.rs",
  "crates/prodex-runtime-store/src/continuations/status.rs",
  "crates/prodex-runtime-store/src/continuations/status/mojo.rs",
  "crates/prodex-runtime-store/src/profile_backoff/backoff.rs",
  "crates/prodex-runtime-store/src/profile_backoff/score.rs",
  "crates/prodex-runtime-proxy/src/health/backoff.rs",
  "crates/prodex-runtime-proxy/src/health/score.rs",
  "crates/prodex-runtime-proxy/src/health/latency.rs",
  "crates/prodex-runtime-proxy/src/health/inflight.rs",
  "crates/prodex-runtime-proxy/src/health/health_decisions.rs",
  "crates/prodex-cli/src/runtime_args/super_tail_extract.rs",
  "crates/prodex-cli/src/lib.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/schema.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools/builtin.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request/native_project.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request/simple.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request/tools.rs",
  "crates/prodex-runtime-proxy/src/response_forwarding.rs",
  "crates/prodex-runtime-proxy/src/log_event.rs",
  "crates/prodex-runtime-proxy/src/payload_detection/sse.rs",
  "crates/prodex-runtime-proxy/src/payload_detection/error_messages.rs",
  "crates/prodex-quota/src/render/pool.rs",
  "crates/prodex-quota/src/render/model_capacity.rs",
  "crates/prodex-provider-core/src/translators/gemini/stream.rs",
  "crates/prodex-provider-core/src/translators/gemini/stream/events.rs",
  "crates/prodex-provider-core/src/translators/gemini/stream/shaping.rs",
  "crates/prodex-provider-core/src/translators/gemini/response/status.rs",
  "crates/prodex-provider-core/src/translators/gemini/response/build.rs",
  "crates/prodex-provider-core/src/translators/gemini/response/metadata.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls/chat.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request_contents.rs",
  "crates/prodex-provider-core/src/translators.rs",
  "crates/prodex-provider-core/src/translators/gemini.rs",
  "crates/prodex-provider-core/src/translators/gemini/request.rs",
  "crates/prodex-provider-core/src/gemini_bridge.rs",
  "crates/prodex-provider-core/src/translators/gemini/request_contents.rs",
  "crates/prodex-provider-core/src/translators/gemini/request_transform.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/generation_config.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/generation_config/thinking.rs",
  "crates/prodex-provider-core/src/translators/deepseek/response.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling/response_tool_calls.rs",
  "crates/prodex-provider-core/src/translators/deepseek.rs",
  "crates/prodex-provider-core/src/translators/deepseek/request_transform.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params/reject.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params/reasoning.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params/metadata.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_probe.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo_tests.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items/history.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_choice.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_shape.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/web_search.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/entry.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/mojo.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping.rs",
  "crates/prodex-provider-core/src/translators/deepseek/response/metadata.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/response_values.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping_tests.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/request.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/messages.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/semantics_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/stream.rs",
  "crates/prodex-provider-core/src/translators/kiro/response.rs",
  "crates/prodex-provider-core/src/translators/kiro/acp.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro/stream.rs",
  "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs",
];

const UNCONDITIONAL_MOJO_FILES = new Set([
  "crates/prodex-app/src/app_commands/log_throughput_state.rs",
  "crates/prodex-mojo-core/src/log_throughput_policy.rs",
  "crates/prodex-runtime-broker/src/version_guard.rs",
  "crates/prodex-mojo-core/src/super_provider_config.rs",
  "crates/prodex-cli/src/runtime_args.rs",
  "crates/prodex-app/src/super_expose/protocol/dispatch.rs",
  "crates/prodex-mojo-core/src/log.rs",
  "crates/prodex-app/src/app_commands/log_transcript.rs",
  "crates/prodex-mojo-core/src/sub_agent_policy.rs",
  "crates/prodex-cli/src/sub_agent.rs",
  "crates/prodex-mojo-core/src/rich/super_expose.rs",
  "crates/prodex-app/src/super_expose/protocol/tool_contract.rs",
  "crates/prodex-app/src/super_expose/protocol/validation.rs",
  "crates/prodex-app/src/super_expose/protocol.rs",
  "crates/prodex-mojo-core/src/provider_usage.rs",
  "crates/prodex-provider-core/src/usage.rs",
  "crates/prodex-runtime-proxy/src/route_decision_trace.rs",
  "crates/prodex-runtime-proxy/src/route_decision_trace/reason.rs",
  "crates/prodex-runtime-tuning/src/mojo.rs",
  "crates/prodex-runtime-tuning/src/lib.rs",
  "crates/prodex-app/src/runtime_proxy/lineage/remember.rs",
  "crates/prodex-mojo-core/src/smart_context_markers.rs",
  "crates/prodex-app/src/runtime_state_shared/semantic_index/markers.rs",
  SMART_CONTEXT_SYMBOLS_CONSUMER_FILE,
  SMART_CONTEXT_SYMBOLS_ADAPTER_FILE,
  "crates/prodex-mojo-core/src/smart_context_artifact_ref.rs",
  "crates/prodex-app/src/runtime_proxy/smart_context/artifact_manifest.rs",
  "crates/prodex-app/src/runtime_proxy/smart_context/artifact_refs.rs",
  "crates/prodex-session-store/src/session_selector.rs",
  "crates/prodex-session-store/src/report.rs",
  "crates/prodex-mojo-core/src/runtime_lineage.rs",
  "crates/prodex-runtime-state/src/lineage.rs",
  "crates/prodex-app/src/runtime_state_shared/line_index.rs",
  "crates/prodex-profile-export/src/envelope.rs",
  "crates/prodex-profile-export/src/data_model.rs",
  "crates/prodex-runtime-state/src/quota.rs",
  "crates/prodex-runtime-proxy/src/lib.rs",
  "crates/prodex-runtime-broker/src/continuity.rs",
  "crates/prodex-state/src/provider_capabilities.rs",
  "crates/prodex-state/src/lib.rs",
  "crates/prodex-codex-config/src/lib.rs",
  "crates/prodex-runtime-state/src/background.rs",
  "crates/prodex-redaction/src/lib.rs",
  "crates/prodex-quota/src/models.rs",
  "crates/prodex-profile-identity/src/lib.rs",
  "crates/prodex-domain/src/governance/inspection.rs",
  "crates/prodex-cli/src/runtime_features.rs",
  "crates/prodex-runtime-launch/src/lib.rs",
  "crates/prodex-provider-core/src/translators.rs",
  "crates/prodex-provider-core/src/translators/gemini.rs",
  "crates/prodex-provider-core/src/translators/gemini/request.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/generation_config.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request_contents.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request/tools.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request/simple.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request/native_project.rs",
  "crates/prodex-provider-core/src/translators/gemini/request_contents.rs",
  "crates/prodex-provider-core/src/translators/gemini/request_transform.rs",
  "crates/prodex-provider-core/src/translators/deepseek/response/metadata.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/response_values.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping.rs",
  "crates/prodex-provider-core/src/translators/kiro/acp.rs",
  "crates/prodex-provider-core/src/translators/kiro/response.rs",
  "crates/prodex-quota/src/render/gemini.rs",
  "crates/prodex-quota/src/capacity.rs",
  "crates/prodex-quota/src/render/windows.rs",
  "crates/prodex-app/src/app_commands/status.rs",
  "crates/prodex-observability/src/lib.rs",
  "crates/prodex-observability/src/metric_label.rs",
  "crates/prodex-runtime-store/src/profile_backoff/score.rs",
  "crates/prodex-quota/src/render.rs",
  "crates/prodex-quota/src/render/remaining_percent.rs",
  "crates/prodex-quota/src/render/quota_policy.rs",
  "crates/prodex-runtime-proxy/src/selection_policy.rs",
  "crates/prodex-runtime-proxy/src/selection_plan.rs",
  "crates/prodex-runtime-proxy/tests/src/selection_plan.rs",
  "crates/prodex-runtime-proxy/tests/src/selection_plan/large_pool.rs",
  "crates/prodex-runtime-policy/src/types/runtime_proxy_preset.rs",
  "crates/prodex-runtime-proxy/tests/src/smart_context/static_context.rs",
  "crates/prodex-runtime-proxy/src/smart_context/static_context.rs",
  "crates/prodex-runtime-proxy/src/smart_context/core.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/adaptive.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/artifacts.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/rewrite_policy.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/budget.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/telemetry.rs",
  PRECOMMIT_BUDGET_FILE,
  "crates/prodex-runtime-proxy/src/attempt_outcome.rs",
  "crates/prodex-runtime-proxy/src/websocket_message.rs",
  "crates/prodex-runtime-proxy/src/websocket_response_tracking.rs",
  "crates/prodex-runtime-proxy/src/compatibility_surface.rs",
  "crates/prodex-runtime-proxy/src/error_policy.rs",
  "crates/prodex-runtime-proxy/src/error_policy/rate_limit_header.rs",
  "crates/prodex-runtime-proxy/src/error_policy/retry_after.rs",
  "crates/prodex-runtime-proxy/src/error_policy/signal.rs",
  "crates/prodex-runtime-proxy/src/error_policy/stream.rs",
  "crates/prodex-runtime-proxy/src/quota/mojo.rs",
  "crates/prodex-runtime-proxy/src/selection_policy/mojo.rs",
  "crates/prodex-runtime-proxy/src/selection_prompt_cache_mojo.rs",
  "crates/prodex-runtime-proxy/tests/src/selection_policy.rs",
  "crates/prodex-runtime-quota/src/selection/scoring.rs",
  "crates/prodex-runtime-quota/src/selection/scoring/profile_order.rs",
  "crates/prodex-runtime-doctor/src/parsing/log_line.rs",
  "crates/prodex-runtime-doctor/src/parsing/request_timeline.rs",
  "crates/prodex-runtime-doctor/src/parsing/route_profile.rs",
  "crates/prodex-runtime-doctor/src/parsing/selection.rs",
  "crates/prodex-runtime-doctor/src/state_summary/profiles.rs",
  "crates/prodex-runtime-doctor/src/state_summary/quota.rs",
  "crates/prodex-runtime-doctor/src/state_summary/routes.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/next_steps.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/final_summary.rs",
  "crates/prodex-runtime-doctor/src/suggestions.rs",
  "crates/prodex-cli/src/runtime_args/super_tail_extract.rs",
  "crates/prodex-cli/src/lib.rs",
  "crates/prodex-provider-core/src/translators/anthropic/messages/stream.rs",
  "crates/prodex-provider-core/src/translators/anthropic/messages.rs",
  "crates/prodex-provider-core/src/translators/anthropic/messages/web_search.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_response.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_response/stream.rs",
  "crates/prodex-provider-core/src/translators/gemini/response/grounding.rs",
  "crates/prodex-provider-core/src/gemini_bridge/response_state.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/schema.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools/builtin.rs",
  "crates/prodex-runtime-proxy/src/response_forwarding.rs",
  "crates/prodex-runtime-proxy/src/log_event.rs",
  "crates/prodex-runtime-proxy/src/payload_detection/sse.rs",
  "crates/prodex-runtime-proxy/src/payload_detection/error_messages.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/estimation.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/token_budget.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization.rs",
  "crates/prodex-runtime-proxy/src/smart_context/safety.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rollout.rs",
  "crates/prodex-runtime-proxy/src/smart_context/regression.rs",
  "crates/prodex-runtime-proxy/src/health/score.rs",
  "crates/prodex-runtime-proxy/src/health/latency.rs",
  "crates/prodex-runtime-proxy/src/health/inflight.rs",
  "crates/prodex-runtime-proxy/src/health/health_decisions.rs",
  "crates/prodex-quota/src/render/pool.rs",
  "crates/prodex-provider-core/src/translators/gemini/stream.rs",
  "crates/prodex-provider-core/src/translators/gemini/stream/events.rs",
  "crates/prodex-provider-core/src/translators/gemini/stream/shaping.rs",
  "crates/prodex-provider-core/src/translators/gemini/response/status.rs",
  "crates/prodex-provider-core/src/translators/gemini/response/build.rs",
  "crates/prodex-provider-core/src/translators/gemini/response/metadata.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls/chat.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling/response_tool_calls.rs",
  "crates/prodex-provider-core/src/translators/deepseek.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params/reject.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params/reasoning.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params/metadata.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_probe.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items/history.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_choice.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_shape.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/web_search.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/entry.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/mojo.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/request.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/messages.rs",
  "crates/prodex-provider-core/src/translators/kiro/stream.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_params.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request_mojo.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request_mojo_tests.rs",
  "crates/prodex-provider-core/src/errors.rs",
  "crates/prodex-provider-core/src/catalog.rs",
  "crates/prodex-provider-core/src/implementation_registry.rs",
  "crates/prodex-provider-core/src/catalog/reasoning.rs",
  "crates/prodex-provider-core/src/catalog/serialization.rs",
  "crates/prodex-provider-core/src/models.rs",
]);
const FEATURE_OFF_RUST_PATH = /\bnot\s*\(\s*feature\s*=\s*"(?:mojo|mojo-core|runtime-log-mojo|state-summary-mojo)"\s*\)/u;
const ANTHROPIC_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages/response.rs";
const ANTHROPIC_MESSAGES_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages.rs";
const ANTHROPIC_WEB_SEARCH_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages/web_search.rs";
const ANTHROPIC_SSE_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/anthropic_messages_sse_reader.rs";
const ANTHROPIC_REQUEST_FALLBACK_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages/request_fallback.rs";
const ANTHROPIC_REQUEST_ORACLE_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages/mojo_request_tests.rs";
const REMOVED_ORACLE_FILES = [
  GEMINI_COMPACT_SNIPPET_REMOVED_RUST_FILE,
  SMART_CONTEXT_SYMBOLS_RUST_FILE,
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/strict_schema.rs",
  "crates/prodex-observability/src/rust.rs",
  "crates/prodex-observability/src/metric_label/mojo_parity_tests.rs",
  ANTHROPIC_REQUEST_FALLBACK_FILE,
  ANTHROPIC_REQUEST_ORACLE_FILE,
  "crates/prodex-context/src/critical_signal/rust_oracle.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/oracle.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/pressure.rs",
  "crates/prodex-runtime-proxy/src/quota/rust_oracles.rs",
  "crates/prodex-runtime-launch/src/args_oracle.rs",
  "crates/prodex-runtime-launch/src/args_resume.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/next_steps/compatibility.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/final_summary/default_diagnosis.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/final_summary/pressure.rs",
  "crates/prodex-runtime-doctor/src/suggestions/compatibility.rs",
  "crates/prodex-cli/src/runtime_args/super_tail_extract/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/schema/composition.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/optional_fields.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request/simple/builtin.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/controls.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/validation.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request/validation.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request/validation/input_content.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_util.rs",
  "crates/prodex-provider-core/src/catalog_parity_tests.rs",
  "crates/prodex-provider-core/src/implementation_registry/rust.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/adjacency.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items/push.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items/push/fields.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_probe/input.rs",
  "crates/prodex-runtime-proxy/src/payload_detection/sse/rust_oracle.rs",
  "crates/prodex-runtime-proxy/src/selection_policy/rust_oracles.rs",
  "crates/prodex-runtime-proxy/src/compatibility_surface/rust_oracle.rs",
  "crates/prodex-runtime-proxy/src/error_policy/json.rs",
  "crates/prodex-runtime-proxy/src/selection_prompt_cache_rust.rs",
  "crates/prodex-provider-core/src/translators/deepseek/request.rs",
  "crates/prodex-provider-core/src/translators/deepseek/request/mojo_parity_tests.rs",
  "crates/prodex-provider-core/src/translators/deepseek/request/params.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params_tests.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling/messages.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling/messages/chat_items.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling/messages/input_tool_calls.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling/messages/local_shell.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/shape.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/shape/mcp.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/tool_choice.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/tools.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/tools/custom.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/tools/mcp_toolset.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/tools/namespace.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/tools/tool_search.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/util.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/web_search.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/mojo_tests.rs",
];
const HARD_REPLACED_RUST_FILES = new Set([
  "crates/prodex-quota/src/render/gemini.rs",
  "crates/prodex-quota/src/capacity.rs",
  "crates/prodex-observability/src/lib.rs",
  "crates/prodex-observability/src/metric_label.rs",
  "crates/prodex-runtime-store/src/profile_backoff/score.rs",
  "crates/prodex-context/src/critical_signal.rs",
  "crates/prodex-quota/src/render/windows.rs",
  "crates/prodex-quota/src/render/remaining_percent.rs",
  "crates/prodex-quota/src/render/quota_policy.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/token_budget.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/estimation.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/rewrite_policy.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/budget.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/telemetry.rs",
  "crates/prodex-runtime-proxy/src/health/score.rs",
  "crates/prodex-runtime-proxy/src/health/latency.rs",
  "crates/prodex-runtime-proxy/src/health/inflight.rs",
  "crates/prodex-runtime-proxy/src/health/health_decisions.rs",
  "crates/prodex-runtime-proxy/src/websocket_message.rs",
  "crates/prodex-runtime-proxy/src/websocket_response_tracking.rs",
  "crates/prodex-quota/src/render/model_capacity.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/adaptive.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/calibration.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/observed.rs",
  "crates/prodex-runtime-proxy/src/smart_context/safety.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rollout.rs",
  "crates/prodex-runtime-proxy/src/smart_context/regression.rs",
  "crates/prodex-runtime-proxy/src/selection_policy.rs",
  "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs",
  "crates/prodex-runtime-proxy/src/selection_plan.rs",
  "crates/prodex-runtime-proxy/tests/src/selection_plan.rs",
  "crates/prodex-runtime-proxy/tests/src/selection_plan/large_pool.rs",
  "crates/prodex-runtime-policy/src/types/runtime_proxy_preset.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/static_context.rs",
  "crates/prodex-runtime-proxy/tests/src/smart_context/static_context.rs",
  "crates/prodex-runtime-proxy/src/smart_context/static_context.rs",
  "crates/prodex-runtime-proxy/src/smart_context/core.rs",
  "crates/prodex-runtime-proxy/src/quota.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/artifacts.rs",
  "crates/prodex-mojo-core/src/runtime/candidate_plan.rs",
  PRECOMMIT_BUDGET_FILE,
  PRECOMMIT_BUDGET_TEST_FILE,
  "crates/prodex-runtime-proxy/src/attempt_outcome.rs",
  "crates/prodex-runtime-proxy/src/compatibility_surface.rs",
  "crates/prodex-runtime-proxy/src/error_policy.rs",
  "crates/prodex-runtime-proxy/src/error_policy/rate_limit_header.rs",
  "crates/prodex-runtime-proxy/src/error_policy/retry_after.rs",
  "crates/prodex-runtime-proxy/src/error_policy/signal.rs",
  "crates/prodex-runtime-proxy/src/error_policy/stream.rs",
  "crates/prodex-runtime-proxy/src/selection_policy/mojo.rs",
  "crates/prodex-runtime-proxy/src/selection_prompt_cache_mojo.rs",
  "crates/prodex-runtime-tuning/src/capacity.rs",
  "crates/prodex-provider-core/src/fallback/chains.rs",
  "crates/prodex-provider-core/src/translators/anthropic/messages/stream.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_response.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_response/stream.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_params.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request_mojo.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request_mojo_tests.rs",
  "crates/prodex-runtime-launch/src/args.rs",
  "crates/prodex-runtime-doctor/src/parsing/log_line.rs",
  "crates/prodex-runtime-doctor/src/parsing/request_timeline.rs",
  "crates/prodex-runtime-doctor/src/parsing/route_profile.rs",
  "crates/prodex-runtime-doctor/src/parsing/selection.rs",
  "crates/prodex-runtime-doctor/src/state_summary/profiles.rs",
  "crates/prodex-runtime-doctor/src/state_summary/quota.rs",
  "crates/prodex-runtime-doctor/src/state_summary/routes.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/next_steps.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/final_summary.rs",
  "crates/prodex-runtime-doctor/src/suggestions.rs",
  "crates/prodex-cli/src/runtime_args/super_tail_extract.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/schema.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools/builtin.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request/native_project.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request/simple.rs",
  "crates/prodex-runtime-proxy/src/response_forwarding.rs",
  "crates/prodex-runtime-proxy/src/log_event.rs",
  "crates/prodex-runtime-proxy/src/payload_detection/sse.rs",
  "crates/prodex-runtime-proxy/src/payload_detection/error_messages.rs",
  "crates/prodex-quota/src/render/pool.rs",
  "crates/prodex-provider-core/src/translators/gemini/stream.rs",
  "crates/prodex-provider-core/src/translators/gemini/stream/events.rs",
  "crates/prodex-provider-core/src/translators/gemini/stream/shaping.rs",
  "crates/prodex-provider-core/src/translators/gemini/response/status.rs",
  "crates/prodex-provider-core/src/translators/gemini/response/build.rs",
  "crates/prodex-provider-core/src/translators/gemini/response/metadata.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls/chat.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request_contents.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/generation_config.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/generation_config/thinking.rs",
  "crates/prodex-provider-core/src/translators/deepseek/response.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling/response_tool_calls.rs",
  "crates/prodex-provider-core/src/translators/deepseek.rs",
  "crates/prodex-provider-core/src/translators/deepseek/request_transform.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params/reject.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params/reasoning.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params/metadata.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo_tests.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items/history.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/entry.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/mojo.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping_tests.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/request.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/semantics_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/stream.rs",
  "crates/prodex-provider-core/src/errors.rs",
  "crates/prodex-provider-core/src/catalog.rs",
  "crates/prodex-provider-core/src/implementation_registry.rs",
  "crates/prodex-provider-core/src/catalog/reasoning.rs",
  "crates/prodex-provider-core/src/catalog/serialization.rs",
  "crates/prodex-provider-core/src/models.rs",
]);
const REQUIRED_DEFAULT_FEATURES = new Map([
  ["crates/prodex-app/Cargo.toml", "mojo-core"],
  ["crates/prodex-quota/Cargo.toml", "mojo"],
  ["crates/prodex-runtime-doctor/Cargo.toml", "state-summary-mojo"],
  ["crates/prodex-runtime-launch/Cargo.toml", "mojo"],
]);
const RUNTIME_STATE_BACKGROUND_FILE = "crates/prodex-runtime-state/src/background.rs";
const RUNTIME_LOG_RETENTION_FILE = "crates/prodex-runtime-log/src/retention.rs";
const RUNTIME_LOG_RETENTION_SELECTION_FILE = "crates/prodex-runtime-log/src/retention_selection.rs";
const RUNTIME_STATE_QUOTA_FILE = "crates/prodex-runtime-state/src/quota.rs";
const RUNTIME_PROXY_ROOT_FILE = "crates/prodex-runtime-proxy/src/lib.rs";
const BROKER_CONTINUITY_FILE = "crates/prodex-runtime-broker/src/continuity.rs";
const BROKER_LOG_CACHE_FILE = "crates/prodex-runtime-broker-log/src/lib.rs";
const BROKER_VERSION_GUARD_FILE = "crates/prodex-runtime-broker/src/version_guard.rs";
const CODEX_CONFIG_FILE = "crates/prodex-codex-config/src/lib.rs";
const STATE_FILE = "crates/prodex-state/src/lib.rs";
const STATE_PROVIDER_FILE = "crates/prodex-state/src/provider_capabilities.rs";
const STATE_REMEMBER_FILE = "crates/prodex-app/src/runtime_proxy/lineage/remember.rs";
const REDACTION_FILE = "crates/prodex-redaction/src/lib.rs";
const PROFILE_IDENTITY_FILE = "crates/prodex-profile-identity/src/lib.rs";
const SUPER_PROVIDER_CONFIG_FILE = "crates/prodex-cli/src/runtime_args.rs";
const SUPER_PROVIDER_CONFIG_ADAPTER_FILE = "crates/prodex-mojo-core/src/super_provider_config.rs";
const EXTERNAL_PROVIDER_CONFIG_FILE = "crates/prodex-app/src/runtime_external_provider_config.rs";
const SUB_AGENT_POLICY_FILE = "crates/prodex-cli/src/sub_agent.rs";
const SUB_AGENT_CLI_VALIDATION_FILE = "crates/prodex-cli/src/runtime_args/super_validation.rs";
const SUB_AGENT_POLICY_ADAPTER_FILE = "crates/prodex-mojo-core/src/sub_agent_policy.rs";
const SUB_AGENT_RENDER_ADAPTER_FILE = "crates/prodex-mojo-core/src/sub_agent_policy/rendering.rs";
const SUB_AGENT_CHILD_FILE = "crates/prodex-app/src/runtime_tools/sub_agents.rs";
const SUB_AGENT_RENDERING_FILE = "crates/prodex-app/src/runtime_tools/sub_agent_rendering.rs";
const RUNTIME_OVERLAY_POLICY_FILE = "crates/prodex-app/src/runtime_tools/overlay.rs";
const RUNTIME_OVERLAY_POLICY_ADAPTER_FILE = "crates/prodex-mojo-core/src/runtime_overlay_policy.rs";
const CLI_RUNTIME_FEATURE_FILE = "crates/prodex-cli/src/runtime_features.rs";
const DOCTOR_CARGO_FILE = "crates/prodex-runtime-doctor/Cargo.toml";
const RUNTIME_PROXY_CARGO_FILE = "crates/prodex-runtime-proxy/Cargo.toml";
const RUNTIME_TUNING_CARGO_FILE = "crates/prodex-runtime-tuning/Cargo.toml";
const QUOTA_MODELS_FILE = "crates/prodex-quota/src/models.rs";
const QUOTA_AUTH_FILE = "crates/prodex-quota/src/auth.rs";
const QUOTA_TIME_FILE = "crates/prodex-quota/src/render/time.rs";
const QUOTA_COPILOT_FILE = "crates/prodex-quota/src/render/copilot.rs";
const QUOTA_GEMINI_DISPLAY_FILE = "crates/prodex-quota/src/render/gemini.rs";
const QUOTA_REPORTS_FILE = "crates/prodex-quota/src/render/reports.rs";
const QUOTA_ADAPTER_FILE = "crates/prodex-mojo-core/src/quota.rs";
const QUOTA_WINDOWS_FILE = "crates/prodex-quota/src/render/windows.rs";
const QUOTA_RESET_EPOCH_ADAPTER_FILE = "crates/prodex-mojo-core/src/quota/reset_epoch.rs";
const QUOTA_RESET_EPOCH_MOJO_FILE = "mojo/prodex_core/quota.mojo";
const QUOTA_RESET_EPOCH_TEST_FILE = "crates/prodex-mojo-core/tests/quota_reset_epoch.rs";
const QUOTA_RESET_EPOCH_CALLER_TEST_FILE = "crates/prodex-quota/tests/src/render/quota_reset_message.rs";
const REHYDRATE_FILE = "crates/prodex-runtime-proxy/src/smart_context/token_accounting.rs";
const SUPER_OVERRIDE_FILE = "crates/prodex-cli/src/runtime_args/super_tail_extract.rs";
const SUPER_EXPOSE_FILE = "crates/prodex-cli/src/lib.rs";
const SUPER_EXPOSE_PROTOCOL_FILE = "crates/prodex-app/src/super_expose/protocol.rs";
const SUPER_EXPOSE_DISPATCH_FILE = "crates/prodex-app/src/super_expose/protocol/dispatch.rs";
const SUPER_EXPOSE_VALIDATION_FILE = "crates/prodex-app/src/super_expose/protocol/validation.rs";
const SUPER_EXPOSE_TOOL_CONTRACT_FILE = "crates/prodex-app/src/super_expose/protocol/tool_contract.rs";
const SUPER_EXPOSE_RICH_FILE = "crates/prodex-mojo-core/src/rich/super_expose.rs";
const LOG_TRANSCRIPT_FILE = "crates/prodex-app/src/app_commands/log_transcript.rs";
const LOG_ADAPTER_FILE = "crates/prodex-mojo-core/src/log.rs";
const LOG_LOAD_APP_FILE = "crates/prodex-app/src/app_commands/log_load.rs";
const LOG_LOAD_TUI_FILE = "crates/prodex-app/src/app_commands/log_command_tui.rs";
const LOG_LOAD_ADAPTER_FILE = "crates/prodex-mojo-core/src/log_load.rs";
const LOG_STREAM_FILE = "crates/prodex-app/src/app_commands/log_stream.rs";
const LOG_EVENT_SOURCE_FILE = "crates/prodex-app/src/app_commands/log_event_source.rs";
const OBSERVABILITY_ADAPTER_FILE = "crates/prodex-mojo-core/src/observability.rs";
const GEMINI_SCHEMA_FILE = "crates/prodex-provider-core/src/translators/gemini/request/schema.rs";
const GEMINI_TOOLS_FILE = "crates/prodex-provider-core/src/translators/gemini/request/tools.rs";
const GEMINI_STATUS_FILE = "crates/prodex-provider-core/src/translators/gemini/response/status.rs";
const GEMINI_BUFFERED_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/gemini/response/build.rs";
const GEMINI_TOOL_RESPONSE_ORDER_FILE = "crates/prodex-provider-core/src/gemini_bridge/hardening/contents/tool_pairs/order.rs";
const GEMINI_TOOL_RESPONSE_ORDER_ADAPTER_FILE = "crates/prodex-mojo-core/src/provider_constraints/gemini_sse_tool_call_index.rs";
const GEMINI_TOOL_RESPONSE_ORDER_MOJO_FILE = "mojo/prodex_core/provider_constraints.mojo";
const RESPONSE_FORWARDING_FILE = "crates/prodex-runtime-proxy/src/response_forwarding.rs";
const QUOTA_POOL_FILE = "crates/prodex-quota/src/render/pool.rs";
const STATUS_SUMMARY_FILE = "crates/prodex-app/src/app_commands/status.rs";
const QUOTA_MODEL_CAPACITY_FILE = "crates/prodex-quota/src/render/model_capacity.rs";
const RUNTIME_QUOTA_FILE = "crates/prodex-runtime-proxy/src/quota.rs";
const SELECTION_POLICY_FILE = "crates/prodex-runtime-proxy/src/selection_policy.rs";
const HEALTH_ABI_TEST_FILE = "crates/prodex-mojo-core/tests/profile_health.rs";
const PROFILE_HEALTH_CIRCUIT_FILE = "crates/prodex-app/src/runtime_proxy/health_circuit.rs";
const DEEPSEEK_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/deepseek/response.rs";
const DEEPSEEK_RESPONSE_TOOL_CALLS_FILE = "crates/prodex-provider-core/src/translators/deepseek/tooling/response_tool_calls.rs";
const DEEPSEEK_REQUEST_FILE = "crates/prodex-provider-core/src/translators/deepseek/request_transform.rs";
const DEEPSEEK_REQUEST_REJECT_FILE = "crates/prodex-provider-core/src/deepseek_bridge/request_params/reject.rs";
const DEEPSEEK_REASONING_FILE = "crates/prodex-provider-core/src/deepseek_bridge/request_params/reasoning.rs";
const DEEPSEEK_METADATA_FILE = "crates/prodex-provider-core/src/deepseek_bridge/request_params/metadata.rs";
const DEEPSEEK_SIMPLE_REQUEST_FILE = "crates/prodex-provider-core/src/deepseek_bridge/request_probe.rs";
const KIRO_CHAT_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/kiro/response.rs";
const KIRO_MESSAGES_FILE = "crates/prodex-provider-core/src/translators/kiro/request/messages.rs";
const KIRO_REQUEST_FILE = "crates/prodex-provider-core/src/translators/kiro/request.rs";
const KIRO_LOCAL_REWRITE_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro.rs";
const KIRO_PROMPT_ABI_TEST_FILE = "crates/prodex-mojo-core/tests/kiro_prompt.rs";
const KIRO_STREAM_FILE = "crates/prodex-provider-core/src/translators/kiro/stream.rs";
const KIRO_FINAL_STREAM_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro/stream.rs";
const KIRO_ACP_FILE = "crates/prodex-provider-core/src/translators/kiro/acp.rs";
const KIRO_CATALOG_NORMALIZER_FILE = "crates/prodex-provider-core/src/catalog/kiro.rs";
const KIRO_CATALOG_APP_ADAPTER_FILE = "crates/prodex-app/src/profile_commands/kiro/catalog.rs";
const KIRO_CATALOG_ABI_FILE = "crates/prodex-mojo-core/src/json/kiro_catalog.rs";
const KIRO_CATALOG_MOJO_FILE = "mojo/prodex_core/kiro_model_catalog.mojo";
const KIRO_CATALOG_ABI_TEST_FILE = "crates/prodex-mojo-core/tests/kiro_catalog.rs";
const KIRO_ACP_OPERATIONS = [
  "KiroKernelOperation::AcpInitializeRequest",
  "KiroKernelOperation::AcpSessionNewRequest",
  "KiroKernelOperation::AcpSessionPromptRequest",
  "KiroKernelOperation::AcpModel",
  "KiroKernelOperation::AcpAssistantOutput",
  "KiroKernelOperation::AcpResponse",
  "KiroKernelOperation::AcpChatAssistant",
  "KiroKernelOperation::AcpPlanEntry",
  "KiroKernelOperation::AcpError",
  "KiroKernelOperation::AcpSessionInfo",
  "KiroKernelOperation::AcpMetadata",
  "KiroKernelOperation::AcpIncompleteDetails",
];
const KIRO_RESPONSE_HELPER_OPERATIONS = [
  "KiroKernelOperation::ModelList",
  "KiroKernelOperation::ModelNotFound",
  "KiroKernelOperation::InvalidRequestError",
  "KiroKernelOperation::UnsupportedPathError",
  "KiroKernelOperation::FinishReason",
];
const DEEPSEEK_STRICT_TOOLS_FILE = "crates/prodex-provider-core/src/deepseek_bridge/request_tools.rs";
const DEEPSEEK_STRICT_SCHEMA_FILE = "crates/prodex-provider-core/src/deepseek_bridge/request_tools/strict_schema.rs";
const PROVIDER_ERROR_FILE = "crates/prodex-provider-core/src/errors.rs";
const PROVIDER_SURFACE_FILE = "crates/prodex-provider-core/src/surface.rs";
const PROVIDER_CONSTRAINTS_ADAPTER_FILE = "crates/prodex-mojo-core/src/provider_constraints.rs";
const WEBSOCKET_PROXY_POLICY_ADAPTER_FILE = "crates/prodex-mojo-core/src/websocket_proxy_policy.rs";
const WEBSOCKET_PROXY_POLICY_FILE = "crates/prodex-runtime-proxy/src/websocket_proxy.rs";
const TRANSPORT_FAILURE_POLICY_ADAPTER_FILE = "crates/prodex-mojo-core/src/transport_failure_policy.rs";
const TRANSPORT_FAILURE_POLICY_FILE = "crates/prodex-runtime-proxy/src/transport_failure.rs";
const PROVIDER_PRECOMMIT_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline_dispatch/provider_precommit.rs";
const PROVIDER_BRIDGE_METADATA_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge.rs";
const PROVIDER_BRIDGE_ROUTING_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge_routing.rs";
const LOCAL_REWRITE_UPSTREAM_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream.rs";
const LOCAL_REWRITE_PIPELINE_DISPATCH_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline_dispatch.rs";
const LOCAL_REWRITE_BINDING_CANDIDATE_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream/binding_candidate.rs";
const NATIVE_FIRST_ERROR_CLASS_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream/error_class.rs";
const LINEAGE_BINDING_CANDIDATE_FILE = "crates/prodex-mojo-core/src/runtime_lineage/binding_candidate.rs";
const MODEL_SPEC_FILE = "crates/prodex-provider-core/src/surface/models.rs";
const PROMPT_CACHE_SELECTION_FILE = "crates/prodex-runtime-proxy/src/selection_plan.rs";
const FINGERPRINT_DELTA_FILE = "crates/prodex-runtime-proxy/src/smart_context/static_context.rs";
const FINGERPRINT_ARTIFACTS_FILE = "crates/prodex-runtime-proxy/src/smart_context/normalization/artifacts.rs";
const SMART_CONTEXT_CORE_FILE = "crates/prodex-runtime-proxy/src/smart_context/core.rs";
const SMART_CONTEXT_DUPLICATE_VALIDATION_FILE = "crates/prodex-app/src/runtime_proxy/smart_context/rewrite_validation.rs";
const SMART_CONTEXT_DUPLICATE_ADAPTER_FILE = "crates/prodex-mojo-core/src/runtime_decisions/smart_context_policy.rs";
const SMART_CONTEXT_DUPLICATE_MOJO_FILE = "mojo/prodex_core/smart_context.mojo";
const SMART_CONTEXT_POLICY_MOJO_FILE = "mojo/prodex_core/smart_context_policy.mojo";
const ADAPTIVE_BUDGET_FILE = "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/adaptive.rs";
const DEEPSEEK_SHAPING_FILE = "crates/prodex-provider-core/src/translators/deepseek/stream/shaping.rs";
const DEEPSEEK_STREAM_RESPONSE_VALUES_FILE = "crates/prodex-provider-core/src/translators/deepseek/stream/response_values.rs";
const DEEPSEEK_RESPONSE_METADATA_FILE = "crates/prodex-provider-core/src/translators/deepseek/response/metadata.rs";
const DEEPSEEK_STREAM_PROMOTED_OPERATIONS = [
  "DeepSeekKernelOperation::StreamToolCallDelta",
  "DeepSeekKernelOperation::StreamChunkMetadata",
  "DeepSeekKernelOperation::StreamChoiceMetadata",
  "DeepSeekKernelOperation::StreamChoiceDelta",
  "DeepSeekKernelOperation::StreamResponseMetadata",
];
const RUNTIME_DOCTOR_MARKERS_FILE = "crates/prodex-runtime-doctor/src/markers.rs";
const RUNTIME_DOCTOR_FAILURE_CLASS_FILE = "crates/prodex-runtime-doctor/src/diagnosis/final_summary/log_summary.rs";
const CHAT_TOOLS_BRIDGE_FILE = "crates/prodex-provider-core/src/chat_tools_bridge.rs";
const CHAT_TOOLS_MOJO_FILE = "crates/prodex-provider-core/src/chat_tools_bridge/mojo.rs";
const DEEPSEEK_SHAPING_COMPLETED_FNS = [
  "deepseek_provider_core_response_completed_event",
  "deepseek_provider_core_response_created_event",
  "deepseek_provider_core_stream_output_text_item",
  "deepseek_provider_core_stream_tool_call_added_item",
  "deepseek_provider_core_stream_tool_call_item",
  "deepseek_provider_core_stream_function_call_arguments_delta_source",
  "deepseek_provider_core_stream_text_delta_source",
  "deepseek_provider_core_output_item_added_event",
  "deepseek_provider_core_function_call_arguments_delta_event",
  "deepseek_provider_core_output_text_delta_event",
  "deepseek_provider_core_output_item_done_event",
];
const GEMINI_TOOL_CALLS_FILE = "crates/prodex-provider-core/src/translators/gemini/response_tool_calls.rs";
const GEMINI_CHAT_TOOL_CALLS_FILE = "crates/prodex-provider-core/src/translators/gemini/response_tool_calls/chat.rs";
const GEMINI_BRIDGE_REQUEST_FILE = "crates/prodex-provider-core/src/gemini_bridge/request.rs";
const GEMINI_BRIDGE_ROOT_FILE = "crates/prodex-provider-core/src/gemini_bridge.rs";
const GEMINI_BRIDGE_REQUEST_CONTENTS_FILE = "crates/prodex-provider-core/src/gemini_bridge/request_contents.rs";
const GEMINI_REQUEST_TRANSFORM_FILE = "crates/prodex-provider-core/src/translators/gemini/request_transform.rs";
const GEMINI_REQUEST_CONTENTS_FILE = "crates/prodex-provider-core/src/translators/gemini/request_contents.rs";
const GEMINI_BRIDGE_SIMPLE_FILE = "crates/prodex-provider-core/src/gemini_bridge/request/simple.rs";
const GEMINI_BRIDGE_NATIVE_PROJECT_FILE = "crates/prodex-provider-core/src/gemini_bridge/request/native_project.rs";
const GEMINI_BRIDGE_TOOLS_FILE = "crates/prodex-provider-core/src/gemini_bridge/request/tools.rs";
const GEMINI_TRANSLATOR_REQUEST_FILE = "crates/prodex-provider-core/src/translators/gemini/request.rs";
const GEMINI_TRANSLATOR_GENERATION_CONFIG_FILE = "crates/prodex-provider-core/src/translators/gemini/request/generation_config.rs";
const GEMINI_GENERATION_CONFIG_FILE = "crates/prodex-provider-core/src/translators/gemini/request/generation_config.rs";
const GEMINI_THINKING_FILE = "crates/prodex-provider-core/src/translators/gemini/request/generation_config/thinking.rs";
const ANTHROPIC_RESPONSE_FORBIDDEN_PATTERNS = [
  [/\bfn\s+anthropic_response_block_input\s*\(/u, "Rust response block classifier"],
  [/\benum\s+ResponseBlockKind\b/u, "duplicate Rust response block kind"],
  [/\benum\s+ResponsePlanKind\b/u, "duplicate Rust response plan kind"],
  [/\bstruct\s+ResponsePlanItem\b/u, "duplicate Rust response plan item"],
  [/\bstruct\s+ResponseBlockInput\b/u, "duplicate Rust response block input"],
  [/\bfn\s+plan_with_rust\s*\(/u, "Rust response planner"],
  [/\b(?:Some\s*\(\s*)?"(?:text|tool_use|server_tool_use|web_search_tool_result|thinking)"(?:\s*\))?\s*=>/u,
    "Rust response block classification"],
  [/\bResponsePlanItem\s*\{\s*kind\s*:\s*ResponsePlanKind\s*::/u,
    "Rust response plan construction", true],
  [FEATURE_OFF_RUST_PATH, "feature-off Rust response path"],
];

const FORBIDDEN_MARKERS = [
  "prodex_mojo_fallback",
  "use_rust_fallback",
  "rust_fallback",
  "fallback-to-rust",
];

export function findViolations(files) {
  const smartContextCapsuleOrderViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === SMART_CONTEXT_CAPSULE_ORDER_FILE) {
      const body = contents.match(
        /\bpub\(in crate::smart_context\) fn smart_context_select_memory_capsules_impl\([^]*?^\}/mu,
      )?.[0] ?? "";
      const violations = [];
      if (!body.includes("prodex_mojo_core::rich::order_smart_context_capsules(")) {
        violations.push(`${filePath}: memory-capsule ordering must use Mojo`);
      }
      if (!body.includes(".chunks(65_536)")) {
        violations.push(`${filePath}: memory-capsule admission must retain 65,536-item batches`);
      }
      if (FEATURE_OFF_RUST_PATH.test(body)) {
        violations.push(`${filePath}: memory-capsule ordering cannot have a feature-off Rust path`);
      }
      if (/\.sort(?:_by_key|_unstable_by_key|_unstable|_by)?\s*\(|smart_context_capsule_order\s*\(/u.test(body)) {
        violations.push(`${filePath}: contains restored Rust memory-capsule ordering policy`);
      }
      return violations;
    }
    if (filePath === SMART_CONTEXT_CAPSULE_ARTIFACTS_FILE &&
        /\bfn\s+smart_context_capsule_order\s*\(/u.test(contents)) {
      return [`${filePath}: contains restored Rust memory-capsule ordering policy`];
    }
    if (filePath === SMART_CONTEXT_CAPSULE_ORDER_ADAPTER_FILE) {
      const violations = [];
      if (!contents.includes("prodex_mojo_smart_context_capsule_order_v1(")) {
        violations.push(`${filePath}: memory-capsule order adapter must call the versioned Mojo ABI`);
      }
      if (!contents.includes("input.id.as_ptr()") ||
          !contents.includes("i64::try_from(input.id.len())") ||
          !contents.includes("SMART_CONTEXT_CAPSULE_ORDER_MAX_COUNT")) {
        violations.push(`${filePath}: memory-capsule ABI must use count-bounded per-ID pointers and checked lengths`);
      }
      if (/SMART_CONTEXT_CAPSULE_ORDER_MAX_ID_BYTES|4\s*\*\s*1024\s*\*\s*1024|4_?194_?304|\.as_bytes\s*\(/u.test(contents)) {
        violations.push(`${filePath}: memory-capsule ABI must not cap or copy aggregate ID bytes`);
      }
      return violations;
    }
    if (filePath === SMART_CONTEXT_CAPSULE_ORDER_MOJO_FILE) {
      const required = [
        '@export("prodex_mojo_smart_context_capsule_order_v1")',
        "def smart_context_capsule_order_before(",
        "SMART_CONTEXT_CAPSULE_ORDER_MAX_COUNT: Int64 = 65_537",
        "id_addresses_address: UInt",
        "id_lengths_address: UInt",
        "length < 0",
        "(length > 0 and address == 0)",
      ];
      const violations = required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: memory-capsule ordering must remain Mojo-owned (${marker})`);
      if (/SMART_CONTEXT_CAPSULE_ORDER_MAX_ID_BYTES|4\s*\*\s*1024\s*\*\s*1024|4_?194_?304/u.test(contents)) {
        violations.push(`${filePath}: memory-capsule ABI must not cap aggregate ID bytes`);
      }
      return violations;
    }
    return [];
  });
  const markerViolations = [
    ...files.flatMap(([filePath, contents]) =>
      FORBIDDEN_MARKERS.filter((marker) => contents.includes(marker)).map(
        (marker) => `${filePath}: promoted Mojo code contains ${marker}`,
      ),
    ),
    ...files.flatMap(([filePath, contents]) => {
      if (filePath !== DEEPSEEK_INPUT_HISTORY_FILE) return [];
      const between = (startName, endName) => {
        const start = contents.indexOf(`pub fn ${startName}(`);
        const end = contents.indexOf(`pub fn ${endName}(`, start);
        return start >= 0 && end > start ? contents.slice(start, end) : "";
      };
      const keyPlan = between(
        "deepseek_provider_core_first_function_call_output_call_id",
        "deepseek_provider_core_history_has_tool_call",
      );
      const historyMatch = between(
        "deepseek_provider_core_history_has_tool_call",
        "deepseek_provider_core_tool_call_ids",
      );
      const rustKeyCopy = /"(?:function_call_output|custom_tool_call_output|mcp_call_output|mcp_tool_result)"|find_map\s*\(/u.test(keyPlan);
      const rustHistoryCopy = /history\.iter\(\)\.any|tool_calls|\.get\("id"\)/u.test(historyMatch);
      return (
        keyPlan.includes("DeepSeekKernelOperation::ResponsesHistoryCallId")
        && historyMatch.includes("DeepSeekKernelOperation::ResponsesHistoryContainsCallId")
        && !rustKeyCopy
        && !rustHistoryCopy
      )
        ? []
        : [`${filePath}: DeepSeek input/history replay decisions must use the Mojo kernel`];
    }),
  ];
  const featureOffViolations = files
    .filter(([filePath, contents]) =>
      UNCONDITIONAL_MOJO_FILES.has(filePath) && FEATURE_OFF_RUST_PATH.test(contents),
    )
    .map(([filePath]) => `${filePath}: Mojo-owned operation cannot have a feature-off Rust path`);
  const liveLogRecordViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== LIVE_LOG_RECORD_FILE) return [];
    const production = contents.split("#[cfg(test)]", 1)[0];
    const required = [
      "record_exceeds_bound(line.len())?",
      "nested_string_clip_end(text)?",
      "json_plan(serialized.len())?",
      "truncate_plain_text(line)",
      "let line = bounded_live_log_line(line)?;",
    ];
    const missing = required.filter((marker) => !production.includes(marker));
    if (missing.length > 0) {
      return [`${filePath}: live-log record decisions must propagate through Mojo (${missing.join(", ")})`];
    }
    return /clip_json_strings|\.char_indices\s*\(|MAX_RUNTIME_LIVE_LOG_LINE_BYTES/u.test(production)
      ? [`${filePath}: contains restored Rust live-log clipping or truncation policy`]
      : [];
  });
  const runtimePolicyPresetViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === RUNTIME_POLICY_PRESET_CONSUMER_FILE) {
      const required = "runtime_tuning_proxy_preset_plan(";
      const violations = contents.includes(required)
        ? []
        : [`${filePath}: runtime-proxy preset resolution must use the Mojo plan`];
      if (/apply_non_preset_overrides|env_preset\.or_else\(\|\|\s*self\.preset\(\)\)/u.test(contents)) {
        violations.push(`${filePath}: contains restored Rust preset precedence or override merging`);
      }
      return violations;
    }
    if (filePath === RUNTIME_POLICY_PRESET_ADAPTER_FILE) {
      return contents.includes("prodex_runtime_proxy_preset_plan_v1(") &&
          contents.includes("RUNTIME_PROXY_PRESET_PLAN_ABI_VERSION")
        ? []
        : [`${filePath}: preset planning adapter must retain its versioned Mojo ABI`];
    }
    if (filePath === RUNTIME_POLICY_PRESET_MOJO_FILE) {
      return contents.includes('@export("prodex_runtime_proxy_preset_plan_v1")')
        ? []
        : [`${filePath}: runtime-proxy preset plan must be implemented in Mojo`];
    }
    if (filePath === RUNTIME_POLICY_PRESET_TEST_FILE) {
      return contents.includes("required_mojo_resolves_preset_precedence_and_all_overrides") &&
          contents.includes("prodex_mojo_required")
        ? []
        : [`${filePath}: preset ABI needs direct required-Mojo regression coverage`];
    }
    if (filePath === RUNTIME_POLICY_PRESET_CALLER_FILE) {
      return contents.includes("runtime_policy_proxy_caller_uses_mojo_preset_plan") &&
          contents.includes("runtime_policy_proxy_from_root(")
        ? []
        : [`${filePath}: runtime-policy caller needs a preset-plan regression test`];
    }
    return [];
  });
  const profileHealthCircuitViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== PROFILE_HEALTH_CIRCUIT_FILE) return [];
    const body = contents.match(
      /\bfn\s+runtime_profile_circuit_half_open_probe_seconds\s*\([^)]*\)\s*->\s*i64\s*\{([^{}]*)\}/u,
    )?.[1];
    return body?.includes("runtime_proxy_crate::runtime_profile_circuit_half_open_probe_seconds(")
      && !/\b(?:checked_shl|saturating_(?:sub|mul))\b/u.test(body)
      ? []
      : [`${filePath}: half-open profile-health timing must delegate to the Mojo runtime-health adapter`];
  });
  const exactnessPlannerViolations = files
    .filter(([filePath, contents]) => filePath === SMART_CONTEXT_CORE_FILE &&
      !contents.includes("prodex_mojo_core::runtime::smart_context_exactness_plan("))
    .map(([filePath]) => `${filePath}: Smart Context exactness must use the Mojo plan`);
  const adaptiveBudgetViolations = files
    .filter(([filePath, contents]) => filePath === ADAPTIVE_BUDGET_FILE &&
      !contents.includes("prodex_mojo_core::runtime::smart_context_adaptive_budget_plan("))
    .map(([filePath]) => `${filePath}: adaptive budget must use the Mojo plan`);
  const providerUsageViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-provider-core/src/usage.rs") {
      const required = [
        "prodex_mojo_core::provider_usage::extract_json(",
        "prodex_mojo_core::provider_usage::calculate_cost(",
        "prodex_mojo_core::provider_usage::merged_total(",
        "prodex_mojo_core::provider_usage::merge_latest_present(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": provider usage migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        /\bfn\s+first_u64\s*\(/u.test(production)
        || /saturating_mul\(rate\)\s*\/\s*1_000_000/u.test(production)
        || /self\.total_tokens\.or_else/u.test(production)
      ) {
        violations.push(filePath + ": contains restored Rust provider-usage semantics");
      }
      if (/if\s+\w+\.(input|output|total)_tokens\.is_some\(\)\s*\{\s*\w+\.\1_tokens\s*=/u.test(production)) {
        violations.push(filePath + ": contains restored Rust SSE usage merge policy");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/provider_usage.rs") {
      const required = [
        "prodex_provider_usage_extract_v1(",
        "prodex_provider_usage_cost_v1(",
        "prodex_provider_usage_merged_total_v1(",
        "prodex_provider_usage_merge_latest_present_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": provider usage ABI adapter must retain " + call);
    }
    return [];
  });
  const auditUsageViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-audit-log/src/lib.rs") {
      const required = [
        "prodex_mojo_core::audit_log_policy::normalize_usage_token(",
        "prodex_mojo_core::audit_log_policy::normalized_total_tokens(",
        "prodex_mojo_core::audit_log_policy::budget_window_plan(",
        "prodex_mojo_core::audit_log_policy::summarize_usage(",
        "prodex_mojo_core::audit_log_policy::budget_evaluation(",
        "prodex_mojo_core::audit_log_policy::query_has_filters(",
        "prodex_mojo_core::audit_log_policy::query_matches(",
        "prodex_mojo_core::audit_log_policy::format_query(",
        "prodex_mojo_core::audit_log_policy::format_search_scope(",
        "prodex_mojo_core::audit_log_policy::truncate_text(",
        "prodex_mojo_core::audit_log_policy::profile_name(",
        "prodex_mojo_core::audit_log_policy::account_hint(",
        "prodex_mojo_core::audit_log_policy::email_domain(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": audit usage hard replacement must retain " + call);
      if (
        /summary\.(?:requests|total_tokens|cost_micros)\s*>=/u.test(contents)
        || /self\.input_tokens\s*\.saturating_add/u.test(contents)
        || /\.chars\(\)[^;]{0,300}ch\.is_ascii_alphanumeric/u.test(contents)
        || contents.includes("self.component.is_some() || self.action.is_some()")
        || contents.includes(".is_none_or(|component| event.component == component)")
        || contents.includes('parts.push(format!("component={component}"))')
        || contents.includes('"searched {} of {} bytes (byte range {}..{})"')
        || contents.includes("chars.by_ref().take(max_chars)")
        || contents.includes(".chars().rev().take(4)")
        || contents.includes(".rsplit_once('@')")
        || contents.includes(".chars().take(100).collect()")
        || contents.includes('reasons.push(format!("request limit reached')
        || contents.includes('reasons.push(format!("token limit reached')
        || contents.includes('reasons.push(format!("cost limit reached')
        || contents.includes("fn floor_epoch(")
        || contents.includes("match self {")
      ) {
        violations.push(filePath + ": contains restored Rust audit query/display/metadata semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/audit_log_policy.rs") {
      const required = [
        "prodex_audit_usage_token_normalize_v1(",
        "prodex_audit_usage_total_v1(",
        "prodex_audit_budget_window_plan_v1(",
        "prodex_audit_usage_summary_v1(",
        "prodex_audit_budget_flags_v1(",
        "prodex_audit_budget_evaluation_v1(",
        "prodex_audit_query_has_filters_v1(",
        "prodex_audit_query_matches_v1(",
        "prodex_audit_query_format_v1(",
        "prodex_audit_search_scope_format_v1(",
        "prodex_audit_truncate_text_v1(",
        "prodex_audit_profile_name_v1(",
        "prodex_audit_account_hint_v1(",
        "prodex_audit_email_domain_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": audit usage ABI adapter must retain " + call);
    }
    return [];
  });
  const coreFilePolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-core/src/lib.rs") {
      const required = [
        "core_file_policy::owned_root_temp_file_name(",
        "core_file_policy::root_temp_file_pid(",
        "core_file_policy::stale_root_temp_file_should_remove(",
        "core_file_policy::runtime_proxy_log_file_name_is_owned(",
        "core_file_policy::login_temp_dir_name_is_owned(",
        "core_file_policy::runtime_broker_artifact_key_range(",
        "core_file_policy::runtime_broker_lease_pid(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": core file-policy hard replacement must retain " + call);
      if (
        /name\.starts_with\("state\.json\."\)/u.test(contents)
        || /name\.strip_suffix\("\.tmp"\)/u.test(contents)
        || /strip_prefix\("runtime-broker-"\)/u.test(contents)
        || /file_name\s*\.split\('-'\)/u.test(contents)
      ) {
        violations.push(filePath + ": contains restored Rust core file-policy semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/core_file_policy.rs" &&
        !contents.includes("prodex_core_file_policy_v1(")) {
      return [filePath + ": core file-policy ABI adapter must retain prodex_core_file_policy_v1("];
    }
    return [];
  });
  const mcpStdioPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-mcp-stdio/src/lib.rs") {
      const required = [
        "mcp_stdio_policy::header_is_content_length(",
        "mcp_stdio_policy::parse_content_length(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": MCP framing hard replacement must retain " + call);
      if (
        /to_ascii_lowercase\(\)\.starts_with\("content-length:"\)/u.test(contents)
        || /split_once\(':'\)/u.test(contents)
        || /parse::<usize>\(\)/u.test(contents)
      ) {
        violations.push(filePath + ": contains restored Rust MCP Content-Length semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/mcp_stdio_policy.rs") {
      const required = [
        "prodex_mcp_header_is_content_length_v1(",
        "prodex_mcp_content_length_parse_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": MCP framing ABI adapter must retain " + call);
    }
    return [];
  });
  const updateNoticeMigrationViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === UPDATE_NOTICE_VERSION_CALLER_FILE) {
      const required = [
        "update_notice_policy::should_emit_notice(",
        "update_notice_policy::install_channel(",
        "update_notice_policy::cache_is_fresh(",
        "release_version_is_valid(",
        "map_update_notice_mojo(",
        "Err(error) if is_update_notice_mojo_error(&error) => return Err(error)",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": update-notice hard replacement must retain " + call);
      if (
        /replace\('\\\\',\s*"\/"\)/u.test(contents)
        || /normalized_path\.contains/u.test(contents)
        || /now\.saturating_sub\(cached_checked_at\)/u.test(contents)
        || /semver::Version|Version::parse|parse_release_version|candidate\s*>\s*current/u.test(contents)
      ) {
        violations.push(filePath + ": contains restored Rust update-notice semantics");
      }
      return violations;
    }
    if (filePath === UPDATE_NOTICE_VERSION_UPDATER_FILE) {
      const required = [
        "update_notice_policy::update_decision(",
        "Err(error) if is_update_notice_mojo_error(&error) => return Err(error)",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": release-version caller must retain " + call);
      if (
        /semver::Version|Version::parse|parse_release_version|cmp_precedence/u.test(contents)
        || contents.includes("release_version_is_valid(")
        || contents.includes("compare_release_versions(")
      ) {
        violations.push(filePath + ": contains restored Rust release-version semantics");
      }
      return violations;
    }
    if (filePath === UPDATE_NOTICE_VERSION_ADAPTER_MODULE_FILE) {
      const required = [
        "update_notice_policy::release_version_is_valid(",
        "update_notice_policy::compare_release_versions(",
        "ReleaseVersionOrder::Total",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": release-version adapter must retain " + call);
      if (/semver::Version|Version::parse|parse_release_version|cmp_precedence/u.test(contents)) {
        violations.push(filePath + ": contains restored Rust release-version semantics");
      }
      return violations;
    }
    if (filePath === UPDATE_NOTICE_VERSION_ADAPTER_FILE) {
      const required = [
        "prodex_update_notice_policy_v1(",
        "release_version_is_valid(",
        "compare_release_versions(",
        "update_decision(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": update-notice ABI adapter must retain " + call);
    }
    if (filePath === UPDATE_NOTICE_VERSION_MOJO_FILE) {
      const required = [
        '@export("prodex_update_notice_policy_v1")',
        "update_notice_parse_release_version(",
        "update_notice_release_version_compare(",
        "UPDATE_NOTICE_RELEASE_VERSION_VALID",
        "UPDATE_NOTICE_RELEASE_VERSION_COMPARE",
        "UPDATE_NOTICE_UPDATE_DECISION",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": Mojo release-version owner must retain " + call);
    }
    return [];
  });
  const sharedAttachmentMigrationViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-shared-codex-fs/src/image_attachments.rs") {
      const required = [
        "shared_attachment_policy::image_tag_path_range(",
        "shared_attachment_policy::next_clipboard_path(",
        "shared_attachment_policy::next_attachment_path(",
        "shared_attachment_policy::clipboard_file_name(",
        "shared_attachment_policy::persistable_attachment_file_name(",
        "shared_attachment_policy::rollout_file_name(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": shared attachment hard replacement must retain " + call);
      if (
        /fn\s+image_tag_path_attr\s*\(/u.test(contents)
        || /fn\s+codex_session_path_continues\s*\(/u.test(contents)
        || /fn\s+is_codex_session_path_byte\s*\(/u.test(contents)
        || /CODEX_ATTACHMENT_PATH_MARKERS/u.test(contents)
      ) {
        violations.push(filePath + ": contains restored Rust shared-attachment scanner semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/shared_attachment_policy.rs" &&
        !contents.includes("prodex_shared_attachment_policy_v1(")) {
      return [filePath + ": shared attachment ABI adapter must retain prodex_shared_attachment_policy_v1("];
    }
    return [];
  });
  const runtimeCookieMigrationViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-runtime-cookies/src/lib.rs") {
      const required = [
        "runtime_cookie_policy::set_cookie_pair(",
        "runtime_cookie_policy::caller_cookie_name(",
        "runtime_cookie_policy::attribute_plan(",
        "runtime_cookie_policy::default_path_plan(",
        "runtime_cookie_policy::path_matches(",
        "runtime_cookie_policy::scheme_is_secure(",
        "runtime_cookie_policy::normalize_host(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime-cookie hard replacement must retain " + call);
      if (
        /fn\s+runtime_proxy_cookie_(?:apply_max_age|name_is_safe|value_is_safe)\s*\(/u.test(contents)
        || /host\.trim_matches\('\.'\)\.to_ascii_lowercase\(\)/u.test(contents)
        || /request_path\s*\.strip_prefix\(cookie_path\)/u.test(contents)
      ) {
        violations.push(filePath + ": contains restored Rust runtime-cookie text semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/runtime_cookie_policy.rs") {
      const required = [
        "prodex_runtime_cookie_pair_plan_v1(",
        "prodex_runtime_cookie_attribute_plan_v1(",
        "prodex_runtime_cookie_default_path_v1(",
        "prodex_runtime_cookie_path_matches_v1(",
        "prodex_runtime_cookie_scheme_secure_v1(",
        "prodex_runtime_cookie_host_normalize_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime-cookie ABI adapter must retain " + call);
    }
    return [];
  });
  const runtimeCookiePolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-runtime-cookies/src/lib.rs") {
      const required = [
        "runtime_cookie_policy::set_cookie_pair(",
        "runtime_cookie_policy::attribute_plan(",
        "runtime_cookie_policy::normalize_host(",
        "runtime_cookie_policy::scheme_is_secure(",
        "runtime_cookie_policy::default_path_plan(",
        "runtime_cookie_policy::caller_cookie_name(",
        "runtime_cookie_policy::path_matches(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime-cookie hard replacement must retain " + call);
      if (
        /fn\s+runtime_proxy_cookie_(?:apply_attribute|apply_max_age|name_is_safe|value_is_safe)\s*\(/u.test(contents)
        || /strip_prefix\('-'\)\.unwrap_or/u.test(contents)
        || /trim_matches\('\.'\)\.to_ascii_lowercase/u.test(contents)
      ) {
        violations.push(filePath + ": contains restored Rust runtime-cookie semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/runtime_cookie_policy.rs") {
      const required = [
        "prodex_runtime_cookie_pair_plan_v1(",
        "prodex_runtime_cookie_attribute_plan_v1(",
        "prodex_runtime_cookie_default_path_v1(",
        "prodex_runtime_cookie_path_matches_v1(",
        "prodex_runtime_cookie_scheme_secure_v1(",
        "prodex_runtime_cookie_host_normalize_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime-cookie ABI adapter must retain " + call);
    }
    return [];
  });
  const sharedHistoryPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-shared-codex-fs/src/history.rs") {
      const required = [
        "shared_history_policy::dedup_plan(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": shared history hard replacement must retain " + call);
      if (
        contents.includes("let mut seen = BTreeSet::new()")
        || contents.includes("!seen.insert(line.to_string())")
        || contents.includes("let next_len = content")
      ) {
        violations.push(filePath + ": contains restored Rust shared-history dedup/size semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/shared_history_policy.rs" &&
        !contents.includes("prodex_shared_history_dedup_plan_v1(")) {
      return [filePath + ": shared history ABI adapter must retain prodex_shared_history_dedup_plan_v1("];
    }
    return [];
  });
  const deepseekSimpleRequestViolations = files
    .filter(([filePath, contents]) => filePath === DEEPSEEK_SIMPLE_REQUEST_FILE &&
      (!contents.includes("DeepSeekRequestPolicyOperation::SimpleRequest") ||
        !contents.includes("prodex_mojo_core::rich::deepseek_request_policy(")))
    .map(([filePath]) => `${filePath}: DeepSeek simple-request eligibility must use Mojo`);
  const deepseekMetadataViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== DEEPSEEK_METADATA_FILE) return [];
    const required = [
      "DeepSeekKernelOperation::RequestMetadata",
      "DeepSeekKernelOperation::ResponseFormat",
      "DeepSeekRequestPolicyOperation::ResponseFormatShape",
      "DeepSeekRequestPolicyOperation::MetadataShape",
      "DeepSeekRequestPolicyOperation::JsonGuidance",
    ];
    const violations = required
      .filter((marker) => !contents.includes(marker))
      .map((marker) => filePath + ": DeepSeek metadata/format migration must retain " + marker);
    if (
      /\bfn\s+deepseek_provider_core_message_has_json_guidance\s*\(/u.test(contents)
      || /\bmatch\s+format_type\s*\{/u.test(contents)
      || /to_ascii_lowercase\(\)\.contains\(["']json["']\)/u.test(contents)
    ) {
      violations.push(filePath + ": contains restored Rust DeepSeek metadata/format semantics");
    }
    return violations;
  });
  const kiroChatResponseViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== KIRO_CHAT_RESPONSE_FILE) return [];
    const start = contents.indexOf("pub fn kiro_provider_core_chat_completion_value_from_response");
    const end = contents.indexOf("\npub fn kiro_provider_core_apply_response_runtime_metadata", start);
    const mapper = start < 0 ? "" : contents.slice(start, end < 0 ? undefined : end);
    return mapper.includes("kiro_provider_core_try_chat_completion_value_from_response") &&
      mapper.includes("kiro_rewrite_chat_response_json(") &&
      !FEATURE_OFF_RUST_PATH.test(mapper) &&
      !/response\.(?:get|pointer)\s*\(|output\.(?:iter|get)\s*\(|json!\s*\(|kiro_provider_core_chat_completion_finish_reason\s*\(/u.test(mapper)
      ? [] : [`${filePath}: Kiro chat response mapping must use Mojo without a Rust copy`];
  });
  const kiroFinalStreamViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== KIRO_FINAL_STREAM_FILE) return [];
    const start = contents.indexOf("pub(super) fn runtime_kiro_send_final_stream");
    if (start < 0) return [`${filePath}: Kiro terminal stream path must use Mojo final event plan`];
    const end = contents.indexOf("\n}\n", start);
    const functionBody = contents.slice(start, end < 0 ? undefined : end);
    const violations = [];
    if (!functionBody.includes("kiro_provider_core_response_final_event(")) {
      violations.push(`${filePath}: Kiro terminal stream path must use Mojo final event plan`);
    }
    if (/response\s*\.\s*get\(\s*"status"\s*\)/u.test(functionBody)) {
      violations.push(`${filePath}: contains restored Rust Kiro terminal status selection`);
    }
    return violations;
  });
  const kiroStreamPlanViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== KIRO_STREAM_FILE) return [];
    const start = contents.indexOf("pub fn kiro_provider_core_response_final_event(");
    if (start < 0) return [`${filePath}: Kiro final event helper must use Mojo final event plan`];
    const end = contents.indexOf("\n}\n", start);
    const helper = contents.slice(start, end < 0 ? undefined : end);
    return helper.includes("KiroKernelOperation::ResponseFinalEvent") &&
      helper.includes("kiro_mojo_value(input)")
      ? [] : [`${filePath}: Kiro final event helper must use Mojo final event plan`];
  });
  const kiroResponseHelperViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== KIRO_CHAT_RESPONSE_FILE) return [];
    const violations = KIRO_RESPONSE_HELPER_OPERATIONS
      .filter((marker) => !contents.includes(marker))
      .map((marker) => `${filePath}: Kiro response hard replacement must retain ${marker}`);
    const start = contents.indexOf("pub fn kiro_provider_core_anthropic_message_value_from_response");
    const mapper = start < 0 ? "" : contents.slice(start);
    if (
      !mapper.includes("kiro_rewrite_anthropic_response_json(")
      || FEATURE_OFF_RUST_PATH.test(mapper)
      || /response\.(?:get|pointer)\s*\(|output\.(?:iter|get)\s*\(|KiroKernelOperation::Anthropic(?:ToolUseBlock|Response)|kiro_provider_core_stream_content_text/u.test(mapper)
    ) {
      violations.push(`${filePath}: Kiro Anthropic response mapping must use one raw Mojo rewrite without a Rust copy`);
    }
    if (/\bfn\s+kiro_provider_core_anthropic_(?:stop_reason|tool_use_block)\s*\(/u.test(contents)) {
      violations.push(`${filePath}: contains replaced Rust Kiro Anthropic response semantics`);
    }
    return violations;
  });
  const kiroAcpViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== KIRO_ACP_FILE) return [];
    const violations = KIRO_ACP_OPERATIONS
      .filter((marker) => !contents.includes(marker))
      .map((marker) => `${filePath}: Kiro ACP hard replacement must retain ${marker}`);
    if (contents.includes("KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_EVENTS") ||
        contents.includes("kiro_provider_core_truncated_tool_activity_item") ||
        contents.includes("serde_json::json")) {
      violations.push(`${filePath}: contains restored Rust Kiro ACP shaping semantics`);
    }
    return violations;
  });
  const kiroMessageShapeViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === KIRO_REQUEST_FILE) {
      const violations = contents.includes("kiro_rewrite_chat_request_json(")
        ? []
        : [`${filePath}: Kiro chat request rewrite must use the raw Mojo rewrite`];
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        production.includes("kiro_rewrite_legacy_chat_tools")
        || production.includes('object.remove("functions")')
        || production.includes('object.remove("function_call")')
      ) {
        violations.push(`${filePath}: contains restored Rust Kiro legacy chat-tool rewrite semantics`);
      }
      return violations;
    }
    if (filePath === KIRO_MESSAGES_FILE) {
      const required = [
        "KiroKernelOperation::PromptFromChatMessages",
        "KiroKernelOperation::LegacyFunctionTool",
        "KiroKernelOperation::LegacyToolChoice",
      ];
      const violations = required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Kiro message shaping must use ${marker}`);
      if (
        /message\.get\(|function\.get\(|kiro_provider_core_prompt_(?:section|message_text|array_text|object_text|role_label)\s*\(/u.test(contents)
        || /\.trim\(\)/u.test(contents)
      ) {
        violations.push(`${filePath}: contains replaced Rust Kiro prompt or legacy-function semantics`);
      }
      return violations;
    }
    if (filePath === KIRO_LOCAL_REWRITE_FILE) {
      return contents.includes(
        "kiro_provider_core_prompt_from_chat_messages as runtime_kiro_prompt_from_messages",
      ) && contents.includes("runtime_kiro_prompt_from_messages(&translated.messages)")
        ? []
        : [`${filePath}: local Kiro chat rewrite must reach the Mojo-backed prompt adapter`];
    }
    if (filePath === KIRO_PROMPT_ABI_TEST_FILE) {
      const required = [
        "KiroKernelOperation::PromptFromChatMessages",
        "kiro_prompt_recursively_applies_text_content_output_precedence_and_unicode_trim",
        "kiro_prompt_joins_only_nonempty_sections_and_uses_empty_fallback",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Kiro prompt ABI tests must retain ${marker}`);
    }
    return [];
  });
  const kiroCatalogViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === KIRO_CATALOG_NORMALIZER_FILE) {
      const required = [
        "kiro_model_catalog_plan(",
        "KiroModelCatalogPlan::Ready",
        "merge_catalog_ids(",
        "merge_provider_model_catalog_json(ProviderId::Kiro",
      ];
      const violations = required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Kiro catalog adapter must retain ${marker}`);
      if (/(?:first_models_array|first_nonempty_array|first_nonempty_string|first_positive_u64)|\.trim\(\)/u.test(contents)) {
        violations.push(`${filePath}: contains replaced Rust Kiro model-catalog decisions`);
      }
      return violations;
    }
    if (filePath === KIRO_CATALOG_APP_ADAPTER_FILE) {
      const required = [
        "prodex_provider_core::normalize_kiro_model_catalog(value)",
        "prodex_provider_core::normalize_kiro_model_catalog_models(models)",
      ];
      const violations = required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Kiro catalog caller must use ${marker}`);
      if (/(?:first_models_array|first_nonempty_array|first_nonempty_string|first_positive_u64)|merge_catalog_ids|\.trim\(\)/u.test(contents)) {
        violations.push(`${filePath}: contains replaced Rust Kiro model-catalog decisions`);
      }
      return violations;
    }
    if (filePath === KIRO_CATALOG_ABI_FILE) {
      return contents.includes("fn prodex_mojo_kiro_catalog_normalize_v1(")
        && contents.includes("pub fn kiro_model_catalog_plan(")
        ? []
        : [`${filePath}: Kiro catalog plan must use its versioned Mojo ABI`];
    }
    if (filePath === KIRO_CATALOG_MOJO_FILE) {
      return contents.includes("KIRO_CATALOG_ABI_VERSION: Int64 = 1")
        && contents.includes("def kiro_model_catalog_normalize_v1(")
        ? []
        : [`${filePath}: Kiro model-catalog planner must retain ABI v1`];
    }
    if (filePath === KIRO_CATALOG_ABI_TEST_FILE) {
      const required = [
        "kiro_model_catalog_plan_uses_alias_precedence_unicode_trim_and_stable_source_order",
        "kiro_catalog_plan_returns_typed_missing_empty_and_limit_issues",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: required-Mojo Kiro catalog test must retain ${marker}`);
    }
    return [];
  });
  const deepseekStrictSchemaViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === DEEPSEEK_STRICT_TOOLS_FILE) {
      const violations = contents.includes("DeepSeekKernelOperation::StrictFunctionSchema")
        ? [] : [`${filePath}: strict schema normalization must use Mojo`];
      if (/\bdeepseek_provider_core_(?:validate_strict_schema|reject_strict_schema_keywords)\b/u.test(contents)) {
        violations.push(`${filePath}: contains a Rust strict-schema validator`);
      }
      return violations;
    }
    if (filePath === DEEPSEEK_STRICT_SCHEMA_FILE &&
      /\bdeepseek_provider_core_(?:validate_strict_schema|reject_strict_schema_keywords|sanitize_strict_(?:schema|object_schema))\b/u.test(contents)) {
      return [`${filePath}: contains replaced Rust strict-schema behavior`];
    }
    return [];
  });
  const quotaModelPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== QUOTA_MODELS_FILE) return [];
    const required = [
      "prodex_mojo_core::quota::quota_report_sort_next(",
      "prodex_mojo_core::quota::quota_report_sort_label(",
      "prodex_mojo_core::quota::quota_auth_filter_parse(",
      "prodex_mojo_core::quota::quota_auth_filter_matches(",
      "prodex_mojo_core::quota::plan_capacity_pressure_scale_bps(",
      "prodex_mojo_core::quota::scale_quota_pressure_for_plan(",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => filePath + ": quota model policy must retain Mojo call " + call);
    const labelBody = contents.match(/pub fn label\(self\)\s*->\s*&'static str\s*\{[^]*?^\s*\}/mu)?.[0];
    if (labelBody?.includes("match self")) {
      violations.push(filePath + ": contains restored Rust quota report sort label mapping");
    }
    return violations;
  });
  const quotaPlannerViolations = files
    .filter(([filePath, contents]) => filePath === RUNTIME_QUOTA_FILE &&
      (!contents.includes("mojo::quota_snapshot_plan(") ||
        !contents.includes("mojo::quota_gate_plan(")))
    .map(([filePath]) => `${filePath}: quota snapshot and gate decisions must use Mojo`);
  const anthropicResponseViolations = files.flatMap(([filePath, contents]) =>
    filePath !== ANTHROPIC_RESPONSE_FILE
      ? []
      : ANTHROPIC_RESPONSE_FORBIDDEN_PATTERNS
        .filter(([pattern, , productionOnly]) => pattern.test(
          productionOnly ? contents.split("#[cfg(test)]", 1)[0] : contents,
        ))
        .map(([, reason]) => `${filePath}: contains ${reason}`),
  );
  const anthropicEnvelopeViolations = files
    .filter(([filePath, contents]) => filePath === ANTHROPIC_MESSAGES_FILE &&
      /\bfn\s+(?:anthropic_response_envelope_rust|anthropic_usage)\s*\(/u.test(contents))
    .map(([filePath]) => `${filePath}: contains a Rust response envelope implementation`);
  const anthropicRequestViolations = files.flatMap(([filePath, contents]) => {
    if (REMOVED_ORACLE_FILES.includes(filePath)) {
      return [`${filePath}: retained Rust fallback or oracle`];
    }
    if (filePath === ANTHROPIC_MESSAGES_FILE &&
      /\bfn\s+(?:translate_chat_request_to_anthropic_rust|anthropic_messages|append_anthropic_message|anthropic_message_blocks|anthropic_tool_call_blocks|anthropic_tool_call_block|anthropic_tools|anthropic_tool_choice|anthropic_tool_name)\s*\(/u.test(contents)) {
      return [`${filePath}: contains Rust Anthropic request semantics`];
    }
    if (filePath === ANTHROPIC_WEB_SEARCH_FILE &&
      /\bfn\s+(?:anthropic_web_search_tool|validate_anthropic_web_search_options)\s*\(/u.test(contents)) {
      return [`${filePath}: contains Rust Anthropic request web-search semantics`];
    }
    return [];
  });
  const anthropicWebSearchViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === ANTHROPIC_SSE_FILE &&
      (!contents.includes("prodex_provider_core::anthropic_web_search_stream_item") ||
        /\bfn\s+anthropic_web_search_stream_sources\s*\(/u.test(contents) ||
        contents.includes("Anthropic Messages stream translation requires Mojo support"))) {
      return [`${filePath}: live Anthropic web-search shaping must use Mojo in every feature mode`];
    }
    if (filePath === ANTHROPIC_WEB_SEARCH_FILE &&
      (!contents.includes("AnthropicRequestKernelOperation::WebSearchCall") ||
        !contents.includes("AnthropicRequestKernelOperation::WebSearchResult") ||
        /\bfn\s+anthropic_web_search_sources\s*\(/u.test(contents))) {
      return [`${filePath}: Anthropic web-search result shaping must use Mojo`];
    }
    if (filePath === ANTHROPIC_MESSAGES_FILE &&
      contents.includes("Anthropic Messages web-search result translation requires Mojo support")) {
      return [`${filePath}: Anthropic web-search result has a feature-off rejection`];
    }
    return [];
  });
  const quotaSelectionPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== SELECTION_POLICY_FILE) return [];
    const functions = [
      "runtime_selection_quota_pressure_band_reason",
      "runtime_quota_precommit_floor_percent_for_route",
      "runtime_quota_window_precommit_guard",
      "runtime_quota_precommit_guard_reason",
      "runtime_quota_window_usable_for_auto_rotate",
      "runtime_quota_summary_allows_soft_affinity",
      "runtime_quota_soft_affinity_rejection_reason",
    ];
    const violations = [];
    for (const name of functions) {
      const body = contents.match(new RegExp(`\\bpub fn ${name}\\([^]*?^\\}`, "mu"))?.[0];
      if (!body?.includes("runtime_quota_selection_policy_code(") &&
          !body?.includes("runtime_quota_summary_policy_code(")) {
        violations.push(`${filePath}: ${name} must retain Mojo quota-selection dispatch`);
      }
    }
    if (!contents.includes("prodex_mojo_core::runtime::quota_selection_policy(")) {
      violations.push(`${filePath}: quota selection adapter must call prodex-mojo-core`);
    }
    for (const marker of [
      "runtime_soft_affinity_policy_reason_label(",
      "runtime_affinity_unavailable_reason_label(",
      "runtime_affinity_selection_kind_label(",
    ]) {
      if (!contents.includes(marker)) {
        violations.push(`${filePath}: selection label adapter must retain ${marker}`);
      }
    }
    if (!contents.includes("pub use prodex_mojo_core::runtime::{")) {
      violations.push(`${filePath}: WebSocket selection result types must be re-exported from prodex-mojo-core`);
    }
    for (const localMirror of [
      "pub enum RuntimeWebsocketTransportFailurePlan",
      "pub struct RuntimeWebsocketFailureDispositionPlan",
      "pub enum RuntimeWebsocketInvalidPreviousResponseAction",
      "pub enum RuntimeWebsocketChainReuseReason",
      "pub struct RuntimeWebsocketInvalidPreviousResponsePlan",
      "pub enum RuntimeWebsocketQuotaFallbackPlan",
    ]) {
      if (contents.includes(localMirror)) {
        violations.push(`${filePath}: contains restored Rust WebSocket selection type mirror ${localMirror}`);
      }
    }
    return violations;
  });
  const appSelectionPolicyMirrorViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== "crates/prodex-app/src/runtime_proxy/selection/policy.rs") return [];
    const violations = [];
    if (!contents.includes("RuntimeCandidateAffinity,")) {
      violations.push(filePath + ": app selection policy must reuse canonical RuntimeCandidateAffinity");
    }
    if (contents.includes("pub(crate) struct RuntimeCandidateAffinity")) {
      violations.push(filePath + ": contains restored app RuntimeCandidateAffinity mirror");
    }
    if (contents.includes("runtime_candidate_affinity_to_proxy(")) {
      violations.push(filePath + ": contains restored RuntimeCandidateAffinity conversion mirror");
    }
    return violations;
  });
  const runtimeStateBackgroundViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== RUNTIME_STATE_BACKGROUND_FILE) return [];
    const required = [
      "prodex_mojo_core::runtime_state::mutation_policy(",
      "prodex_mojo_core::runtime_state::queue_pressure_active(",
      "prodex_mojo_core::runtime_state::enqueue_backlog(",
      "prodex_mojo_core::runtime_state::queue_threshold(",
      "prodex_mojo_core::runtime_state::queue_enqueue_plan(",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => `${filePath}: runtime-state migration must retain Mojo call ${call}`);
    const functions = [
      "runtime_state_save_requires_continuation_journal",
      "runtime_state_save_sections",
      "runtime_hot_continuation_state_mutation",
    ];
    for (const name of functions) {
      const body = contents.match(new RegExp(`\\bpub fn ${name}\\([^]*?^\\}`, "mu"))?.[0];
      if (!body?.includes("runtime_state_mutation_policy(")) {
        violations.push(`${filePath}: ${name} must retain Mojo mutation-policy dispatch`);
      }
    }
    if (/\bfn\s+(?:runtime_state_save_sections_rust|runtime_state_save_requires_continuation_journal_rust|runtime_hot_continuation_state_mutation_rust|runtime_background_queue_enqueue_plan_rust)\s*\(/u.test(contents)) {
      violations.push(`${filePath}: contains restored Rust runtime-state policy semantics`);
    }
    return violations;
  });
  const runtimeStateQuotaViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== RUNTIME_STATE_QUOTA_FILE) return [];
    const required = [
      "prodex_mojo_core::runtime_state_quota::timestamp_touch_should_persist(",
      "prodex_mojo_core::runtime_state_quota::probe_cache_freshness(",
      "prodex_mojo_core::runtime_state_quota::snapshot_usability(",
      "prodex_mojo_core::runtime_state_quota::probe_usage_snapshot_apply_plan(",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => filePath + ": runtime-state quota migration must retain Mojo call " + call);
    if (/now\.saturating_sub\(checked_at\)|now\.saturating_sub\(timestamp\)/u.test(contents)) {
      violations.push(filePath + ": contains restored Rust runtime-state quota timing policy");
    }
    return violations;
  });
  const runtimeProxyRootViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== RUNTIME_PROXY_ROOT_FILE) return [];
    const required = [
      "prodex_mojo_core::json::runtime_proxy_request_metadata(",
      "prodex_mojo_core::rich::runtime_proxy_path_plan(",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => filePath + ": runtime proxy root migration must retain Mojo call " + call);
    if (/\bfn\s+(?:runtime_proxy_legacy_version_segment|runtime_request_value_previous_response_input_item_is_tool_output|runtime_input_is_reconstructable_full_history)\s*\(/u.test(contents)) {
      violations.push(filePath + ": contains retired Rust runtime proxy path/request semantic helper");
    }
    if (/\.get\("previous_response_id"\)[^;]{0,500}previous_response_fresh_fallback_shape/su.test(contents)) {
      violations.push(filePath + ": contains restored Rust previous-response request-shape semantics");
    }
    return violations;
  });
  const logThroughputViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === RUNTIME_LOG_RETENTION_FILE) {
      const required = [
        "mojo_retention::bounded_text_policy_value(",
        "selection::remove_expired_runtime_logs(",
        "selection::remove_over_budget_runtime_logs(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime-log policy must retain " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (production.includes(".parse::<u64>()")) {
        violations.push(filePath + ": contains restored Rust runtime-log environment numeric parser");
      }
      if (
        /\bfn\s+remove_(?:expired|over_budget)_runtime_logs\s*\(/u.test(production)
        || /\.sort_by(?:_key)?\s*\(/u.test(production)
        || production.includes("log_expired_removal_allowed(")
        || production.includes("log_over_budget_plan(")
      ) {
        violations.push(filePath + ": contains restored Rust runtime-log retention selection semantics");
      }
      return violations;
    }
    if (filePath === RUNTIME_LOG_RETENTION_SELECTION_FILE) {
      const required = [
        "mojo_retention::log_expired_candidate_plan(",
        "mojo_retention::log_over_budget_candidate_plan(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime-log candidate selection must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        /\.(?:sort|sort_by|sort_by_key|sort_unstable|sort_unstable_by|sort_unstable_by_key)\s*\(/u.test(production)
        || /\bmodified_epoch_seconds\s*(?:<=|<)\s*oldest_allowed\b/u.test(production)
        || /\*?\bremaining_count\s*(?:<=|>|<|>=)\s*(?:policy\.)?max_files\b/u.test(production)
        || /\*?\btotal_bytes\s*(?:<=|>|<|>=)\s*(?:policy\.)?total_bytes\b/u.test(production)
        || /\bfn\s+(?:log_expired_removal_allowed|log_over_budget_plan)\s*\(/u.test(production)
      ) {
        violations.push(filePath + ": contains restored Rust runtime-log retention selection semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-app/src/app_commands/log_throughput_state.rs") {
      const required = [
        "prodex_mojo_core::log_throughput_policy::observation_plan(",
        "prodex_mojo_core::log_throughput_policy::duplicate_live_disk_replay(",
        "prodex_mojo_core::log_throughput_policy::select_active_profile_candidate(",
        "prodex_mojo_core::log_throughput_policy::select_active_rate_candidate(",
        "prodex_mojo_core::log_throughput_policy::select_live_identity_candidate(",
        "prodex_mojo_core::log_throughput_policy::select_historical_identity_candidate(",
        "prodex_mojo_core::log_throughput_policy::sample_expired(",
        "prodex_mojo_core::log_throughput_policy::bounded_insert_needs_eviction(",
        "prodex_mojo_core::log_throughput_policy::finish_rate_candidate(",
        "prodex_mojo_core::log_throughput_policy::completed_rate(",
        "prodex_mojo_core::log_throughput_policy::stream_rate(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": log-throughput migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        production.includes("OUTPUT_THROUGHPUT_MIN_SAMPLE")
        || /output_tokens as f64 \* 1_000\.0 \/ duration as f64/u.test(production)
        || /checked_sub\(\*first_generation_ms\)/u.test(production)
        || /\.max_by(?:_key)?\s*\(/u.test(production)
        || production.includes("OUTPUT_THROUGHPUT_WINDOW")
        || production.includes("OUTPUT_THROUGHPUT_MAX_STREAMS")
        || production.includes("OUTPUT_THROUGHPUT_MAX_OBSERVATIONS")
        || production.includes('starts_with("broker:")')
        || production.includes('starts_with("direct:")')
      ) {
        violations.push(filePath + ": contains restored Rust log-throughput counter/rate semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/log_throughput_policy.rs") {
      const required = [
        "prodex_log_throughput_sample_plan_v1(",
        "prodex_log_throughput_completed_rate_v1(",
        "prodex_log_throughput_stream_rate_v1(",
        "prodex_log_retention_policy_v1(",
        "prodex_log_retention_candidates_v1(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": log-throughput ABI adapter must retain " + call);
      if (/\b(?:log_expired_removal_allowed|log_over_budget_plan)\s*\(/u.test(contents)) {
        violations.push(filePath + ": contains retired Rust runtime-log retention policy APIs");
      }
      return violations;
    }
    return [];
  });
  const brokerVersionGuardViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === BROKER_VERSION_GUARD_FILE) {
      const required = [
        "prodex_mojo_core::runtime_broker_continuity::binary_identity_present(",
        "prodex_mojo_core::runtime_broker_continuity::binary_identity_matches(",
        "prodex_mojo_core::runtime_broker_continuity::binary_identity_replacement_reason(",
        "prodex_mojo_core::runtime_broker_continuity::binary_identity_version_mismatch(",
        "prodex_mojo_core::runtime_broker_continuity::version_guard_plan(",
        "prodex_mojo_core::runtime_broker_continuity::parse_prodex_version(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": broker version-guard migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        /if let \(Some\(current_sha256\), Some\(other_sha256\)\)/u.test(production)
        || /split_whitespace\(\)/u.test(production)
        || /active_requests > 0 \|\| live_leases > 0/u.test(production)
      ) {
        violations.push(filePath + ": contains restored Rust broker version-guard semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/runtime_broker_continuity.rs") {
      const required = [
        "prodex_runtime_broker_identity_policy_v1(",
        "prodex_runtime_broker_guard_plan_v1(",
        "prodex_runtime_broker_parse_version_v1(",
        "prodex_runtime_broker_log_cache_relation_v1(",
        "prodex_runtime_broker_lru_evict_index_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": broker version-guard ABI adapter must retain " + call);
    }
    return [];
  });
  const brokerContinuityViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== BROKER_CONTINUITY_FILE) return [];
    const required = [
      "prodex_mojo_core::runtime_broker_continuity::continuity_line_plan",
      "prodex_mojo_core::runtime_broker_continuity::effective_score",
      "prodex_mojo_core::runtime_broker_continuity::stale_verified",
      "prodex_mojo_core::runtime_broker_continuity::route_kind",
      "prodex_mojo_core::runtime_broker_continuity::health_key_kind",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => `${filePath}: broker continuity migration must retain Mojo call ${call}`);
    if (/\bfn\s+(?:runtime_broker_continuation_status_last_event_at|runtime_broker_continuity_failure_event|runtime_broker_continuity_failure_reason|runtime_broker_known_continuity_failure_event|runtime_broker_parse_json_string|runtime_broker_log_field_value|runtime_broker_skip_log_whitespace|runtime_broker_skip_log_field_value|runtime_broker_parse_log_field_value)\s*\(/u.test(contents)) {
      violations.push(`${filePath}: contains retired Rust broker continuity parsing/policy semantics`);
    }
    return violations;
  });
  const brokerLogCacheViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== BROKER_LOG_CACHE_FILE) return [];
    const required = [
      "log_fingerprint_relation(",
      "lru_evict_index(",
      "continuity_event_kind(",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => filePath + ": broker-log cache hard replacement must retain Mojo call " + call);
    if (
      contents.includes("current.len < previous.len || current.modified_at < previous.modified_at")
      || contents.includes(".min_by_key(|(_, entry)| entry.last_used_at)")
      || contents.includes('"chain_retried_owner" | "chain_dead_upstream_confirmed" | "stale_continuation"')
    ) {
      violations.push(filePath + ": contains restored Rust broker-log cache semantics");
    }
    return violations;
  });
  const codexConfigViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== CODEX_CONFIG_FILE) return [];
    const required = [
      "prodex_mojo_core::codex_config::profile_name_valid(",
      "prodex_mojo_core::codex_config::profile_v2_name(",
      "prodex_mojo_core::codex_config::config_override(",
      "prodex_mojo_core::codex_config::normalize_value(",
      "prodex_mojo_core::codex_config::model_provider_plan(",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => `${filePath}: Codex config migration must retain Mojo call ${call}`);
    if (/\bfn\s+(?:parse_config_override_string|parse_config_override_exact_string)\s*\(/u.test(contents) ||
        /while\s+index\s*<\s*args\.len\(\)/u.test(contents)) {
      violations.push(`${filePath}: contains retired Rust Codex config argument scanning semantics`);
    }
    const providerResolution = contents.match(
      /\bpub fn codex_non_openai_model_provider_with_profile_v2\([^]*?^\}/mu,
    )?.[0] ?? "";
    if (!providerResolution.includes("prodex_mojo_core::codex_config::model_provider_plan(")) {
      violations.push(`${filePath}: model-provider resolution must retain Mojo plan dispatch`);
    }
    if (/\bfn\s+codex_model_provider_setting_from_config\s*\(/u.test(contents)) {
      violations.push(`${filePath}: contains restored Rust model-provider source selection`);
    }
    return violations;
  });
  const statePolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === STATE_REMEMBER_FILE) {
      const required = "prodex_mojo_core::state_policy::binding_merge_plan(";
      const violations = contents.includes(required)
        ? [] : [`${filePath}: hard-binding remember must retain Mojo state policy`];
      const rememberBody = contents.match(/fn remember_hard_binding\([^]*?^\}/mu)?.[0] ?? "";
      if (
        rememberBody.includes("binding.profile_name == profile_name =>")
        || rememberBody.includes("binding.binding_identity.as_ref().zip(binding_identity)")
           && !rememberBody.includes(required)
      ) {
        violations.push(`${filePath}: contains restored Rust hard-binding remember semantics`);
      }
      return violations;
    }
    if (filePath === STATE_PROVIDER_FILE) {
      const required = "prodex_mojo_core::state_policy::provider_capabilities(";
      const violations = contents.includes(required)
        ? [] : [`${filePath}: provider capabilities must retain Mojo state policy`];
      if (contents.includes("ProviderCapabilities::new(")) {
        violations.push(`${filePath}: contains restored Rust provider capability table`);
      }
      return violations;
    }
    if (filePath !== STATE_FILE) return [];
    const required = [
      "prodex_mojo_core::state_policy::last_run_selection_keep(",
      "prodex_mojo_core::state_policy::binding_merge_plan(",
      "prodex_mojo_core::state_policy::binding_keep(",
      "prodex_mojo_core::state_policy::active_profile_choice(",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => `${filePath}: state migration must retain Mojo call ${call}`);
    if (/\blet\s+oldest_allowed\s*=\s*now\.saturating_sub\(/u.test(contents) ||
        /right\.bound_at\s*>\s*left\.bound_at/u.test(contents) ||
        /existing_selected_at\s*>\s*incoming_selected_at/u.test(contents)) {
      violations.push(`${filePath}: contains restored Rust state merge/retention semantics`);
    }
    return violations;
  });
  const redactionViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === REDACTION_FILE) {
      const required = [
        "prodex_mojo_core::redaction::key_looks_sensitive(",
        "prodex_mojo_core::redaction::redact_secret_like_text(",
        "prodex_mojo_core::redaction::redact_gateway_text(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => `${filePath}: redaction migration must retain Mojo call ${call}`);
      const retired = /\bfn\s+(?:redaction_redact_email_tokens|redaction_redact_long_digit_tokens|redaction_redact_matching_tokens|redaction_redact_sensitive_key_value_text|redaction_process_field_name|redaction_sensitive_field_replacement|redaction_parse_potential_field_name|redaction_redacted_field_value|redaction_redact_authorization_like_values|redaction_try_authorization_value|redaction_redact_prefixed_api_key_tokens|redaction_secret_token_end)\s*\(/u;
      if (retired.test(contents)) {
        violations.push(`${filePath}: contains retired Rust redaction semantics`);
      }
      return violations;
    }
    if (filePath !== PRESIDIO_LOCAL_REDACTION_FILE) return [];
    const production = contents.split("#[cfg(test)]\nmod tests", 1)[0];
    const violations = [];
    if (!production.includes("prodex_mojo_core::redaction::local_inspect_and_redact(")) {
      violations.push(`${filePath}: local inspection must call the Mojo redaction kernel`);
    }
    const retiredLocal = /\bfn\s+(?:local_matches|detect_private_keys|detect_labeled_credentials|detect_bearer_tokens|detect_prefixed_api_keys|detect_emails|detect_financial_identifiers|sensitive_key_kind|delimited_value_range|skip_ascii_whitespace|secret_token_end|email_byte)\s*\(/u;
    if (retiredLocal.test(production)) {
      violations.push(`${filePath}: contains retired Rust local-redaction semantics`);
    }
    return violations;
  });
  const governanceInspectionViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== GOVERNANCE_INSPECTION_CONSUMER_FILE) return [];
    const required = [
      "governance_finding_minimum_classification(",
      "governance_findings_exceed_classification(",
      "governance_classification_label(",
      "governance_coverage_combine(",
      "governance_coverage_label(",
      "governance_content_location_path_valid(",
      "governance_inspection_token_valid(",
      "governance_inspection_limits_valid(",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => filePath + ": governance inspection must retain Mojo call " + call);
    if (
      contents.includes('Self::Public => "public"')
      || contents.includes('Self::Full => "full"')
      || contents.includes("byte.is_ascii_alphanumeric() || matches!(byte")
      || contents.includes("max_detectors == 0")
    ) {
      violations.push(filePath + ": contains restored Rust governance inspection semantics");
    }
    return violations;
  });
  const governanceInspectionOrderingViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === GOVERNANCE_INSPECTION_CONSUMER_FILE) {
      const required = [
        "prodex_mojo_core::policy::governance_inspection_order(",
        "apply_mojo_order(findings, &order.finding_indices)",
        "apply_mojo_order(tags, &order.tag_indices)",
        "apply_mojo_order(reason_codes, &order.reason_code_indices)",
      ];
      const violations = required
        .filter((value) => !contents.includes(value))
        .map((value) => `${filePath}: inspection ordering must use Mojo plan ${value}`);
      if (/\b(?:findings|tags|reason_codes)\.sort(?:_by)?\s*\(|\.dedup\s*\(/u.test(contents)) {
        violations.push(`${filePath}: contains restored Rust inspection ordering or deduplication`);
      }
      return violations;
    }
    if (filePath === GOVERNANCE_INSPECTION_ADAPTER_FILE &&
        (!contents.includes("fn prodex_mojo_governance_inspection_order_v1(") ||
          !contents.includes("pub fn governance_inspection_order(") ||
          !contents.includes("validate_governance_order("))) {
      return [`${filePath}: inspection ordering adapter must use the bounded Mojo ABI and validate returned indices`];
    }
    if (filePath === GOVERNANCE_INSPECTION_MOJO_FILE &&
        (!contents.includes('@export("prodex_mojo_governance_inspection_order_v1")') ||
          !contents.includes("governance_finding_order_sort(") ||
          !contents.includes("governance_view_order_sort(") ||
          !contents.includes("governance_view_order_deduplicate("))) {
      return [`${filePath}: finding order and metadata deduplication must stay in the governance Mojo kernel`];
    }
    if (filePath === GOVERNANCE_INSPECTION_TEST_FILE &&
        !contents.includes("inspection_result_ordering_uses_mojo_key_and_deduplicates")) {
      return [`${filePath}: production governance ordering needs a caller-boundary regression test`];
    }
    return [];
  });
  const profileIdentityViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-domain/src/secrets.rs") {
      const body = contents.match(/\bpub fn is_well_formed\(&self\) -> bool \{[^]*?^    \}/mu)?.[0];
      const violations = body?.includes(
        "prodex_mojo_core::secret_policy::secret_reference_is_well_formed(",
      ) ? [] : [filePath + ": SecretRef::is_well_formed must retain Mojo validation"];
      if (/\bfn\s+secret_ref_part_is_well_formed\s*\(/u.test(contents)) {
        violations.push(filePath + ": contains restored Rust secret-reference validation");
      }
      const rotation = contents.match(
        /\bpub fn validate\(&self\) -> Result<\(\), SecretRotationPolicyError> \{[^]*?^    \}/mu,
      )?.[0];
      if (!rotation?.includes(
        "prodex_mojo_core::secret_policy::secret_rotation_policy_decision(",
      )) {
        violations.push(filePath + ": SecretRotationPolicy::validate must retain Mojo policy");
      }
      if (/\bself\.max_age_seconds\s*==\s*0|\bself\.overlap_seconds\s*>=\s*self\.max_age_seconds/u.test(rotation ?? "")) {
        violations.push(filePath + ": contains restored Rust secret rotation bounds policy");
      }
      return violations;
    }
    if (filePath !== PROFILE_IDENTITY_FILE) return [];
    const functions = [
      ["find_matching_profile_identity", "mojo_profile_identity::find_matching_profile_identity("],
      ["unique_profile_name_from_base", "mojo_profile_identity::profile_name_candidate("],
      ["normalize_email", "mojo_profile_identity::normalize_email("],
      ["normalize_account_id", "mojo_profile_identity::normalize_account_id("],
      ["canonical_profile_identity_key", "mojo_profile_identity::canonical_profile_identity_key("],
      ["profile_name_from_email", "mojo_profile_identity::profile_name_from_email("],
      ["profile_name_looks_email_derived_for_other_email", "mojo_profile_identity::profile_name_looks_email_derived_for_other_email("],
      ["validate_profile_name", "mojo_profile_identity::validate_profile_name("],
      ["resolve_add_profile_source_kind", "mojo_profile_identity::add_profile_source_plan("],
      ["should_activate_profile", "mojo_profile_identity::should_activate_profile("],
      ["resolve_remove_profile_targets", "mojo_profile_identity::remove_profile_targets_plan("],
      ["should_delete_profile_home", "mojo_profile_identity::profile_home_delete_plan("],
    ];
    const violations = [];
    for (const [name, requiredCall] of functions) {
      const expression = new RegExp("\\bpub fn " + name + "(?:<[^>]+>)?\\([^]*?^\\}", "mu");
      const body = contents.match(expression)?.[0];
      if (!body?.includes(requiredCall)) {
        violations.push(filePath + ": " + name + " must retain Mojo profile-identity planning");
      }
    }
    if (/\bfn\s+(?:profile_identity_email_matches_target|strip_unique_profile_suffix|profile_name_base_looks_email_derived)\s*\(/u.test(contents)) {
      violations.push(filePath + ": contains a retired Rust profile-identity semantic helper");
    }
    return violations;
  });
  const superProviderConfigViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === SUPER_PROVIDER_CONFIG_FILE) {
      const required = [
        "prodex_mojo_core::super_provider_config::external_provider_alias(",
        "prodex_mojo_core::super_provider_config::provider_config_entries(",
        "prodex_mojo_core::super_provider_config::toml_string_literal(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": Super provider config migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const retired of [
        "match value.trim().to_ascii_lowercase().as_str()",
        "let overrides = [",
        "value.replace('\\',",
      ]) {
        if (production.includes(retired)) {
          violations.push(filePath + ": contains restored Rust Super provider config/alias/TOML semantics");
          break;
        }
      }
      return violations;
    }
    if (filePath === SUPER_PROVIDER_CONFIG_ADAPTER_FILE) {
      const required = [
        "prodex_super_external_provider_alias_v1(",
        "prodex_super_toml_string_v1(",
        "prodex_super_provider_config_v1(",
        "prodex_external_catalog_model_count_v1(",
        "prodex_external_catalog_model_at_v1(",
        "prodex_external_catalog_model_find_exact_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": Super provider config ABI adapter must retain " + call);
    }
    return [];
  });
  const externalProviderCatalogViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === EXTERNAL_PROVIDER_CONFIG_FILE) {
      const required = [
        "provider_catalog_entries_for(",
        "provider_catalog_entry(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": external-provider catalog must reuse canonical provider-core source " + call);
      for (const retired of [
        "external_catalog_static_models(",
        "external_catalog_model_metadata(",
        "ExternalCatalogProviderClass",
      ]) {
        if (contents.includes(retired)) {
          violations.push(filePath + ": production external-provider catalog must not use the duplicate Mojo static table");
          break;
        }
      }
      return violations;
    }
    if (filePath === "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs") {
      return contents.includes("let static_models = provider.models();")
        ? [] : [filePath + ": external-provider catalog builder must consume canonical static models"];
    }
    return [];
  });
  const subAgentPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === SUB_AGENT_CLI_VALIDATION_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const required = "prodex_mojo_core::sub_agent_policy::provider_url_violation(";
      const violations = production.includes(required)
        ? [] : [filePath + ": sub-agent provider URL validation must call Mojo provider URL policy"];
      const retired = [
        "args.sub_agent_url.is_some() && provider != ProviderId::Local",
        "args.sub_agent && provider == ProviderId::Local && args.sub_agent_url.is_none()",
      ];
      if (retired.some((predicate) => production.includes(predicate))) {
        violations.push(filePath + ": contains restored Rust sub-agent provider URL predicates");
      }
      return violations;
    }
    if (filePath === SUB_AGENT_POLICY_FILE) {
      const required = [
        "prodex_mojo_core::sub_agent_policy::parse_concurrency(",
        "prodex_mojo_core::sub_agent_policy::concurrency_valid(",
        "prodex_mojo_core::sub_agent_policy::reasoning_effort(",
        "prodex_mojo_core::sub_agent_policy::model_nonempty(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": sub-agent CLI policy must retain Mojo call " + call);
      const retired = [
        "if value == 0 || value > HARD_MAX_SUB_AGENT_CONCURRENCY",
        "value.bytes().all(|byte| byte.is_ascii_digit())",
        "SUB_AGENT_MAX_CONCURRENCY_PRESETS.contains(&parsed)",
        "match value.trim().to_ascii_lowercase().as_str()",
        "(!value.trim().is_empty())",
      ];
      if (retired.some((pattern) => contents.includes(pattern))) {
        violations.push(filePath + ": contains restored Rust sub-agent parsing/validation semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-app/src/runtime_tools/sub_agent_catalog.rs") {
      const violations = contents.includes(".filter_map(|effort| effort.label()?.parse::<SubAgentReasoningEffort>().ok())")
        ? []
        : [filePath + ": sub-agent effort choices must reuse Mojo-owned provider effort labels"];
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (production.includes("ProviderReasoningEffort::None =>")) {
        violations.push(filePath + ": contains restored Rust provider-to-sub-agent effort mapping");
      }
      return violations;
    }
    if (filePath === SUB_AGENT_POLICY_ADAPTER_FILE) {
      const required = [
        "prodex_sub_agent_policy_v1(",
        "prodex_sub_agent_child_argv_plan_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": sub-agent policy adapter must retain Mojo ABI call " + call);
    }
    if (filePath === SUB_AGENT_RENDER_ADAPTER_FILE) {
      const required = [
        "const RENDER_ABI_VERSION: i64 = 1;",
        "prodex_sub_agent_render_v1(",
        "fn sub_agent_render_v1_has_exact_output_and_checked_edges()",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => filePath + ": sub-agent render adapter must retain versioned Mojo ABI contract " + marker);
    }
    if (filePath === SUB_AGENT_RENDERING_FILE) {
      const production = contents.split("#[cfg(test)]\nmod tests", 1)[0];
      const required = [
        "prodex_mojo_core::sub_agent_policy::render_overlay(",
        "prodex_mojo_core::sub_agent_policy::render_enabled_dry_run_report(",
        "prodex_mojo_core::sub_agent_policy::render_disabled_dry_run_report(",
      ];
      const violations = required
        .filter((call) => !production.includes(call))
        .map((call) => filePath + ": sub-agent text output must use Mojo renderer " + call);
      for (const retired of [
        "SUB_AGENT_RULES:",
        "Act as lead and sole integrator:",
        "Each delegated task must request a concise structured result:",
        "Sub-agent concurrency enforcement: cross-process exclusive slot leases",
        "fn markdown_safe_value(",
        "fn shell_quote(",
        "fn render_posix_launcher_command(",
        "fn render_powershell_launcher_command(",
      ]) {
        if (production.includes(retired)) {
          violations.push(filePath + ": contains a replaced Rust sub-agent text template");
          break;
        }
      }
      return violations;
    }
    if (filePath === SUB_AGENT_CHILD_FILE) {
      const violations = [];
      if (!contents.includes("child_argv_plan(")) {
        violations.push(filePath + ": sub-agent child argv construction must retain Mojo planner");
      }
      if (!contents.includes("provider_model_reasoning_resolution(")) {
        violations.push(filePath + ": sub-agent reasoning compatibility must retain canonical Mojo-backed catalog resolver");
      }
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const retired of [
        "let mut args = vec![OsString::from(\"s\"), OsString::from(\"--no-sub-agent\")];",
        "args.push(OsString::from(if spec.presidio_enabled {",
        "match spec.provider {",
        "SubAgentReasoningEffort::None => ProviderReasoningEffort::None",
        "supported.contains(&effort)",
      ]) {
        if (production.includes(retired)) {
          violations.push(filePath + ": contains restored Rust sub-agent child argv planning semantics");
          break;
        }
      }
      return violations;
    }
    return [];
  });
  const runtimeOverlayPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === RUNTIME_OVERLAY_POLICY_FILE) {
      const required = [
        "mojo_overlay_config_assignments(",
        "mojo_workspace_trust_indices(",
        "mojo_overlay_transport_flags(",
        "mojo_fresh_projection(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime-overlay migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const retired of [
        'if matches!(arg, "-c" | "--config")',
        'arg == "--remote" || arg.starts_with("--remote=")',
        "match argument.as_ref()",
        'value.trim_start().starts_with("projects=")',
      ]) {
        if (production.includes(retired)) {
          violations.push(filePath + ": contains restored Rust runtime-overlay argv scanning semantics");
          break;
        }
      }
      return violations;
    }
    if (filePath === RUNTIME_OVERLAY_POLICY_ADAPTER_FILE) {
      const required = [
        "prodex_runtime_overlay_config_assignments_v1(",
        "prodex_runtime_overlay_workspace_trust_indices_v1(",
        "prodex_runtime_overlay_transport_flags_v1(",
        "prodex_runtime_overlay_fresh_projection_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime-overlay policy adapter must retain Mojo ABI call " + call);
    }
    return [];
  });
  const cliRuntimeFeatureViolations = files
    .filter(([filePath, contents]) => filePath === CLI_RUNTIME_FEATURE_FILE &&
      /\bfn\s+(?:rust_plan|rollout_budget_reminders|to_codex_config_args_rust|mojo_feature_plan_matches_rust_oracle_for_seeded_inputs)\s*\(/u.test(contents))
    .map(([filePath]) => `${filePath}: contains a Rust runtime-feature planner or oracle`);
  const superExposeProtocolViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === SUPER_EXPOSE_PROTOCOL_FILE) {
      const required = [
        "prodex_mojo_core::rich::super_expose_protocol_version_supported(",
        "SuperExposeMethod as ExposeMethod",
        "SuperExposeTool as ExposeTool",
        "super_expose_route(method, tool)",
      ];
      const violations = required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => filePath + ": Super-expose protocol must retain Mojo-owned surface " + marker);
      if (contents.includes("MCP_PROTOCOL_VERSIONS.contains(&version)")) {
        violations.push(filePath + ": contains restored Rust MCP protocol-version membership semantics");
      }
      if (
        /enum\s+ExposeMethod/u.test(contents)
        || /enum\s+ExposeTool/u.test(contents)
        || /impl\s+ExposeMethod/u.test(contents)
        || /impl\s+ExposeTool/u.test(contents)
      ) {
        violations.push(filePath + ": contains restored Rust Super-expose route or label mirror");
      }
      return violations;
    }
    if (filePath === SUPER_EXPOSE_DISPATCH_FILE) {
      const required = "prodex_mojo_core::rich::super_expose_dispatch_validation(";
      const violations = contents.includes(required)
        ? [] : [filePath + ": Super-expose dispatch validation must retain Mojo call " + required];
      for (const retired of [
        "fn validate_request_id_presence(",
        "fn validate_method_params(",
        "fn validate_initialize_params(",
        "fn validate_tools_call_params(",
      ]) {
        if (contents.includes(retired)) {
          violations.push(filePath + ": contains restored Rust Super-expose dispatch validation semantics");
          break;
        }
      }
      return violations;
    }
    if (filePath === SUPER_EXPOSE_VALIDATION_FILE) {
      const required = [
        "prodex_mojo_core::rich::super_expose_protocol_metadata(",
        "prodex_mojo_core::rich::super_expose_content_type_allowed(",
        "prodex_mojo_core::rich::super_expose_accept_allowed(",
        "prodex_mojo_core::rich::super_expose_json_nesting_within_limit(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": Super-expose validation must retain Mojo call " + call);
      for (const retired of [
        "fn validate_protocol_version(",
        "fn validate_current_protocol_metadata(",
        "fn validate_legacy_method_header(",
        ".split(';')",
        ".split(',')",
        "let mut depth = 0usize",
      ]) {
        if (contents.includes(retired)) {
          violations.push(filePath + ": contains restored Rust Super-expose protocol/media/nesting semantics");
          break;
        }
      }
      return violations;
    }
    if (filePath === SUPER_EXPOSE_TOOL_CONTRACT_FILE) {
      const required = [
        "prodex_mojo_core::rich::super_expose_tool_argument_allowed(",
        "prodex_mojo_core::rich::super_expose_run_id_valid(",
        "prodex_mojo_core::rich::super_expose_string_valid(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": Super-expose tool contract must retain Mojo call " + call);
      for (const retired of [
        "let allowed = match tool",
        "value.starts_with(\"spr_\")",
        "value.chars().any(char::is_control)",
      ]) {
        if (contents.includes(retired)) {
          violations.push(filePath + ": contains restored Rust Super-expose tool/string semantics");
          break;
        }
      }
      return violations;
    }
    if (filePath === SUPER_EXPOSE_RICH_FILE) {
      const required = [
        "prodex_mojo_super_expose_label_v1(",
        "prodex_mojo_super_expose_dispatch_validation_v1(",
        "prodex_mojo_super_expose_protocol_version_supported_v1(",
        "prodex_mojo_super_expose_protocol_metadata_v1(",
        "prodex_mojo_super_expose_media_header_v1(",
        "prodex_mojo_super_expose_json_nesting_v1(",
        "prodex_mojo_super_expose_tool_argument_allowed_v1(",
        "prodex_mojo_super_expose_run_id_valid_v1(",
        "prodex_mojo_super_expose_string_valid_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": Super-expose ABI adapter must retain " + call);
    }
    return [];
  });
  const superExposeViolations = files
    .filter(([filePath, contents]) => filePath === SUPER_EXPOSE_FILE &&
      (!contents.includes("prodex_mojo_core::launch::find_super_expose_alias_index") ||
        /\bfn\s+(?:rewrite_super_expose_alias|super_option_takes_value)\s*\(/u.test(contents)))
    .map(([filePath]) => `${filePath}: Super expose alias scan must use Mojo`);
  const geminiFallbackViolations = files
    .filter(([filePath, contents]) =>
      filePath === "crates/prodex-provider-core/src/fallback/chains/gemini.rs" &&
      /\bfn\s+provider_gemini_model_fallback_alias_chain\s*\(/u.test(contents))
    .map(([filePath]) => `${filePath}: contains a Rust Gemini model fallback table`);
  const geminiGenerationViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === GEMINI_BRIDGE_REQUEST_FILE) {
      const operations = [
        ["gemini_provider_core_generation_config_from_request", "gemini_bridge_request_generation_config"],
        ["gemini_provider_core_generate_content_request_map", "gemini_bridge_request_map"],
        ["gemini_provider_core_generate_content_body_value", "gemini_bridge_request_body"],
      ];
      return operations.flatMap(([name, mojoCall]) => {
        const body = contents.match(new RegExp(`\\bpub fn ${name}\\([^]*?^\\}`, "mu"))?.[0];
        return body?.includes(`request_contents::${mojoCall}(`) && !FEATURE_OFF_RUST_PATH.test(body)
          ? [] : [`${filePath}: ${name} must use Mojo in every feature mode`];
      });
    }
    if (filePath === GEMINI_GENERATION_CONFIG_FILE && /\bfn\s+gemini_generation_config_from_request\s*\(/u.test(contents)) {
      return [`${filePath}: contains a duplicate Gemini generation-config adapter`];
    }
    if (filePath === GEMINI_THINKING_FILE && /\bfn\s+gemini_thinking_config\s*\(/u.test(contents)) {
      return [`${filePath}: contains replaced Rust thinking-config semantics`];
    }
    return [];
  });
  const geminiTranslatorHardReplacementViolations = files.flatMap(([filePath, contents]) => {
    if (/\bgemini_translator_validation_error\b/u.test(contents)) {
      return [`${filePath}: gemini translator validation errors must use the Mojo-produced reason`];
    }
    if (filePath === GEMINI_REQUEST_TRANSFORM_FILE) {
      const required = [
        "gemini_bridge_validate_translator(",
        "gemini_bridge_raw_translator_request(",
        "gemini_request_contents_from_request_mojo(",
        "provider_core_chat_tools_from_responses_request(",
      ];
      const violations = required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Gemini translator hard replacement must retain ${marker}`);
      if (/fn\s+(?:gemini_validate_request_rust|gemini_build_body_rust|gemini_contains_local_media_path|gemini_apply_tools)\s*\(/u.test(contents)) {
        violations.push(`${filePath}: contains restored Rust Gemini translator semantics`);
      }
      return violations;
    }
    if (filePath === GEMINI_REQUEST_CONTENTS_FILE &&
        !contents.includes("GeminiBridgeRequestOperation::TextContents")) {
      return [`${filePath}: Gemini text contents must use Mojo`];
    }
    if (filePath === "crates/prodex-provider-core/src/translators/gemini/stream/shaping.rs") {
      const violations = [];
      if (/#\[cfg\(feature = "mojo"\)\]/u.test(contents)) {
        violations.push(filePath + ": Gemini stream shaping Mojo kernel must be unconditional");
      }
      const completedItem = contents.match(
        /\bpub fn gemini_provider_core_stream_completed_tool_call_item\([^]*?^\}/mu,
      )?.[0] ?? "";
      if (!completedItem.includes("GeminiResponseKernelOperation::StreamCompletedToolCallItem")) {
        violations.push(filePath + ": completed stream tool-call selection must use the Gemini Mojo kernel");
      }
      if (/GeminiResponseKernelOperation::(?:RawFunctionCallItem|FunctionCallItem|ToolSearchCallItem|CustomToolCallItem|OutputMessageItem)\b/u.test(completedItem)) {
        violations.push(filePath + ": contains replaced Rust completed tool-call shaping branches");
      }
      const deltaSignature = contents.match(
        /\bpub fn gemini_provider_core_function_call_arguments_delta_event_with_thought_signature\([^]*?^\}/mu,
      )?.[0] ?? "";
      if (!deltaSignature.includes("GeminiResponseKernelOperation::FunctionCallArgumentsDeltaWithThoughtSignature")) {
        violations.push(filePath + ": function-call delta thought-signature shaping must use the Gemini Mojo kernel");
      }
      if (/event\.get\(|object\.keys\(\)\.all|event\["thought_signature"\]/u.test(deltaSignature)) {
        violations.push(filePath + ": contains restored Rust function-call delta thought-signature shaping");
      }
      const functionDelta = contents.match(
        /\bpub fn gemini_provider_core_stream_function_call_delta\([^]*?^\}/mu,
      )?.[0] ?? "";
      if (!functionDelta.includes("GeminiResponseKernelOperation::StreamFunctionCallDelta") || !functionDelta.includes("input.response = Some(&raw)")) {
        violations.push(filePath + ": stream function-call extraction must stay Mojo-owned");
      }
      if (/\.get\("name"\)|\.get\("args"\)/u.test(functionDelta)) {
        violations.push(filePath + ": contains restored Rust stream function-call extraction");
      }
      const responseId = contents.match(
        /\bpub fn gemini_provider_core_stream_response_id_from_chunk\([^]*?^\}/mu,
      )?.[0] ?? "";
      if (!responseId.includes("GeminiResponseKernelOperation::StreamResponseId") || !responseId.includes("input.response = Some(&raw)")) {
        violations.push(filePath + ": stream response-id extraction must stay Mojo-owned");
      }
      if (/\.get\("responseId"\)|\.get\("id"\)/u.test(responseId)) {
        violations.push(filePath + ": contains restored Rust stream response-id extraction");
      }
      for (const [symbol, operation, forbidden] of [
        ["stream_chunk_metadata", "StreamChunkMetadata", /\.get\(|gemini_responses_usage|gemini_response_metadata|gemini_finish_reason/u],
        ["stream_tool_call_ids", "StreamToolCallIds", /\.filter\(/u],
        ["stream_tool_call_added_item", "StreamAddedToolCallItem", /name\s*==|matches!|AddedFunctionCallItem/u],
      ]) {
        const body = contents.match(new RegExp(`\\bpub fn gemini_provider_core_${symbol}\\([^]*?^\\}`, "mu"))?.[0] ?? "";
        if (!body.includes(`GeminiResponseKernelOperation::${operation}`) || forbidden.test(body)) {
          violations.push(`${filePath}: ${symbol} must stay Mojo-owned without Rust selection`);
        }
      }
      if (/\.get\("id"\)|\.trim\(\)/u.test(functionDelta)) {
        violations.push(filePath + ": explicit stream call-id selection must stay Mojo-owned");
      }
      return violations;
    }
    if (filePath === "crates/prodex-provider-core/src/translators/gemini/response/grounding.rs") {
      const required = [
        "Document::default()",
        "gemini_grounding(",
        "GeminiGroundingOperation::CitationText",
        "GeminiGroundingOperation::WebSearchCall",
      ];
      const violations = required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Gemini grounding adapter must retain ${marker}`);
      for (const retired of [
        "gemini_collect_grounding_chunk_sources",
        "gemini_collect_citation_sources",
        "gemini_collect_url_metadata_sources",
        "gemini_url_source_from_metadata",
        "gemini_push_unique_url_source",
      ]) {
        if (contents.includes(retired)) {
          violations.push(`${filePath}: contains restored Rust Gemini grounding semantics (${retired})`);
        }
      }
      if (/\.get\("(?:groundingMetadata|citationMetadata|urlContextMetadata|webSearchQueries|groundingChunks)"\)/u.test(contents)) {
        violations.push(`${filePath}: contains restored Rust Gemini grounding JSON selection`);
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/json.rs") {
      const required = [
        "prodex_mojo_gemini_grounding_v1(",
        "pub enum GeminiGroundingOperation",
        "pub fn gemini_grounding(",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Gemini grounding ABI adapter must retain ${marker}`);
    }
    if (filePath === "mojo/prodex_core/gemini_response.mojo") {
      const required = [
        "def gemini_put_grounding_citation_text(",
        "def gemini_put_grounding_web_search_call(",
        "def gemini_grounding_v1(",
        "gemini_grounding_source_duplicate(",
        "gemini_citation_line_less(",
      ];
      const violations = required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Mojo Gemini grounding owner must retain ${marker}`);
      for (const retired of [
        "GEMINI_CITATION_TEXT",
        "GEMINI_WEB_SEARCH_CALL",
        "def gemini_put_web_search_call(",
        "def gemini_put_citation_text(",
      ]) {
        if (contents.includes(retired)) {
          violations.push(`${filePath}: contains a duplicate legacy Gemini grounding semantic path (${retired})`);
        }
      }
      return violations;
    }
    if (filePath === "mojo/prodex_core/rich_abi.mojo") {
      const required = [
        '@export("prodex_mojo_gemini_grounding_v1")',
        "gemini_grounding_v1(",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Gemini grounding ABI export must retain ${marker}`);
    }
    if (filePath === GEMINI_BRIDGE_ROOT_FILE &&
        /#\[cfg\(feature = "mojo"\)\]\s*pub\(crate\) use self::request::\{/u.test(contents)) {
      return [`${filePath}: Gemini translator bridge export must be unconditional`];
    }
    if (filePath === GEMINI_BRIDGE_REQUEST_CONTENTS_FILE) {
      if (/#\[cfg\(feature = "mojo"\)\]\s*(?:#\[[^\]]+\]\s*)?(?:pub\(crate\) struct GeminiTranslatorValidationPlan|pub\(crate\) fn gemini_bridge_(?:validate_translator|raw_translator_request))/u.test(contents)) {
        return [`${filePath}: Gemini translator bridge helpers must be unconditional`];
      }
      if (!contents.includes('get("reason")')) {
        return [`${filePath}: Gemini translator validation must retain the Mojo-produced reason`];
      }
    }
    return [];
  });
  const geminiBridgeFallbackViolations = files.flatMap(([filePath, contents]) => {
    const requiredByFile = new Map([
      [GEMINI_BRIDGE_SIMPLE_FILE, "gemini_bridge_request_simple("],
      [GEMINI_BRIDGE_NATIVE_PROJECT_FILE, "gemini_bridge_request_native_project("],
      [GEMINI_BRIDGE_TOOLS_FILE, "gemini_bridge_request_without_tool("],
      [GEMINI_BRIDGE_REQUEST_FILE, "gemini_bridge_request_candidate_count("],
    ]);
    const required = requiredByFile.get(filePath);
    if (required && !contents.includes(required)) {
      return [`${filePath}: Gemini bridge hard replacement must retain ${required}`];
    }
    if (filePath === GEMINI_BRIDGE_REQUEST_CONTENTS_FILE) {
      const operations = [
        "GeminiBridgeRequestOperation::NativeProject",
        "GeminiBridgeRequestOperation::RequestBodyWithoutTool",
        "GeminiBridgeRequestOperation::SimpleRequest",
        "GeminiBridgeRequestOperation::ValidateCandidateCount",
      ];
      return operations
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Gemini bridge kernel adapter must retain ${marker}`);
    }
    if ((filePath === GEMINI_TRANSLATOR_REQUEST_FILE ||
         filePath === GEMINI_TRANSLATOR_GENERATION_CONFIG_FILE) &&
        /(?:gemini_validate_candidate_count|gemini_request_body_without_tool)/u.test(contents)) {
      return [`${filePath}: contains a deleted Gemini feature-off fallback adapter`];
    }
    return [];
  });
  const geminiToolResponseOrderViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === GEMINI_TOOL_RESPONSE_ORDER_FILE) {
      const body = contents.match(
        /\bfn\s+gemini_provider_core_refine_tool_response_order\([^]*?^\}/mu,
      )?.[0] ?? "";
      const violations = [];
      if (!body.includes("gemini_tool_response_part_order(")) {
        violations.push(`${filePath}: Gemini tool-response ordering must use Mojo`);
      }
      if (/\.sort(?:_by_key|_unstable_by_key|_unstable)?\s*\(|\.position\s*\(/u.test(body)) {
        violations.push(`${filePath}: contains restored Rust Gemini tool-response ordering policy`);
      }
      if (FEATURE_OFF_RUST_PATH.test(body)) {
        violations.push(`${filePath}: Gemini tool-response ordering has a feature-off Rust path`);
      }
      return violations;
    }
    if (filePath === GEMINI_TOOL_RESPONSE_ORDER_ADAPTER_FILE &&
        !contents.includes("prodex_provider_constraints_gemini_tool_response_order_v1(")) {
      return [`${filePath}: Gemini tool-response adapter must retain the Mojo ABI call`];
    }
    if (filePath === GEMINI_TOOL_RESPONSE_ORDER_MOJO_FILE &&
        !contents.includes("prodex_provider_constraints_gemini_tool_response_order_v1(")) {
      return [`${filePath}: Gemini tool-response ordering must be implemented in Mojo`];
    }
    return [];
  });
  const hardReplacementViolations = files
    .filter(([filePath, contents]) => HARD_REPLACED_RUST_FILES.has(filePath) &&
      /\b(?:rust_oracle|fn\s+[A-Za-z0-9_]+_rust\s*\()/u.test(contents))
    .map(([filePath]) => `${filePath}: contains a Rust semantic oracle or copy`);
  const precommitBudgetOracleViolations = files
    .filter(([filePath, contents]) => filePath === PRECOMMIT_BUDGET_TEST_FILE &&
      /\bfn\s+precommit_budget_matches_rust_oracle\s*\(/u.test(contents))
    .map(([filePath]) => `${filePath}: contains a Rust pre-commit budget oracle`);
  const deepseekRequestViolations = files
    .filter(([filePath, contents]) => filePath === DEEPSEEK_REQUEST_FILE &&
      !contents.includes("DeepSeekKernelOperation::RawCommonRequest"))
    .map(([filePath]) => `${filePath}: DeepSeek request body must use the Mojo raw kernel`);
  const deepseekRequestRejectViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== DEEPSEEK_REQUEST_REJECT_FILE) return [];
    const operations = [
      ["deepseek_provider_core_reject_unsupported_request_fields", "RequestFields"],
      ["deepseek_provider_core_reject_beta_completion_fields", "BetaFields"],
    ];
    const missingMojoCalls = operations.some(([name, operation]) => {
      const body = contents.match(new RegExp(`\\bpub fn ${name}\\([^]*?^\\}`, "mu"))?.[0];
      return !body?.includes("request_policy::try_plan_value(") ||
        !body.includes(`DeepSeekRequestPolicyOperation::${operation}`);
    });
    const rustPolicy = /\b(?:rust_compat|deepseek_provider_core_reject_(?:deprecated_fields|unsupported_fields|max_tool_calls)|deepseek_provider_core_validate_(?:optional_request_values|background|truncation|text|parallel_tool_calls|stream_options|modalities)|reject_(?:unsupported_request_fields|beta_completion_fields)_rust)\b/u.test(contents);
    if (missingMojoCalls || rustPolicy || /#\[\s*cfg\s*\(/u.test(contents)) {
      return [`${filePath}: request rejection must use both Mojo policies without Rust copies or cfg routing`];
    }
    return [];
  });
  const deepseekReasoningViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== DEEPSEEK_REASONING_FILE) return [];
    const mojoOwned = contents.includes("request_policy::try_plan_value(") &&
      contents.includes("DeepSeekRequestPolicyOperation::ReasoningShape") &&
      contents.includes("DeepSeekKernelOperation::ReasoningParameters");
    const oldMapper = /\b(?:rust_compat|deepseek_provider_core_reasoning_effort(?:_from_responses_request)?|deepseek_provider_core_gemini_openai_reasoning_effort)\b/u.test(contents);
    return mojoOwned && !oldMapper && !/#\[\s*cfg\s*\(/u.test(contents)
      ? [] : [`${filePath}: reasoning must use Mojo without Rust copies or cfg routing`];
  });
  const nativeFirstErrorClassViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== NATIVE_FIRST_ERROR_CLASS_FILE) return [];
    const violations = contents.includes("classify_provider_error(")
      ? []
      : [filePath + ": native first-event error classification must retain canonical Mojo-backed provider classifier"];
    const production = contents.split("#[cfg(test)]", 1)[0];
    if (
      production.includes("match code.as_deref()")
      || production.includes('"rate_limit_error" | "rate_limit_exceeded"')
      || production.includes('"overloaded_error" | "server_is_overloaded"')
      || production.includes('"not_found_error" | "model_not_supported"')
    ) {
      violations.push(filePath + ": contains restored Rust native SSE error-code classification table");
    }
    return violations;
  });
  const providerBridgeMetadataViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === PROVIDER_BRIDGE_METADATA_FILE) {
      const required = [
        "provider_bridge_rate_limit_header_prefix(",
        "provider_bridge_rate_limit_header_label(",
        "provider_bridge_chat_compatible_adapter_label(",
        "provider_bridge_function_tool_name_max_bytes(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": provider bridge metadata migration must retain Mojo call " + call);
      if (!/#\[repr\(i64\)\][^]*?enum RuntimeProviderBridgeKind/u.test(contents)) {
        violations.push(filePath + ": RuntimeProviderBridgeKind must remain repr(i64) for Mojo ABI tags");
      }
      for (const name of [
        "rate_limit_header_prefix",
        "rate_limit_header_label",
        "chat_compatible_adapter_label",
        "function_tool_name_max_bytes",
      ]) {
        const body = contents.match(new RegExp("\\bfn\\s+" + name + "\\(self\\)[^]*?^\\s*\\}", "mu"))?.[0];
        if (body?.includes("match self")) {
          violations.push(filePath + ": contains restored Rust provider bridge metadata table in " + name);
        }
      }
      return violations;
    }
    if (filePath === PROVIDER_SURFACE_FILE) {
      const required = [
        "provider_constraints::provider_id_label(",
        "provider_constraints::provider_wire_format_label(",
        "provider_constraints::provider_endpoint_label(",
        "provider_constraints::provider_capability_status_label(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": provider surface labels must retain Mojo call " + call);
      for (const name of ["ProviderId", "ProviderWireFormat", "ProviderEndpoint", "ProviderCapabilityStatus"]) {
        if (!new RegExp("#\\[repr\\(i64\\)\\](?:\\s*#\\[[^\\n]+\\])*\\s*pub\\s+enum\\s+" + name, "u").test(contents)) {
          violations.push(filePath + ": " + name + " must remain repr(i64) for Mojo surface label ABI");
        }
      }
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (/pub (?:const )?fn label\(self\)[^]*?match self/u.test(production)) {
        violations.push(filePath + ": contains restored Rust provider surface label table");
      }
      return violations;
    }
    if (filePath === PROVIDER_CONSTRAINTS_ADAPTER_FILE) {
      const required = [
        "prodex_provider_bridge_label_v1(",
        "prodex_provider_bridge_function_tool_name_max_bytes_v1(",
        "prodex_provider_bridge_native_passthrough_v1(",
        "prodex_provider_reasoning_effort_label_v1(",
        "prodex_provider_surface_label_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": provider bridge metadata adapter must retain Mojo ABI " + call);
    }
    if (filePath === PROVIDER_BRIDGE_ROUTING_FILE) {
      const body = contents.match(/\bfn\s+runtime_provider_native_passthrough\([^]*?^\}/mu)?.[0];
      const violations = body?.includes("provider_bridge_native_passthrough(")
        ? []
        : [filePath + ": provider native-passthrough policy must retain Mojo decision"];
      if (
        body?.includes("RuntimeProviderBridgeKind::OpenAiResponses")
        || body?.includes("ProviderCapabilityStatus::Native")
        || body?.includes("ProviderCapabilityStatus::Passthrough")
        || body?.includes("ResponsesCompact")
      ) {
        violations.push(filePath + ": contains restored Rust provider native-passthrough decision matrix");
      }
      return violations;
    }
    if (filePath === PROVIDER_PRECOMMIT_FILE) {
      const violations = contents.includes("provider as i64")
        ? []
        : [filePath + ": provider precommit must use stable RuntimeProviderBridgeKind ABI tag"];
      if (contents.includes("fn runtime_provider_bridge_kind_tag(")) {
        violations.push(filePath + ": contains restored Rust provider bridge tag mapper");
      }
      return violations;
    }
    return [];
  });
  const websocketProxyPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === WEBSOCKET_PROXY_POLICY_FILE) {
      const required = [
        "mojo_websocket_proxy::default_port(",
        "mojo_websocket_proxy::proxy_url_candidate(",
        "mojo_websocket_proxy::value_matches(",
        "mojo_websocket_proxy::pattern_matches(",
        "mojo_websocket_proxy::pattern_host_port(",
        "mojo_websocket_proxy::normalize_host(",
        "mojo_websocket_proxy::authority(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": websocket proxy migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const retired of [
        '.trim_matches(|ch| ch == \'[\' || ch == \']\')',
        "pattern.matches(':').count()",
        'format!("http://{trimmed}")',
        'format!("[{host}]:{port}")',
        ".split(',')",
        "ascii_casefold_ends_with(",
        "ascii_casefold_equal_exact(",
      ]) {
        if (production.includes(retired)) {
          violations.push(filePath + ": contains restored Rust websocket proxy string/NO_PROXY policy");
          break;
        }
      }
      return violations;
    }
    if (filePath === WEBSOCKET_PROXY_POLICY_ADAPTER_FILE) {
      const required = [
        "prodex_websocket_proxy_default_port_v1(",
        "prodex_websocket_proxy_url_candidate_v1(",
        "prodex_websocket_proxy_pattern_plan_v1(",
        "prodex_websocket_proxy_pattern_matches_v1(",
        "prodex_websocket_proxy_value_matches_v1(",
        "prodex_websocket_proxy_normalize_host_v1(",
        "prodex_websocket_proxy_authority_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": websocket proxy adapter must retain Mojo ABI " + call);
    }
    return [];
  });
  const transportFailurePolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === TRANSPORT_FAILURE_POLICY_FILE) {
      const required = [
        "mojo_transport_failure::failure_kind_label(",
        "mojo_transport_failure::upstream_connect_failure_marker(",
        "mojo_transport_failure::classify_message(",
        "mojo_transport_failure::health_penalty(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": transport-failure migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const retired of [
        "RuntimeTransportFailureMessageRule",
        "RUNTIME_TRANSPORT_FAILURE_MESSAGE_RULES",
        "ascii_casefold_contains(",
        '"upstream_connect_timeout"',
        '"upstream_connect_dns_error"',
        '"upstream_tls_handshake_error"',
      ]) {
        if (production.includes(retired)) {
          violations.push(filePath + ": contains restored Rust transport-failure classifier/label policy");
          break;
        }
      }
      if (!/#\[repr\(i64\)\][^]*?enum\s+RuntimeTransportFailureKind/u.test(production)) {
        violations.push(filePath + ": RuntimeTransportFailureKind must remain repr(i64) for Mojo policy ABI");
      }
      return violations;
    }
    if (filePath === TRANSPORT_FAILURE_POLICY_ADAPTER_FILE) {
      const required = [
        "prodex_transport_failure_text_v1(",
        "prodex_transport_failure_classify_message_v1(",
        "prodex_transport_failure_health_penalty_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": transport-failure adapter must retain Mojo ABI " + call);
    }
    return [];
  });
  const providerPrecommitPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === PROVIDER_CONSTRAINTS_ADAPTER_FILE) {
      return contents.includes("prodex_provider_precommit_policy_v1(")
        ? []
        : [filePath + ": provider precommit adapter must retain Mojo ABI"];
    }
    if (filePath === PROVIDER_PRECOMMIT_FILE) {
      const required = [
        "provider_precommit_health_action(",
        "provider_precommit_metric_class(",
        "provider_precommit_buffered_fallback_class(",
        "provider_precommit_live_fallback_class(",
        "provider_precommit_should_prefetch(",
        "provider_precommit_sse_action(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": provider precommit migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const retired of [
        "fallback_class == Some(ProviderErrorClass::Transient)",
        "match (result, fallback_class)",
        "if event.quota_blocked {",
        "!live.native_anthropic_messages",
      ]) {
        if (production.includes(retired)) {
          violations.push(filePath + ": contains restored Rust provider precommit policy semantics");
          break;
        }
      }
      return violations;
    }
    if (filePath === LOCAL_REWRITE_UPSTREAM_FILE) {
      const body = contents.match(/\bfn\s+runtime_local_rewrite_openai_error_can_retry\([^]*?^\}/mu)?.[0];
      const violations = body?.includes("runtime_gateway_application_provider_retry_precommit(")
        ? []
        : [filePath + ": OpenAI credential retry must retain Mojo-backed provider retry policy"];
      if (
        body?.includes("attempt_index + 1 < attempt_count")
        || body?.includes("ProviderErrorClass::Auth")
        || body?.includes("ProviderErrorClass::Quota")
        || body?.includes("ProviderErrorClass::RateLimit")
        || body?.includes("ProviderErrorClass::Transient")
      ) {
        violations.push(filePath + ": contains restored Rust OpenAI retry eligibility matrix");
      }
      const nativeFirstBody = contents.match(/\bpub\(super\) fn\s+runtime_local_rewrite_precommit_native_first_event\([^]*?^\}/mu)?.[0];
      if (!nativeFirstBody?.includes("provider_precommit_native_first_should_prefetch(")) {
        violations.push(filePath + ": native-first SSE prefetch eligibility must retain Mojo provider policy");
      }
      if (
        nativeFirstBody?.includes("!live.native_anthropic_messages")
        || nativeFirstBody?.includes("(200..300).contains(&live.status)")
        || nativeFirstBody?.includes("!live.prefix.is_empty()")
      ) {
        violations.push(filePath + ": contains restored Rust native-first SSE prefetch eligibility policy");
      }
      const previousBody = contents.match(/\bpub\(super\) fn\s+runtime_local_rewrite_previous_response_id\([^]*?^\}/mu)?.[0];
      if (!previousBody?.includes("runtime_request_previous_response_id_from_bytes(")) {
        violations.push(filePath + ": local rewrite previous-response extraction must retain canonical Mojo-backed request metadata helper");
      }
      if (
        previousBody?.includes("serde_json::from_slice")
        || previousBody?.includes('.get("previous_response_id")')
        || previousBody?.includes(".map(str::trim)")
      ) {
        violations.push(filePath + ": contains restored Rust previous_response_id parsing semantics");
      }
      return violations;
    }
    if (filePath === PROVIDER_ERROR_FILE) {
      const enumDecl = contents.match(/#\[repr\(i64\)\][^]*?pub enum ProviderErrorClass\s*\{/u)?.[0];
      return enumDecl
        ? []
        : [filePath + ": ProviderErrorClass must remain ABI-stable for Mojo policy tags"];
    }
    return [];
  });
  const providerErrorMemberViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== PROVIDER_ERROR_FILE) return [];
    const body = contents.match(/\bpub fn provider_error_rejects_request_member\([^]*?^\}/mu)?.[0];
    const rustMatcher = /\bfn\s+(?:normalized|mentions_member|has_rejection_marker|value_mentions_member|value_has_rejection_marker|explicitly_rejects)\s*\(/u.test(contents);
    return body?.includes("prodex_mojo_core::json::provider_error_rejects_member(") &&
      !FEATURE_OFF_RUST_PATH.test(body) && !rustMatcher
      ? [] : [`${filePath}: request-member rejection must use Mojo without a Rust matcher`];
  });
  const deepseekResponseToolCallViolations = files
    .filter(([filePath, contents]) => filePath === DEEPSEEK_RESPONSE_TOOL_CALLS_FILE &&
      (!contents.includes("DeepSeekKernelOperation::ResponseToolCallItem") ||
        /\bfn\s+(?:deepseek_split_flat_namespace_tool_name|deepseek_chat_tool_call_thought_signature)\s*\(/u.test(contents)))
    .map(([filePath]) => `${filePath}: DeepSeek response tool-call shaping must use the Mojo kernel`);
  const chatToolViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === CHAT_TOOLS_BRIDGE_FILE &&
      (!contents.includes("pub use entry::*;") ||
        /\bmod\s+(?:tool_choice|tools|util|web_search)\s*;/u.test(contents))) {
      return [`${filePath}: chat-tool operations must use the Mojo entrypoints`];
    }
    if (filePath === CHAT_TOOLS_MOJO_FILE && !contents.includes("transform_chat_tools(")) {
      return [`${filePath}: chat-tool operations must invoke the Mojo kernel`];
    }
    return [];
  });
  const previousResponseOutcomeLabelViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== "crates/prodex-runtime-proxy/src/attempt_outcome.rs") return [];
    const required = [
      "runtime_previous_response_fallback_shape_label(value)",
      "runtime_previous_response_retry_reason_label(",
      "runtime_previous_response_chain_reason_label(",
      "runtime_previous_response_outcome_label(1)",
      "runtime_previous_response_outcome_label(0)",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => filePath + ": previous-response labels must retain Mojo call " + call);
    const forbidden = [
      '"tool_output_only"',
      '"empty_input"',
      '"session_replayable"',
      '"continuation_only"',
      '"non_blocking_retry"',
      '"locked_affinity_no_turn_state"',
      '"previous_response_not_found_locked_affinity"',
      '"blocked_nonreplayable_without_affinity"',
      '"blocked_without_affinity"',
    ];
    for (const value of forbidden) {
      if (contents.includes(value)) {
        violations.push(filePath + ": contains restored Rust previous-response label semantic " + value);
      }
    }
    return violations;
  });
  const affinityChainLogRenderViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-runtime-proxy/src/route_affinity_log.rs") {
      const required = [
        "prodex_mojo_core::log::render_route_affinity_log(",
        "prodex_mojo_core::log::render_route_affinity_owner_logs(",
      ];
      const missing = required.filter((call) => !contents.includes(call));
      if (missing.length > 0) {
        return missing.map((call) => filePath + ": route-affinity log rendering must use Mojo " + call);
      }
      const forbidden = [
        " route_affinity_recompute reason=",
        " route_affinity_recompute_result reason=",
        " compact_followup_owner profile=",
        " transport=http",
      ];
      return forbidden
        .filter((value) => contents.includes(value))
        .map((value) => filePath + ": contains restored Rust route-affinity log semantic " + value);
    }
    if (filePath === "crates/prodex-runtime-proxy/src/chain_log.rs") {
      if (!contents.includes("prodex_mojo_core::log::render_chain_log(")) {
        return [filePath + ": chain log rendering must use Mojo"];
      }
      const forbidden = [
        " chain_retried_owner profile=",
        " chain_dead_upstream_confirmed profile=",
        " websocket_session=",
      ];
      return forbidden
        .filter((value) => contents.includes(value))
        .map((value) => filePath + ": contains restored Rust chain-log semantic " + value);
    }
    if (filePath === "crates/prodex-mojo-core/src/log.rs") {
      const required = [
        "prodex_mojo_route_affinity_log_render_v1(",
        "prodex_mojo_route_affinity_owner_logs_v1(",
        "prodex_mojo_chain_log_render_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime log adapter must retain Mojo ABI " + call);
    }
    return [];
  });
  const previousResponseLogRenderViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== "crates/prodex-runtime-proxy/src/previous_response_log.rs") return [];
    if (!contents.includes("prodex_mojo_core::log::render_previous_response_log(")) {
      return [filePath + ": previous-response log rendering must use Mojo"];
    }
    const forbidden = [
      "previous_response_log_suffix",
      "previous_response_not_found_prefix",
      "previous_response_event_prefix",
      '" previous_response_not_found profile="',
      '" previous_response_retry_immediate profile="',
      '" stale_continuation reason=',
      '" previous_response_fresh_fallback',
      '" previous_response_affinity_released profile="',
    ];
    return forbidden
      .filter((value) => contents.includes(value))
      .map((value) => filePath + ": contains restored Rust previous-response log rendering semantic " + value);
  });
  const structuredLogPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-runtime-proxy/src/log_event.rs") {
      const required = [
        "prodex_mojo_core::log::structured_log_field_policy(",
        "prodex_mojo_core::log::structured_log_sanitize(",
        "prodex_mojo_core::log::structured_log_strip_location(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": structured-log policy must retain Mojo call " + call);
      const forbidden = [
        /\bfn\s+runtime_proxy_log_key_needs_skip\s*\(/u,
        /\bfn\s+runtime_proxy_log_key_is_known_safe\s*\(/u,
        /\bfn\s+runtime_proxy_log_key_is_free_form\s*\(/u,
        /\bfn\s+runtime_proxy_log_value_is_stable_code\s*\(/u,
        /\bfn\s+runtime_proxy_log_key_is_location\s*\(/u,
        /\bfn\s+runtime_proxy_strip_log_location_secrets\s*\(/u,
        /\bfn\s+runtime_proxy_log_field_value_needs_quotes\s*\(/u,
      ];
      if (forbidden.some((pattern) => pattern.test(contents))) {
        violations.push(filePath + ": contains restored Rust structured-log policy semantics");
      }
      return violations;
    }
    if (
      filePath === "crates/prodex-mojo-core/src/log.rs"
      && (
        !contents.includes("prodex_mojo_structured_log_field_policy_v1(")
        || !contents.includes("prodex_mojo_structured_log_sanitize_v1(")
        || !contents.includes("prodex_mojo_structured_log_location_strip_v1(")
      )
    ) {
      return [filePath + ": structured-log ABI adapter must retain Mojo policy exports"];
    }
    return [];
  });
  const runtimeProxyObservabilityLabelViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-runtime-proxy/src/websocket_message.rs") {
      if (!contents.includes("runtime_websocket_direct_fallback_reason_label(self as i64)")) {
        return [filePath + ": websocket direct-fallback reason labels must use Mojo"];
      }
      if (
        contents.includes('"precommit_budget_exhausted"')
        || contents.includes('"candidate_exhausted"')
      ) {
        return [filePath + ": contains restored Rust websocket direct-fallback reason labels"];
      }
    }
    if (filePath === "crates/prodex-runtime-proxy/src/error_policy/stream.rs") {
      if (
        !contents.includes("runtime_http_error_class_label(class as i64)")
        || !contents.includes("runtime_http_error_action_label(action as i64)")
      ) {
        return [filePath + ": HTTP error observability labels must use Mojo"];
      }
      if (
        contents.includes("match class")
        || contents.includes("match action")
        || contents.includes('"profile_unavailable"')
        || contents.includes('"transient_5xx"')
        || contents.includes('"rotate_profile"')
      ) {
        return [filePath + ": contains restored Rust HTTP error observability label tables"];
      }
    }
    if (filePath === "crates/prodex-runtime-proxy/src/quota.rs") {
      const required = [
        "runtime_precommit_quota_block_reason_label(self as i64)",
        "runtime_quota_pressure_band_reason_label(band as i64)",
        "runtime_quota_window_status_reason_label(status as i64)",
        "runtime_quota_source_label(source as i64)",
      ];
      const missing = required.filter((call) => !contents.includes(call));
      if (missing.length > 0) {
        return missing.map((call) => filePath + ": quota observability labels must retain Mojo call " + call);
      }
      if (
        contents.includes('"quota_critical_floor_before_send"')
        || contents.includes('"quota_windows_unavailable_after_reprobe"')
        || contents.includes('"quota_healthy"')
        || contents.includes('"probe_cache"')
        || contents.includes('"persisted_snapshot"')
      ) {
        return [filePath + ": contains restored Rust quota observability label tables"];
      }
    }
    return [];
  });
  const websocketExecutorLabelViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-runtime-proxy/src/websocket_tcp_connect_executor/local_pressure.rs") {
      if (!contents.includes("runtime_websocket_local_pressure_label(self as i64)")) {
        return [filePath + ": websocket local-pressure labels must use Mojo"];
      }
      if (
        contents.includes('"dns_resolve_timeout"')
        || contents.includes('"dns_resolve_executor_overflow"')
        || contents.includes('"tcp_connect_executor_overflow"')
      ) {
        return [filePath + ": contains restored Rust websocket local-pressure label table"];
      }
    }
    if (filePath === "crates/prodex-runtime-proxy/src/websocket_tcp_connect_executor/task_kind.rs") {
      const required = [
        "runtime_websocket_task_label(self as i64)",
        "runtime_websocket_worker_thread_prefix(self as i64)",
        "runtime_websocket_dispatcher_thread_name(self as i64)",
        "runtime_websocket_overflow_enqueue_event(self as i64)",
        "runtime_websocket_overflow_dispatch_event(self as i64)",
        "runtime_websocket_overflow_reject_event(self as i64)",
      ];
      const missing = required.filter((call) => !contents.includes(call));
      if (missing.length > 0) {
        return missing.map((call) => filePath + ": websocket executor labels must retain Mojo call " + call);
      }
      if (
        contents.includes('"tcp_connect"')
        || contents.includes('"dns_resolve"')
        || contents.includes('"prodex-ws-connect"')
        || contents.includes('"prodex-ws-dns"')
        || contents.includes('"websocket_connect_overflow_')
        || contents.includes('"websocket_dns_overflow_')
      ) {
        return [filePath + ": contains restored Rust websocket executor label table"];
      }
    }
    return [];
  });
  const candidateSkipReasonViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-runtime-proxy/src/selection_plan.rs") {
      const required = [
        "candidate_skip_reason_kind(tag)",
        "decision.ready_skip_reason",
        "decision.fallback_skip_reason",
        "optimistic_current_candidate_decision(",
        "optimistic_candidate_reason_include_quota(",
        "runtime_route_reason_kind_from_tag",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": candidate skip reasons must retain Mojo-produced mapping " + call);
      if (
        contents.includes("impl RuntimeProfileAvailabilityState")
        || contents.includes('Some("auth_failure_backoff")')
        || contents.includes('Some("quota_exhausted_before_send")')
        || contents.includes("mojo_candidate_quota_guard_reason")
        || contents.includes("pub enum RuntimeOptimisticCurrentCandidateSkipReason")
        || contents.includes("OPTIMISTIC_CANDIDATE_AUTH_FAILURE")
        || contents.includes("OPTIMISTIC_CANDIDATE_QUOTA_THIN")
        || contents.includes("QuotaPressureBand(")
      ) {
        violations.push(filePath + ": contains restored Rust candidate skip-reason semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-app/src/runtime_proxy/selection_plan.rs") {
      if (
        !contents.includes("ready_skip_reason: candidate.ready_skip_reason")
        || !contents.includes("fallback_skip_reason: candidate.fallback_skip_reason")
      ) {
        return [filePath + ": app candidate plan must carry Mojo-produced skip reasons"];
      }
      if (
        contents.includes("self.availability.skip_reason()")
        || contents.includes('Some("auth_failure_backoff")')
        || contents.includes('Some("quota_exhausted_before_send")')
      ) {
        return [filePath + ": contains restored app-side candidate skip-reason semantics"];
      }
    }
    return [];
  });
  const infoRenderViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-terminal-ui/src/info.rs") {
      const required = [
        "info_render::format_quota_data_summary(",
        "info_render::format_runtime_policy_summary(",
        "info_render::format_runtime_logs_summary(",
        "info_render::format_runtime_tuning_workers(",
        "info_render::format_runtime_tuning_budgets(",
        "info_render::format_runtime_tuning_transport(",
        "info_render::format_pool_remaining(",
        "info_render::format_relative_duration(",
        "info_render::format_process_summary(",
        "info_render::format_load_summary(",
        "info_render::format_token_usage_summary(",
        "info_render::format_process_summary(",
        "info_render::format_load_summary(",
        "info_render::format_token_usage_summary(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": terminal info hard replacement must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        production.includes('format!("workers proxy=')
        || production.includes('format!("precommit=')
        || production.includes('format!("http-connect=')
        || production.includes("let seconds = seconds.max(0)")
        || production.includes("quota-compatible profile(s): live=")
        || production.includes("No active prodex runtime detected")
        || production.includes("No token_usage events found in")
        || production.includes("Yes ({total_count} total")
        || production.includes("No active prodex runtime detected")
        || production.includes("No token_usage events found in")
        || production.includes("Yes ({total_count} total")
      ) {
        violations.push(filePath + ": contains restored Rust terminal info rendering semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-app/src/app_commands/status/render.rs") {
      const required = [
        "info_render::format_human_bytes(",
        "info_render::format_human_count(",
        "info_render::format_token_efficiency(",
        "info_render::format_memory_percent(",
        "info_render::format_text_sparkline(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": status rendering must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        /\bfn\s+(?:human_bytes|human_count|token_efficiency|resource_memory_percent|text_sparkline)\s*\(/u.test(production)
      ) {
        violations.push(filePath + ": contains restored Rust status formatting semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-terminal-ui/src/runtime_launch.rs") {
      const required = [
        "info_render::format_runtime_launch_scored_candidate(",
        "info_render::format_runtime_provider_direct_launch_message(",
        "info_render::format_runtime_launch_quota_inspect_hint(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime-launch hard replacement must retain Mojo call " + call);
      if (
        contents.includes("Auto-rotating to profile")
        || contents.includes("Auto-selecting profile")
        || contents.includes("Quota preflight blocked profile")
        || contents.includes("Detected model_provider")
        || contents.includes("Inspect with")
      ) {
        violations.push(filePath + ": contains restored Rust runtime-launch rendering semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/info_render.rs") {
      const required = [
        "prodex_terminal_info_render_v1(",
        "format_human_bytes(",
        "format_human_count(",
        "format_token_efficiency(",
        "format_memory_percent(",
        "format_text_sparkline(",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => filePath + ": terminal info ABI adapter must retain " + marker);
    }
    if (filePath === "mojo/prodex_core/info_render.mojo") {
      const required = [
        "INFO_RENDER_HUMAN_BYTES",
        "INFO_RENDER_HUMAN_COUNT",
        "INFO_RENDER_TOKEN_EFFICIENCY",
        "INFO_RENDER_MEMORY_PERCENT",
        "INFO_RENDER_TEXT_SPARKLINE",
        "info_render_human_bytes(",
        "info_render_human_count(",
        "info_render_token_efficiency(",
        "info_render_memory_percent(",
        "info_render_text_sparkline(",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => filePath + ": Mojo status formatter must retain " + marker);
    }
    return [];
  });
  const doctorMarkerViolations = files
    .filter(([filePath, contents]) => filePath === RUNTIME_DOCTOR_MARKERS_FILE &&
      (!contents.includes("runtime_doctor_marker_known(") ||
        /\b(?:runtime_doctor_marker_registry|RuntimeDoctorMarker|RUNTIME_DOCTOR_MARKERS)\b/u.test(contents)))
    .map(([filePath]) => `${filePath}: marker recognition must use the Mojo classifier`);
  const doctorFailureClassViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== RUNTIME_DOCTOR_FAILURE_CLASS_FILE) return [];
    const body = contents.match(/\bfn\s+runtime_doctor_failure_class_counts\s*\([^]*?^\}/mu)?.[0];
    const markerLiteral = /"(?:runtime_proxy_[a-z0-9_]+|profile_[a-z0-9_]+|websocket_[a-z0-9_]+|compact_[a-z0-9_]+|local_rewrite_[a-z0-9_]+|previous_response_[a-z0-9_]+|chain_[a-z0-9_]+|stale_continuation|state_save_[a-z0-9_]+|continuation_journal_[a-z0-9_]+|upstream_[a-z0-9_]+|stream_read_error|local_writer_error|selection_skip_sync_probe|local_selection_blocked|quota_blocked|quota_critical_floor_before_send|responses_pre_send_skip)"/u;
    const semanticOwner = body?.includes("prodex_mojo_core::rich::runtime_doctor_marker_semantics(") ||
      ["admission", "auth", "continuation", "persistence", "quota", "transport"]
        .every((label) => body?.includes(`counts.failure_${label}`));
    return semanticOwner && !markerLiteral.test(body)
      ? []
      : [`${filePath}: failure-class counts must use Mojo tags without Rust marker lists`];
  });
  const doctorMarkerSummaryCountsViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === DOCTOR_MARKER_SUMMARY_COUNTS_CONSUMER_FILE) {
      const reducer = contents.match(/\bfn\s+runtime_doctor_marker_summary_counts\s*\([^]*?^\}/mu)?.[0];
      const finalize = contents.match(/\bpub fn runtime_doctor_finalize_log_summary\s*\([^]*?^\}/mu)?.[0];
      if (!finalize) {
        return contents.includes("runtime_doctor_facet_count(summary")
          ? [`${filePath}: fixed selection and failure totals must use the Mojo batch reducer before and after quota-floor synthesis`]
          : [];
      }
      return reducer?.includes("prodex_mojo_core::rich::runtime_doctor_marker_summary_counts(") &&
          finalize.includes("runtime_doctor_marker_summary_counts(summary)") &&
          finalize.includes("runtime_doctor_failure_class_counts(runtime_doctor_marker_summary_counts(summary))") &&
          !/for\s*\(\s*\(\s*marker\s*,\s*count\s*\)\s*in\s*&summary\.marker_counts/u.test(contents)
        ? []
        : [`${filePath}: fixed selection and failure totals must use the Mojo batch reducer before and after quota-floor synthesis`];
    }
    if (filePath === DOCTOR_MARKER_SUMMARY_COUNTS_SELECTION_FILE) {
      const body = contents.match(/\bpub\(super\) fn runtime_doctor_record_selection_summary\s*\([^]*?^\}/mu)?.[0];
      return body && /summary\.selection_summary\.(?:picked|kept|skipped|blocked)\s*\+=/u.test(body)
        ? [`${filePath}: selection bucket totals must be reduced from marker counts, not incremented per event`]
        : [];
    }
    if (filePath === DOCTOR_MARKER_ABI_ADAPTER_FILE &&
        (!contents.includes("prodex_mojo_runtime_doctor_marker_summary_counts_v1(") ||
          !contents.includes("RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_ABI_VERSION: i64 = 1") ||
          !contents.includes("RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_MAX_BATCH: usize = 256"))) {
      return [`${filePath}: marker summary adapter must use the versioned bounded Mojo ABI`];
    }
    if (filePath === DOCTOR_MARKER_SUMMARY_COUNTS_MOJO_FILE &&
        (!contents.includes('@export("prodex_mojo_runtime_doctor_marker_summary_counts_v1")') ||
          !contents.includes("RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_ABI_VERSION: Int64 = 1") ||
          !contents.includes("RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_MAX_BATCH: Int64 = 256") ||
          !contents.includes("runtime_doctor_marker_selection_bucket(marker)") ||
          !contents.includes("runtime_doctor_marker_failure_class(marker)"))) {
      return [`${filePath}: fixed marker totals must retain their versioned bounded Mojo reducer`];
    }
    if (filePath === DOCTOR_MARKER_ABI_TEST_FILE &&
        !contents.includes("marker_summary_counts_abi_tags_fixed_selection_and_failure_totals")) {
      return [`${filePath}: direct ABI coverage must exercise fixed selection and failure totals`];
    }
    if (filePath === DOCTOR_MARKER_SUMMARY_COUNTS_CALLER_TEST_FILE &&
        !contents.includes("runtime_doctor_marker_summary_reducer_preserves_caps_synthesis_order_and_unicode")) {
      return [`${filePath}: runtime-doctor caller coverage must protect reducer caps, synthesis order, and Unicode truncation`];
    }
    return [];
  });
  const doctorCompactExitCountsViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === DOCTOR_COMPACT_EXIT_COUNTS_CONSUMER_FILE) {
      const body = contents.match(/\bpub\(super\) fn runtime_doctor_compact_exit_counts\s*\([^]*?^\}/mu)?.[0];
      const restoredAliases = /"compact_(?:exit_)?(?:candidate_exhausted|committed|committed_owner|followup_owner|lineage_released|overload_conservative_retry|precommit_budget_exhausted|pressure_shed|quota_unclassified|retryable_failure|transport_failure)"/u;
      return body?.includes("prodex_mojo_core::rich::runtime_doctor_compact_exit_counts(") &&
          !restoredAliases.test(body)
        ? []
        : [`${filePath}: compact-exit alias grouping must use the Mojo reducer without Rust marker tables`];
    }
    if (filePath === DOCTOR_MARKER_ABI_ADAPTER_FILE &&
        (!contents.includes("prodex_mojo_runtime_doctor_compact_exit_counts_v1(") ||
          !contents.includes("RUNTIME_DOCTOR_COMPACT_EXIT_COUNTS_ABI_VERSION: i64 = 1") ||
          !contents.includes("RUNTIME_DOCTOR_COMPACT_EXIT_COUNT_LABELS"))) {
      return [`${filePath}: compact-exit adapter must use the versioned Mojo reducer and fixed output slots`];
    }
    if (filePath === DOCTOR_MARKER_SUMMARY_COUNTS_MOJO_FILE &&
        (!contents.includes('@export("prodex_mojo_runtime_doctor_compact_exit_counts_v1")') ||
          !contents.includes("def runtime_doctor_compact_exit_bucket(") ||
          !contents.includes('rich_view_matches_literal["compact_exit_candidate_exhausted"]'))) {
      return [`${filePath}: compact-exit alias policy must remain in the production Mojo reducer`];
    }
    if (filePath === DOCTOR_MARKER_ABI_TEST_FILE) {
      const aliases = [
        "compact_candidate_exhausted", "compact_exit_candidate_exhausted",
        "compact_committed", "compact_exit_committed",
        "compact_committed_owner", "compact_exit_committed_owner",
        "compact_followup_owner", "compact_exit_followup_owner",
        "compact_lineage_released", "compact_exit_lineage_released",
        "compact_overload_conservative_retry", "compact_exit_overload_conservative_retry",
        "compact_precommit_budget_exhausted", "compact_exit_precommit_budget_exhausted",
        "compact_pressure_shed", "compact_exit_pressure_shed",
        "compact_quota_unclassified", "compact_exit_quota_unclassified",
        "compact_retryable_failure", "compact_exit_retryable_failure",
        "compact_transport_failure",
      ];
      if (!contents.includes("compact_exit_counts_abi_sums_aliases_and_rejects_invalid_inputs") ||
          !aliases.every((alias) => contents.includes(`"${alias}"`))) {
        return [`${filePath}: direct ABI coverage must assert all compact-exit marker buckets and invalid inputs`];
      }
    }
    if (filePath === DOCTOR_COMPACT_EXIT_COUNTS_CALLER_TEST_FILE &&
        !contents.includes("runtime_doctor_diagnosis_aggregates_compact_exit_alias_counts_in_stable_order")) {
      return [`${filePath}: production diagnosis caller must protect compact-exit alias totals and order`];
    }
    return [];
  });
  const doctorTimelineDetailViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === DOCTOR_TIMELINE_DETAIL_CONSUMER_FILE) {
      const body = contents.match(/\bpub\(super\) fn runtime_doctor_request_timeline_detail\s*\([^]*?^\}/mu)?.[0];
      const rustFormatter = /runtime_doctor_truncate_value|\.chars\(\)|parts\.join\(/u;
      return body?.includes("prodex_mojo_core::rich::runtime_doctor_render(") &&
          !rustFormatter.test(contents) &&
          contents.includes("request_timeline_detail_preserves_field_order_cap_and_unicode")
        ? []
        : [`${filePath}: request-timeline field ordering, cap, truncation, and joining must use the Mojo renderer with caller coverage`];
    }
    if (filePath === DOCTOR_RENDER_ADAPTER_FILE &&
        (!contents.includes("RUNTIME_DOCTOR_RENDER_REQUEST_TIMELINE_DETAIL: i64 = 22") ||
          !contents.includes("prodex_mojo_runtime_doctor_render_v1(") ||
          !contents.includes("timeline_detail_preserves_order_caps_fields_and_truncates_unicode"))) {
      return [`${filePath}: request-timeline rendering must retain its bounded real-Mojo operation and ABI test`];
    }
    if (filePath === DOCTOR_RENDER_MOJO_FILE &&
        (!contents.includes("RENDER_REQUEST_TIMELINE_DETAIL: Int64 = 22") ||
          !contents.includes("def runtime_doctor_render_request_timeline_detail(") ||
          !contents.includes("def runtime_doctor_render_put_bounded_value(") ||
          !contents.includes("prefix_characters + 1") ||
          !contents.includes("maximum_characters + 1") ||
          !contents.includes("runtime_doctor_render_request_timeline_detail(writer, input)") ||
          !contents.includes('@export("prodex_mojo_runtime_doctor_render_v1")'))) {
      return [`${filePath}: request-timeline detail semantics must stay in the production Mojo renderer`];
    }
    return [];
  });
  const doctorLastMarkerLineViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === DOCTOR_LAST_MARKER_LINE_CONSUMER_FILE) {
      const body = contents.match(/\bpub\(super\) fn runtime_doctor_truncate_line\s*\([^]*?^\}/mu)?.[0];
      return body?.includes("RUNTIME_DOCTOR_RENDER_LAST_MARKER_LINE_TRUNCATION") &&
          body.includes("prodex_mojo_core::rich::runtime_doctor_render(") &&
          !/\.chars\(\)|\.take\(|\.count\(\)/u.test(body) &&
          contents.includes("last_marker_line_trims_before_mojo_unicode_truncation")
        ? []
        : [`${filePath}: last-marker line truncation must use the Mojo renderer with caller coverage`];
    }
    if (filePath === DOCTOR_RENDER_ADAPTER_FILE &&
        (!contents.includes("RUNTIME_DOCTOR_RENDER_LAST_MARKER_LINE_TRUNCATION: i64 = 23") ||
          !contents.includes("last_marker_line_preserves_160_unicode_scalars_and_truncates_at_boundary"))) {
      return [`${filePath}: last-marker line truncation must retain bounded real-Mojo operation 23 and its ABI boundary test`];
    }
    if (filePath === DOCTOR_RENDER_MOJO_FILE &&
        (!contents.includes("RENDER_LAST_MARKER_LINE_TRUNCATION: Int64 = 23") ||
          !contents.includes("def runtime_doctor_render_put_bounded_value(") ||
          !contents.includes("def runtime_doctor_render_last_marker_line(") ||
          !contents.includes("runtime_doctor_render_put_bounded_value(writer, runtime_doctor_render_input_value(input, index), 48, StringSlice(\"...\"), 3)") ||
          !contents.includes("runtime_doctor_render_put_bounded_value(\n        writer,\n        runtime_doctor_render_input_value(input, 0),\n        160,") ||
          !contents.includes("runtime_doctor_render_last_marker_line(writer, input)") ||
          !contents.includes("input.operation > RENDER_LAST_MARKER_LINE_TRUNCATION"))) {
      return [`${filePath}: bounded Unicode truncation for timeline details and last-marker lines must stay in the production Mojo renderer`];
    }
    return [];
  });
  const runtimeDoctorPlanInputViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === RUNTIME_DOCTOR_PLAN_ADAPTER_FILE) {
      const rustValidators = /\bfn (?:input_is_valid|summary_plan_input_is_valid|state_plan_input_is_valid|route_plan_input_is_valid)\s*\(/u;
      return !rustValidators.test(contents) &&
          contents.includes("input_contracts_are_validated_by_mojo")
        ? []
        : [`${filePath}: runtime-doctor fixed-layout input contracts must be validated by Mojo and covered through the real Rust caller`];
    }
    if (filePath === RUNTIME_DOCTOR_PLAN_MOJO_FILE &&
        (!contents.includes("def runtime_doctor_input_valid(") ||
          !contents.includes("if not runtime_doctor_input_valid(input):") ||
          !contents.includes("def runtime_doctor_summary_validate_input(") ||
          !contents.includes("for index in range(Int(RUNTIME_DOCTOR_SUMMARY_MARKER_COUNT)):") ||
          !contents.includes("runtime_doctor_count_valid(input.marker_counts[index])") ||
          !contents.includes("if not runtime_doctor_summary_validate_input(input):") ||
          !contents.includes("def runtime_doctor_state_validate_input(") ||
          !contents.includes("if not runtime_doctor_state_validate_input(input):") ||
          !contents.includes("def runtime_doctor_route_plan_valid(") ||
          !contents.includes("if not runtime_doctor_route_plan_valid(input):"))) {
      return [`${filePath}: all runtime-doctor plan ABI input contracts, including the bounded marker arena, must be validated in Mojo`];
    }
    return [];
  });
  const cliDefaultRunViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === CLI_DEFAULT_RUN_CONSUMER_FILE) {
      const body = contents.match(/\bpub fn should_default_cli_invocation_to_run\s*\([^]*?^\}/mu)?.[0];
      if (!body) {
        return contents.includes("parse_cli_command_from")
          ? [`${filePath}: CLI default-run decision must use the Mojo launch-arguments policy`]
          : [];
      }
      return body?.includes("prodex_mojo_core::launch::default_cli_invocation_to_run(") &&
          !/matches!\s*\(/u.test(body)
        ? []
        : [`${filePath}: CLI default-run decision must use the Mojo launch-arguments policy`];
    }
    if (filePath === CLI_DEFAULT_RUN_ABI_FILE &&
        (!contents.includes("const CLI_DEFAULT_RUN: i64 = 13;") ||
          !contents.includes("pub fn default_cli_invocation_to_run(") ||
          !contents.includes("prodex_mojo_launch_args_v1("))) {
      return [`${filePath}: CLI default-run adapter must retain launch Mojo operation 13`];
    }
    if (filePath === CLI_DEFAULT_RUN_MOJO_FILE &&
        (!contents.includes("LAUNCH_CLI_DEFAULT_RUN: Int64 = 13") ||
          !contents.includes("def launch_cli_default_run_policy(") ||
          !contents.includes("launch_cli_default_run_policy(arguments, count, metadata)"))) {
      return [`${filePath}: CLI default-run classification must remain in the launch-arguments Mojo kernel`];
    }
    if (filePath === CLI_DEFAULT_RUN_ABI_TEST_FILE &&
        !contents.includes("default_cli_invocation_policy_uses_real_mojo_classification")) {
      return [`${filePath}: direct real-Mojo CLI default-run coverage is required`];
    }
    if (filePath === CLI_DEFAULT_RUN_CALLER_TEST_FILE &&
        !contents.includes("bare_invocation_defaults_to_run_and_clap_keeps_help_and_version")) {
      return [`${filePath}: CLI default-run behavior must retain caller-boundary coverage`];
    }
    return [];
  });
  const responseMetadataViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === RESPONSE_METADATA_FILE) {
      const body = contents.match(/\bfn\s+runtime_response_metadata_from_value\s*\([^]*?^\}/mu)?.[0];
      const rustSemantics = /\bfn\s+(?:extract_runtime_token_usage_candidate|runtime_token_usage_from_usage_value|extract_runtime_turn_state_from_header_entry|extract_runtime_turn_state_header_value|push_runtime_response_id)\s*\(/u;
      return body?.includes("prodex_mojo_core::json::runtime_response_metadata(") &&
          !rustSemantics.test(contents)
        ? []
        : [`${filePath}: response metadata decisions must use the Mojo plan without Rust copies`];
    }
    if (filePath === RESPONSE_METADATA_ADAPTER_FILE &&
        !contents.includes("prodex_runtime_response_metadata_v1(")) {
      return [`${filePath}: response metadata adapter must call its versioned Mojo ABI`];
    }
    if (filePath === RESPONSE_METADATA_MOJO_FILE &&
        (!contents.includes('@export("prodex_runtime_response_metadata_v1")') ||
          !contents.includes("runtime_response_metadata_token_usage(tree)"))) {
      return [`${filePath}: response metadata production owner must retain its ABI and usage planner`];
    }
    return [];
  });
  const doctorMarkerAbiViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === DOCTOR_MARKER_ABI_ADAPTER_FILE) {
      return contents.includes("prodex_mojo_runtime_doctor_marker_semantics_v2(") &&
          contents.includes("RUNTIME_DOCTOR_MARKER_SEMANTICS_ABI_VERSION: i64 = 2") &&
          !contents.includes("prodex_mojo_runtime_doctor_marker_semantics_v1(")
        ? []
        : [`${filePath}: four-slot marker semantics must use the version-2 ABI`];
    }
    if (filePath === DOCTOR_MARKER_ABI_MOJO_FILE &&
        (!contents.includes('@export("prodex_mojo_runtime_doctor_marker_semantics_v2")') ||
          !contents.includes("RUNTIME_DOCTOR_MARKER_SEMANTICS_ABI_VERSION: Int64 = 2") ||
          !contents.includes("output[unsafe_offset=3] = runtime_doctor_marker_failure_class(marker)") ||
          contents.includes("prodex_mojo_runtime_doctor_marker_semantics_v1"))) {
      return [`${filePath}: four-slot marker output must be exported under the version-2 ABI`];
    }
    if (filePath === DOCTOR_MARKER_ABI_TEST_FILE &&
        (!contents.includes("prodex_mojo_runtime_doctor_marker_semantics_v2(") ||
          !contents.includes("output[3]"))) {
      return [`${filePath}: marker ABI tests must exercise version 2 and its fourth slot`];
    }
    return [];
  });
  const statusSummaryViolations = files
    .filter(([filePath, contents]) => filePath === STATUS_SUMMARY_FILE &&
      !contents.includes("status_quota_summary_batch(&inputs)"))
    .map(([filePath]) => `${filePath}: status quota summary must use the Mojo kernel`);
  const geminiBufferedResponseViolations = files
    .filter(([filePath, contents]) => filePath === GEMINI_BUFFERED_RESPONSE_FILE &&
      (!contents.includes("gemini_buffered_response_kernel(input)") ||
        /\bfn\s+gemini_insert_response_message\s*\(/u.test(contents)))
    .map(([filePath]) => `${filePath}: buffered response assembly must use the Mojo kernel`);
  const fingerprintDeltaViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === FINGERPRINT_ARTIFACTS_FILE && /\bfn\s+smart_context_fingerprint_map\s*\(/u.test(contents)) {
      return [`${filePath}: contains a replaced Rust fingerprint map`];
    }
    if (filePath === FINGERPRINT_DELTA_FILE) {
      const body = contents.match(/\bpub fn smart_context_fingerprint_delta\([^]*?^\}/mu)?.[0];
      if (!body?.includes("smart_context_fingerprint_delta_mojo(") ||
          /\bfn\s+smart_context_fingerprint_delta_rust\s*\(/u.test(contents)) {
        return [`${filePath}: fingerprint delta must use the Mojo plan`];
      }
    }
    return [];
  });
  const modelSpecViolations = files
    .filter(([filePath, contents]) => filePath === MODEL_SPEC_FILE &&
      /\bfn\s+matches_id_or_alias\s*\(/u.test(contents) &&
      !contents.includes("resolve_catalog_model("))
    .map(([filePath]) => `${filePath}: model matcher must use the Mojo catalog kernel`);
  const catalogModelViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs") return [];
    const body = contents.match(/\bpub\(super\) fn external_catalog_model_indices\([^]*?^\}/mu)?.[0];
    return body?.includes("merge_catalog_ids(") &&
      !FEATURE_OFF_RUST_PATH.test(body) &&
      !/\b(?:BTreeSet|to_ascii_lowercase)\b/u.test(body)
      ? [] : [`${filePath}: catalog dedup must use Mojo without a feature-off Rust path`];
  });
  const deepseekShapingViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== DEEPSEEK_SHAPING_FILE) return [];
    const completedViolations = DEEPSEEK_SHAPING_COMPLETED_FNS.flatMap((name) => {
      const start = contents.indexOf(`pub fn ${name}(`);
      if (start < 0) return [`${filePath}: missing Mojo-owned ${name}`];
      const next = contents.indexOf("\npub fn ", start + 1);
      return FEATURE_OFF_RUST_PATH.test(contents.slice(start, next < 0 ? undefined : next))
        ? [`${filePath}: ${name} contains a feature-off Rust path`] : [];
    });
    const choiceDelta = contents.match(/\bpub fn deepseek_provider_core_stream_choice_delta\([^]*?^\}/mu)?.[0];
    if (choiceDelta?.includes("DeepSeekKernelOperation::StreamChoiceDelta") &&
        !choiceDelta.includes('Some("")')) return completedViolations;
    return [...completedViolations,
      `${filePath}: stream choice empty-text filtering must stay in Mojo`];
  });
  const deepseekStreamFallbackViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === DEEPSEEK_SHAPING_FILE) {
      return DEEPSEEK_STREAM_PROMOTED_OPERATIONS
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: DeepSeek stream hard replacement must retain ${marker}`);
    }
    if (filePath === DEEPSEEK_STREAM_RESPONSE_VALUES_FILE) {
      return ["DeepSeekKernelOperation::StreamResponseValue", "DeepSeekKernelOperation::StreamAssistantMessage"]
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: DeepSeek stream response hard replacement must retain ${marker}`);
    }
    if (filePath === DEEPSEEK_RESPONSE_METADATA_FILE) {
      const violations = contents.includes("DeepSeekKernelOperation::ResponseMetadata") ? [] : [filePath + ": DeepSeek response metadata must use Mojo"];
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (production.includes(".get(\"choices\")") || production.includes(".get(\"reasoning_content\")") || production.includes(".get(\"annotations\")") || production.includes(".get(\"finish_reason\")") || production.includes(".get(\"system_fingerprint\")") || production.includes("let mut metadata = serde_json::Map::new()")) {
        violations.push(filePath + ": contains restored Rust DeepSeek response metadata extraction");
      }
      return violations;
    }
    return [];
  });
  const quotaDisplayPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === QUOTA_TIME_FILE) {
      const violations = contents.includes("prodex_mojo_core::quota::quota_window_label(")
        ? []
        : [filePath + ": quota window labels must retain Mojo planner"];
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        production.includes("17_700..=18_300")
        || production.includes("601_200..=608_400")
        || production.includes("2_505_600..=2_678_400")
      ) {
        violations.push(filePath + ": contains restored Rust quota window threshold policy");
      }
      return violations;
    }
    if (filePath === QUOTA_GEMINI_DISPLAY_FILE) {
      const required = [
        "quota_gemini_bucket_label(",
        "quota_gemini_display(",
        "quota_gemini_bucket_summary(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": Gemini quota display migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const retired of [
        "fn gemini_blocked_buckets(",
        '.strip_prefix("models/")',
        ".map(str::to_ascii_lowercase)",
        '"gemini quota unknown"',
        'format!("{label} {remaining}/{total}")',
        ".filter_map(|numeric| numeric.remaining_percent)",
      ]) {
        if (production.includes(retired)) {
          violations.push(filePath + ": contains restored Rust Gemini quota display policy");
          break;
        }
      }
      return violations;
    }
    if (filePath === QUOTA_COPILOT_FILE) {
      const required = [
        "quota_copilot_feature_key(",
        "quota_copilot_display(",
        "quota_copilot_main_remaining_percent(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": Copilot quota display migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const retired of [
        "fn copilot_quota_feature_labels(",
        "fn copilot_blocked_features(",
        '[("chat", "chat"), ("completions", "comp")]',
        'parts.join(" | ")',
        "if blocked.is_empty()",
      ]) {
        if (production.includes(retired)) {
          violations.push(filePath + ": contains restored Rust Copilot quota display policy");
          break;
        }
      }
      return violations;
    }
    if (filePath === QUOTA_REPORTS_FILE) {
      const body = contents.match(/\bfn\s+compare_quota_report_sort_records\([^]*?^\}/mu)?.[0];
      const violations = body?.includes("prodex_mojo_core::quota::quota_report_compare(")
        ? []
        : [filePath + ": quota report sorting must retain Mojo comparator"];
      if (!contents.includes("prodex_mojo_core::quota::quota_workspace_label(")) {
        violations.push(filePath + ": quota workspace labels must retain Mojo policy");
      }
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        /\bfn\s+compare_text\s*\(/u.test(production)
        || body?.includes("match sort")
        || body?.includes("to_ascii_lowercase()")
        || production.includes("fn short_workspace_id(")
        || production.includes(".chars().collect::<Vec<_>>()")
      ) {
        violations.push(filePath + ": contains restored Rust quota report comparator semantics");
      }
      return violations;
    }
    if (filePath === QUOTA_POOL_FILE) {
      const required = [
        "prodex_mojo_core::quota::quota_ready_pool_remaining(",
        "prodex_mojo_core::quota::quota_info_pool_remaining(",
        "copilot_main_remaining_percent(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": quota-pool display migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const retired of [
        '["chat", "completions"]',
        'return "Unavailable".to_string()',
        'windows.join(" | ")',
        'format!("{total_remaining}% across {profiles_with_data} profile(s)")',
      ]) {
        if (production.includes(retired)) {
          violations.push(filePath + ": contains restored Rust quota-pool display policy");
          break;
        }
      }
      return violations;
    }
    if (filePath === QUOTA_AUTH_FILE) {
      const violations = contents.includes("quota_usage_auth_sync_source_label(")
        ? []
        : [filePath + ": auth-sync source labels must retain Mojo mapping"];
      const body = contents.match(/\bpub fn usage_auth_sync_source_label\([^]*?^\}/mu)?.[0];
      if (body?.includes("UsageAuthSyncSource::Reloaded") || body?.includes('"reloaded"')) {
        violations.push(filePath + ": contains restored Rust auth-sync source label mapping");
      }
      return violations;
    }
    if (filePath === QUOTA_ADAPTER_FILE) {
      const required = [
        "prodex_quota_display_label_v1(",
        "prodex_quota_window_label_plan_v1(",
        "prodex_quota_copilot_feature_key_v1(",
        "prodex_quota_copilot_display_v1(",
        "prodex_quota_copilot_main_remaining_percent_v1(",
        "prodex_quota_ready_pool_remaining_v1(",
        "prodex_quota_info_pool_remaining_v1(",
        "prodex_quota_gemini_bucket_label_v1(",
        "prodex_quota_gemini_bucket_summary_v1(",
        "prodex_quota_gemini_display_v1(",
        "prodex_quota_report_compare_v1(",
        "prodex_quota_workspace_label_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": quota display-policy adapter must retain Mojo ABI " + call);
    }
    return [];
  });
  const quotaWindowViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== QUOTA_WINDOWS_FILE) return [];
    const violations = [];
    if (!contents.includes("prodex_mojo_core::quota::quota_blocked_status_label(")) {
      violations.push(filePath + ": blocked quota status rendering must retain Mojo-owned label mapping");
    }
    if (/\bfn\s+quota_error_summary_(?:basic|transport|auth|response)\s*\(/u.test(contents)) {
      violations.push(`${filePath}: contains a Rust quota error classifier`);
    }
    for (const name of ["format_blocked_quota_status", "quota_error_summary"]) {
      const body = contents.match(new RegExp(`\\bfn\\s+${name}\\([^]*?^\\}`, "mu"))?.[0];
      if (body && FEATURE_OFF_RUST_PATH.test(body)) {
        violations.push(`${filePath}: ${name} contains a feature-off Rust classifier`);
      }
    }
    return violations;
  });
  const quotaResetEpochViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === QUOTA_WINDOWS_FILE) {
      const required = [
        "prodex_mojo_core::quota::reset_epoch::quota_reset_json_epoch(&canonical)",
      ];
      const violations = required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: reset JSON extraction must retain ${marker}`);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        /\bfn\s+quota_json_(?:i64|path|object_i64|reset|precedence)\w*\s*\(/u.test(production)
        || production.includes("QuotaResetEpochInput {")
      ) {
        violations.push(`${filePath}: contains restored Rust reset JSON extraction or precedence policy`);
      }
      return violations;
    }
    if (filePath === QUOTA_RESET_EPOCH_ADAPTER_FILE) {
      const required = [
        "pub struct QuotaResetEpochInput",
        "pub fn quota_reset_epoch_precedence(",
        "prodex_quota_reset_epoch_v1(",
        "pub fn quota_reset_json_epoch(",
        "prodex_quota_reset_json_epoch_v1(",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: typed reset-epoch adapter must retain ${marker}`);
    }
    if (filePath === QUOTA_RESET_EPOCH_MOJO_FILE) {
      const required = [
        '@export("prodex_quota_reset_epoch_v1")',
        '@export("prodex_quota_reset_json_epoch_v1")',
        "quota_reset_epoch_plan(",
        "quota_json_object_member_casefold_first(",
        "quota_json_i64(",
        "primary_used >= 100",
        "secondary_used >= 100",
        "fields[9] == 1",
        "fields[11] == 1",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: quota reset parsing and precedence must remain Mojo-owned (${marker})`);
    }
    if (filePath === QUOTA_RESET_EPOCH_TEST_FILE) {
      const required = [
        "quota_reset_epoch_prefers_valid_candidates_in_declared_order",
        "quota_reset_epoch_applies_used_percent_gates_and_missing_values",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: direct Mojo reset-epoch tests must retain ${marker}`);
    }
    if (filePath === QUOTA_RESET_EPOCH_CALLER_TEST_FILE) {
      const required = [
        "quota_reset_json_uses_top_level_then_nested_candidate_order",
        "quota_reset_json_uses_used_percent_gates_and_header_fallback_order",
        "quota_reset_json_preserves_serde_duplicate_key_behavior",
        "quota_reset_json_ignores_malformed_and_non_object_values",
      ];
      return required
        .filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: quota caller-boundary tests must retain ${marker}`);
    }
    return [];
  });
  const rehydrateViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== REHYDRATE_FILE) return [];
    const body = contents.match(/\bpub fn smart_context_auto_rehydrate_plan\([^]*?^fn smart_context_auto_rehydrate_plan_mojo/mu)?.[0];
    return body && FEATURE_OFF_RUST_PATH.test(body)
      ? [`${filePath}: rehydration has a feature-off Rust planner`] : [];
  });
  const budgetTierViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== REHYDRATE_FILE) return [];
    const body = contents.match(/\bpub fn smart_context_token_budget_tier\([^]*?^\}/mu)?.[0];
    return body && (FEATURE_OFF_RUST_PATH.test(body) ||
      !body.includes("smart_context_u64_budget_tier("))
      ? [`${filePath}: budget tier must use the Mojo policy`] : [];
  });
  const staticItemViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== "crates/prodex-runtime-proxy/src/smart_context/static_context.rs") return [];
    const body = contents.match(/\bfn smart_context_stabilize_static_context_items_bounded\([^]*?(?=^fn smart_context_reduce_static_context_items_mojo\()/mu)?.[0];
    return body && !FEATURE_OFF_RUST_PATH.test(body) && body.includes("smart_context_reduce_static_context_items_mojo(")
      ? [] : [`${filePath}: static-item selection must use Mojo in every feature mode`];
  });
  const profileExportPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-profile-export/src/data_model.rs") {
      const required = [
        "prodex_mojo_core::profile_export::validate_collection(",
        "prodex_mojo_core::profile_export::validate_profile_secret_files(",
        "prodex_mojo_core::profile_export::validate_nested_secret_bytes(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => `${filePath}: profile-export limits must retain Mojo call ${call}`);
      if (/profiles\.len\(\)\s*>\s*PROFILE_EXPORT_MAX_PROFILES|secret_files\.len\(\)\s*>\s*PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE|value\.len\(\)\s*>\s*PROFILE_EXPORT_NESTED_JSON_MAX_BYTES/u.test(contents)) {
        violations.push(`${filePath}: contains restored Rust profile-export count/size policy`);
      }
      return violations;
    }
    if (filePath === "crates/prodex-profile-export/src/envelope.rs") {
      const required = [
        "prodex_mojo_core::profile_export::validate_password_bytes(",
        "prodex_mojo_core::profile_export::validate_pbkdf2_iterations(",
        "prodex_mojo_core::profile_export::validate_argon2(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => `${filePath}: profile-export KDF/password limits must retain Mojo call ${call}`);
      if (/password\.is_empty\(\)|PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS\.\.=PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS|PROFILE_EXPORT_ARGON2_MIN_(?:MEMORY_KIB|ITERATIONS|PARALLELISM)/u.test(contents.split("#[cfg(test)]", 1)[0])) {
        violations.push(`${filePath}: contains restored Rust profile-export password/KDF range policy`);
      }
      return violations;
    }
    if (filePath === "crates/prodex-profile-export/src/selection.rs") {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const required = [
        "prodex_mojo_core::profile_export::profile_export_selection_plan(&available, &requested)",
        "prodex_mojo_core::profile_export::profile_export_active_profile_selected(",
        "prodex_mojo_core::profile_export::profile_import_active_profile_plan(",
      ];
      const restoredRust = [
        "available_names.is_empty()",
        "requested_names.is_empty()",
        "seen.insert(",
        "available_names.contains(",
        ".any(|name| name == active_profile)",
        "existing_active_profile.map(ToOwned::to_owned).or_else",
        "resolved_profile_names.get(active)",
      ];
      return required.every((marker) => production.includes(marker)) &&
        restoredRust.every((marker) => !production.includes(marker))
        ? []
        : [`${filePath}: profile selection and active-profile resolution must use Mojo without a Rust policy copy`];
    }
    if (filePath === "crates/prodex-mojo-core/src/profile_export.rs") {
      const required = [
        "prodex_profile_export_selection_v1(",
        "pub fn profile_export_selection_plan(",
        "profile_export_selection_plan_is_mojo_owned",
        '#[path = "profile_export/active_profile.rs"]',
        "profile_active_selection_and_import_plan_are_mojo_owned",
      ];
      return required.filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: profile-export selection adapter must retain ${marker}`);
    }
    if (filePath === "crates/prodex-mojo-core/src/profile_export/active_profile.rs") {
      const required = [
        "prodex_profile_export_active_profile_selected_v1(",
        "pub fn profile_export_active_profile_selected(",
        "prodex_profile_import_active_profile_plan_v1(",
        "pub fn profile_import_active_profile_plan(",
      ];
      return required.filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: active-profile adapter must retain ${marker}`);
    }
    if (filePath === "crates/prodex-mojo-core/src/profile_export/copilot.rs") {
      const required = [
        "prodex_profile_export_copilot_strip_json_line_comments_v1(",
        "pub fn strip_copilot_json_line_comments(",
        "prodex_profile_export_copilot_metadata_v1(",
        "pub fn copilot_version_triplet(",
        "pub fn copilot_platform_label(",
        "prodex_profile_export_copilot_url_v1(",
        "pub fn copilot_user_api_origin(",
        "pub fn copilot_models_api_url(",
        "prodex_profile_export_copilot_import_state_v1(",
        "pub fn copilot_import_state_plan(",
        "copilot_jsonc_line_comment_stripping_is_mojo_owned",
        "copilot_metadata_and_url_policy_are_mojo_owned",
      ];
      return required.filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Copilot JSONC adapter must retain ${marker}`);
    }
    if (filePath === "crates/prodex-profile-export/src/copilot.rs") {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const required = [
        "prodex_mojo_core::profile_export::strip_copilot_json_line_comments(raw)",
        "prodex_mojo_core::profile_export::copilot_version_triplet(raw)",
        "prodex_mojo_core::profile_export::copilot_platform_label(os, arch)",
        "prodex_mojo_core::profile_export::copilot_user_api_origin(host)",
        "prodex_mojo_core::profile_export::copilot_models_api_url(host)",
        "prodex_mojo_core::profile_export::copilot_import_state_plan(",
      ];
      const restoredRust = [
        "fn strip_json_line_comments(",
        "fn append_json_string_char(",
        "fn skip_json_line_comment(",
        "raw.split('.')",
        "match (os, arch)",
        "fn authority_host_and_port(",
        "trim_end_matches('/')",
        'format!("api.{authority}")',
        'format!("https://copilot-api.',
        "let activate = !has_active_profile || activate_requested",
        "requested_name != existing_name",
        "if profile_name_exists(requested_name)",
      ];
      return required.every((marker) => production.includes(marker)) &&
        restoredRust.every((marker) => !production.includes(marker))
        ? []
        : [`${filePath}: Copilot JSONC/version/platform policy must use Mojo without a Rust semantic copy`];
    }
    if (filePath === "mojo/prodex_core/profile_export_policy.mojo") {
      const required = [
        '@export("prodex_profile_export_selection_v1")',
        "if available_count == 0:",
        "if requested_count == 0:",
        "if found < 0:",
        "if not duplicate:",
        '@export("prodex_profile_export_active_profile_selected_v1")',
        "profile_import_views_equal(active, selected",
        '@export("prodex_profile_import_active_profile_plan_v1")',
        "if existing.len > 0:",
        "profile_import_views_equal(source, mapping_sources",
        '@export("prodex_profile_export_copilot_strip_json_line_comments_v1")',
        "value == UInt8(47)",
        "var in_string = False",
        '@export("prodex_profile_export_copilot_metadata_v1")',
        "profile_export_copilot_version_part(",
        "profile_export_copilot_platform(",
        'rich_view_matches_literal["windows"]',
        '@export("prodex_profile_export_copilot_url_v1")',
        "profile_export_copilot_user_origin(",
        "profile_export_copilot_models_url(",
        'StringSlice("https://api.githubcopilot.com")',
        'StringSlice("https://copilot-api.")',
        '@export("prodex_profile_export_copilot_import_state_v1")',
        "PROFILE_EXPORT_COPILOT_STATE_UPDATE_EXISTING",
        "PROFILE_EXPORT_COPILOT_STATE_ACCOUNT_CONFLICT",
        "PROFILE_EXPORT_COPILOT_STATE_REQUESTED_EXISTS",
        "has_active_profile == 0 or activate_requested != 0",
      ];
      return required.filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: requested profile selection must remain Mojo-owned (${marker})`);
    }
    if (filePath === "crates/prodex-profile-export/tests/src/lib.rs") {
      const required = [
        "requested_profile_names_default_to_available_names",
        "requested_profile_names_deduplicate_and_preserve_request_order",
        "requested_profile_names_reject_missing_profiles",
        "imported_active_profile_uses_existing_active_profile_first",
        "export_active_profile_only_survives_when_selected",
        "copilot_config_parser_accepts_copilot_jsonc_comments",
        "copilot_url_helpers_match_import_expectations",
      ];
      return required.filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: profile-export selection caller coverage must retain ${marker}`);
    }
    return [];
  });
  const sessionReportViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-session-store/src/report.rs") {
      const violations = [];
      if (!contents.includes("prodex_mojo_core::json::session_report_metadata(")) {
        violations.push(filePath + ": session report metadata must retain Mojo planning");
      }
      for (const restored of [
        '&["payload", "thread_name"]',
        '&["payload", "model"]',
        '&["payload", "cwd"]',
        '&["payload", "model_provider"]',
      ]) {
        if (contents.includes(restored)) {
          violations.push(filePath + ": contains restored Rust fixed session-metadata path precedence");
          break;
        }
      }
      return violations;
    }
    if (filePath === "crates/prodex-session-store/src/session_selector.rs") {
      const violations = [];
      if (!contents.includes("session_value_metadata(")) {
        violations.push(filePath + ": session selector must retain Mojo-backed metadata selection");
      }
      if (contents.includes("first_string_value(") || contents.includes('.get("type")')) {
        violations.push(filePath + ": contains restored Rust session selector metadata semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/json.rs") {
      const required = [
        "prodex_session_report_metadata_v1(",
        "pub fn session_report_metadata(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": session-report ABI adapter must retain " + call);
    }
    return [];
  });
  const routeReasonViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-runtime-proxy/src/route_decision_trace.rs") {
      const required = "prodex_mojo_core::runtime_route_reason::safe_identifier(";
      const violations = contents.includes(required)
        ? [] : [filePath + ": route-decision identifier migration must retain Mojo call " + required];
      if (
        contents.includes("while !value.is_char_boundary(end)")
        || contents.includes("RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES;")
           && contents.includes("value[..end].to_string()")
      ) {
        violations.push(filePath + ": contains restored Rust route-decision identifier semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-runtime-proxy/src/route_decision_trace/reason.rs") {
      const required = [
        "prodex_mojo_core::runtime_route_reason::lookup(",
        "prodex_mojo_core::runtime_route_reason::stage(",
        "prodex_mojo_core::runtime_route_reason::label(",
        "prodex_mojo_core::runtime_route_reason::normalize_unknown(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": route-decision reason migration must retain Mojo call " + call);
      if (
        contents.includes("VALUES.iter().copied().find(|value| value.as_str() == label)")
        || contents.includes("Self::AuthFailureBackoff | Self::AuthNotQuotaCompatible")
        || contents.includes("ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'")
        || contents.includes('"auth_failure_backoff"')
        || contents.includes('"output_limit_clamped"')
      ) {
        violations.push(filePath + ": contains restored Rust route-reason semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/runtime_route_reason.rs") {
      const required = [
        "prodex_runtime_route_reason_lookup_v1(",
        "prodex_runtime_route_reason_stage_v1(",
        "prodex_runtime_route_reason_label_v1(",
        "prodex_runtime_route_reason_unknown_span_v1(",
        "prodex_runtime_route_identifier_span_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": route-reason ABI adapter must retain " + call);
    }
    return [];
  });
  const runtimeLineageViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === LOCAL_REWRITE_UPSTREAM_FILE) {
      const body = contents.match(/pub\(super\) fn\s+candidate_allowed\([^]*?^\}/mu)?.[0];
      const violations = body?.includes("binding_candidate::candidate_allowed(")
        ? []
        : [filePath + ": local rewrite credential admission must delegate to the binding-candidate adapter"];
      if (body?.includes("binding_identity.as_ref() == Some(identity)") || body?.includes("profile_name == RUNTIME_LOCAL_REWRITE_PROFILE")) {
        violations.push(filePath + ": contains restored Rust local rewrite binding-candidate policy");
      }
      return violations;
    }
    if (filePath === LOCAL_REWRITE_BINDING_CANDIDATE_FILE) {
      const body = contents.match(/pub\(super\) fn\s+candidate_allowed\([^]*?^\}/mu)?.[0];
      const violations = body?.includes("runtime_lineage::local_rewrite_candidate_allowed(")
        ? []
        : [filePath + ": local rewrite credential admission must use the Mojo lineage candidate policy"];
      if (body?.includes("binding_identity.as_ref() == Some(identity)") || body?.includes("profile_name == RUNTIME_LOCAL_REWRITE_PROFILE")) {
        violations.push(filePath + ": contains restored Rust local rewrite binding-candidate policy");
      }
      return violations;
    }
    if (filePath === LOCAL_REWRITE_PIPELINE_DISPATCH_FILE) {
      const body = contents.match(/fn\s+runtime_local_rewrite_validate_bound_provider\([^]*?^\}/mu)?.[0];
      const violations = body?.includes("runtime_lineage::dispatch_binding_candidate_decision(")
        ? []
        : [filePath + ": dispatch hard-binding validation must use the Mojo lineage candidate policy"];
      if (body?.includes("bound.provider() != selected_provider") || body?.includes("selected_identity.is_some_and(|selected| selected != bound)")) {
        violations.push(filePath + ": contains restored Rust dispatch binding identity policy");
      }
      return violations;
    }
    if (filePath === "crates/prodex-runtime-state/src/lineage.rs") {
      const required = [
        "prodex_mojo_core::runtime_lineage::component_valid(",
        "prodex_mojo_core::runtime_lineage::response_turn_state_key(",
        "prodex_mojo_core::runtime_lineage::response_turn_state_parts(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime lineage migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const restored of [
        "value.chars().all(|character| !character.is_control())",
        "fn bounded_lineage_key",
        "response_len.parse::<usize>()",
        "suffix.split_once(':')",
      ]) {
        if (production.includes(restored)) {
          violations.push(filePath + ": contains restored Rust runtime-lineage semantics");
          break;
        }
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/runtime_lineage.rs") {
      const required = [
        "prodex_runtime_lineage_classify_v1(",
        "prodex_runtime_lineage_build_v1(",
        "prodex_runtime_lineage_parts_v1(",
        "pub use binding_candidate::{",
        "local_rewrite_candidate_allowed,",
        "dispatch_binding_candidate_decision,",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime lineage ABI adapter must retain " + call);
    }
    if (filePath === LINEAGE_BINDING_CANDIDATE_FILE) {
      const required = [
        "prodex_runtime_lineage_binding_candidate_allowed_v1(",
        "pub fn local_rewrite_candidate_allowed(",
        "pub fn dispatch_binding_candidate_decision(",
        "fn local_rewrite_candidate_requires_exact_profile_and_identity()",
        "fn dispatch_binding_preserves_provider_only_and_exact_identity_rules()",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": binding-candidate ABI adapter must retain " + call);
    }
    return [];
  });
  const smartContextMarkerViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-app/src/runtime_state_shared/semantic_index/markers.rs") {
      const required = [
        "prodex_mojo_core::smart_context_markers::parse_file_location_token(",
        "prodex_mojo_core::smart_context_markers::normalize_diff_file_path_token(",
        "prodex_mojo_core::smart_context_markers::parse_diff_span(",
        "prodex_mojo_core::smart_context_markers::is_test_failure_line(",
        "prodex_mojo_core::smart_context_markers::test_symbol_span(",
        "prodex_mojo_core::smart_context_markers::error_code(",
        "prodex_mojo_core::smart_context_markers::command_line_kind(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": semantic-marker migration must retain Mojo call " + call);
      for (const restored of [
        "fn runtime_smart_context_parse_file_location_token",
        "fn runtime_smart_context_path_looks_like_file",
        "fn runtime_smart_context_parse_diff_span",
        "fn runtime_smart_context_is_test_failure_line",
        "fn runtime_smart_context_parse_test_symbol",
        "fn runtime_smart_context_parse_error_code",
        "fn runtime_smart_context_parse_bracketed_error_code",
      ]) {
        if (contents.includes(restored) && !contents.includes("prodex_mojo_core::smart_context_markers")) {
          violations.push(filePath + ": contains restored Rust smart-context marker semantics");
          break;
        }
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/smart_context_markers.rs") {
      const required = [
        "prodex_smart_context_markers_v1(",
        "pub fn parse_file_location_token(",
        "pub fn normalize_diff_file_path_token(",
        "pub fn parse_diff_span(",
        "pub fn is_test_failure_line(",
        "pub fn test_symbol_span(",
        "pub fn error_code(",
        "pub fn command_line_kind(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": smart-context marker ABI adapter must retain " + call);
    }
    return [];
  });
  const sessionUsageLimitViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === SESSION_USAGE_LIMIT_CONSUMER_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const body = production.match(/\bfn goal_resume_line_has_usage_limit\([^]*?^\}/mu)?.[0];
      const retiredRust = /\b(?:goal_resume_event_payload_usage_limit|goal_resume_structured_usage_limit|goal_resume_structured_object_usage_limit|goal_resume_ignored_structured_object|goal_resume_quota_code|goal_resume_usage_limit_text)\s*\(/u;
      return body?.includes("runtime_session_usage_limit_marker(") && !retiredRust.test(production)
        ? [] : [`${filePath}: session usage-limit detection must use Mojo without a Rust semantic copy`];
    }
    if (filePath === SESSION_USAGE_LIMIT_ADAPTER_FILE) {
      const required = [
        "RUNTIME_ERROR_MODE_SESSION_USAGE_LIMIT: i64 = 15",
        "pub fn runtime_session_usage_limit_marker(",
        "RuntimeUsageLimitInputFormat::Json",
        "RUNTIME_ERROR_SESSION_USAGE_LIMIT_MAX_BYTES",
      ];
      return required.filter((item) => !contents.includes(item))
        .map((item) => `${filePath}: session usage-limit adapter must retain ${item}`);
    }
    if (filePath === SESSION_USAGE_LIMIT_FACADE_FILE) {
      return contents.includes("runtime_session_usage_limit_marker")
        ? [] : [`${filePath}: session usage-limit Mojo adapter must be exported through rich facade`];
    }
    if (filePath === SESSION_USAGE_LIMIT_MOJO_FILE) {
      const required = [
        'comptime RUNTIME_ERROR_MODE_SESSION_USAGE_LIMIT: Int64 = 15',
        "RUNTIME_ERROR_SESSION_USAGE_LIMIT_MAX_NODES: Int64 = 2_048",
        "def runtime_error_session_usage_limit_marker(",
        "def runtime_error_session_usage_json_matches(",
        "def runtime_error_session_usage_queue_children(",
      ];
      return required.filter((item) => !contents.includes(item))
        .map((item) => `${filePath}: session usage-limit semantics must remain Mojo-owned (${item})`);
    }
    return [];
  });
  const smartContextSymbolIndexViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === SMART_CONTEXT_SYMBOLS_CONSUMER_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const violations = production.includes("prodex_mojo_core::smart_context_symbols::index(")
        ? []
        : [`${filePath}: source symbol indexing must use the Mojo planner`];
      if (/\bruntime_smart_context_(?:parse_symbol_line|symbol_range_bounds|symbol_prefix_start|brace_symbol_end|python_symbol_end|parse_js_test_symbol|parse_js_function_symbol)\s*\(/u.test(production)) {
        violations.push(`${filePath}: contains restored Rust source-symbol parsing semantics`);
      }
      if (production.includes("runtime_smart_context_line_excerpt(")) {
        violations.push(`${filePath}: symbol range excerpt validity must remain Mojo-owned`);
      }
      if (FEATURE_OFF_RUST_PATH.test(production)) {
        violations.push(`${filePath}: source symbol indexing cannot have a feature-off Rust path`);
      }
      return violations;
    }
    if (filePath === SMART_CONTEXT_SYMBOLS_RUST_FILE) {
      return contents.trim()
        ? [`${filePath}: replaced Rust source-symbol parser must remain deleted`]
        : [];
    }
    if (filePath === SMART_CONTEXT_SYMBOLS_ADAPTER_FILE) {
      const required = [
        "prodex_smart_context_symbol_index_v1(",
        "pub fn index(",
        "const ABI_VERSION: i64 = 1",
        "const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => `${filePath}: source-symbol adapter must retain ${call}`);
    }
    if (filePath === SMART_CONTEXT_SYMBOLS_MOJO_FILE) {
      const required = [
        '@export("prodex_smart_context_symbol_index_v1")',
        "SYMBOL_ABI_VERSION: Int64 = 1",
        "SYMBOL_MAX_INPUT_BYTES: Int64 = 64 * 1024 * 1024",
        "def symbol_parse_declaration(",
        "def symbol_brace_end(",
        "def symbol_python_end(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => `${filePath}: source-symbol semantics must remain in the versioned Mojo kernel (${call})`);
    }
    if (filePath === "crates/prodex-mojo-core/src/lib.rs") {
      return contents.includes("pub mod smart_context_symbols;")
        ? []
        : [`${filePath}: source-symbol Mojo adapter must be reachable through mojo-runtime`];
    }
    if (filePath === "crates/prodex-mojo-core/build.rs") {
      return contents.includes("../../mojo/prodex_core/smart_context_symbols.mojo")
        ? []
        : [`${filePath}: source-symbol Mojo kernel must be linked into the production archive`];
    }
    if (filePath === SMART_CONTEXT_SYMBOLS_TEST_FILE) {
      const required = [
        "source_symbol_ranges_cover_rust_python_and_javascript",
        "source_symbol_ranges_preserve_function_type_and_fallback_names",
        "symbol_index_reports_capacity_and_excerpt_truncation",
        "symbol_index_accepts_large_multi_line_artifacts_with_bounded_lines",
        "index(text, 16, 16 * 1024)",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => `${filePath}: direct real-Mojo source-symbol coverage must retain ${call}`);
    }
    if (filePath === SMART_CONTEXT_SYMBOLS_CALLER_TEST_FILE) {
      const required = [
        "runtime_smart_context_artifact_symbol_index_uses_mojo_ranges",
        "symbol_ranges",
        "unicode 雪",
        "content_hash",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => `${filePath}: Smart Context artifact caller coverage must retain ${call}`);
    }
    return [];
  });
  const smartContextArtifactRefViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-app/src/runtime_proxy/smart_context/artifact_refs.rs") {
      const required = [
        "prodex_mojo_core::smart_context_artifact_ref::parse_alias_declaration(",
        "prodex_mojo_core::smart_context_artifact_ref::parse_alias_reference(",
        "prodex_mojo_core::smart_context_artifact_ref::parse_reference(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": artifact-ref migration must retain Mojo call " + call);
      for (const restored of [
        "fn runtime_smart_context_split_artifact_alias_ref",
        "fn runtime_smart_context_trim_artifact_ref_token",
        "fn runtime_smart_context_normalize_artifact_ref",
        "fn runtime_smart_context_artifact_prefix_len",
        "fn runtime_smart_context_artifact_id_end",
        "fn runtime_smart_context_parse_line_ranges",
        "fn runtime_smart_context_parse_line_range_segment",
        "fn runtime_smart_context_parse_line_number",
      ]) {
        if (contents.includes(restored)) {
          violations.push(filePath + ": contains restored Rust smart-context artifact-ref semantics");
          break;
        }
      }
      return violations;
    }
    if (filePath === "crates/prodex-app/src/runtime_proxy/smart_context/artifact_manifest.rs") {
      return contents.includes("prodex_mojo_core::smart_context_artifact_ref::artifact_id_valid(")
        ? [] : [filePath + ": artifact ID validation must remain Mojo-authoritative"];
    }
    if (filePath === "crates/prodex-mojo-core/src/smart_context_artifact_ref.rs") {
      const required = [
        "prodex_smart_context_artifact_ref_v1(",
        "pub fn artifact_id_valid(",
        "pub fn parse_alias_declaration(",
        "pub fn parse_alias_reference(",
        "pub fn parse_reference(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": smart-context artifact-ref ABI adapter must retain " + call);
    }
    return [];
  });
  const smartContextDuplicateTextViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === SMART_CONTEXT_DUPLICATE_VALIDATION_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const required = [
        "smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Rewrite)",
        "smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Probe)",
        "smart_context_rewrite_validation_reason(",
      ];
      const violations = required
        .filter((call) => !production.includes(call))
        .map((call) => filePath + ": duplicate-text path must use Mojo planner " + call);
      for (const restored of [
        "fn runtime_smart_context_value_has_duplicate_text",
        "fn runtime_smart_context_dedupe_value_text",
        "BTreeMap::<String, usize>",
        "BTreeMap::<String, Vec<(usize, &str)>>",
        "seen.entry(&hash)",
        "fn runtime_smart_context_rewrite_is_rehydrate_only",
        "fn runtime_smart_context_regression_reason_label",
      ]) {
        if (production.includes(restored)) {
          violations.push(filePath + ": contains restored Rust duplicate-text decision logic " + restored);
          break;
        }
      }
      return violations;
    }
    if (filePath === SMART_CONTEXT_DUPLICATE_ADAPTER_FILE) {
      return [
        "fn prodex_smart_context_duplicate_text_plan_v1(",
        "fn prodex_smart_context_rewrite_validation_reason_v1(",
      ]
        .filter((marker) => !contents.includes(marker))
        .map((marker) => filePath + ": Smart Context adapter must retain " + marker);
    }
    if (filePath === SMART_CONTEXT_DUPLICATE_MOJO_FILE) {
      const required = [
        '@export("prodex_smart_context_duplicate_text_plan_v1")',
        "def smart_context_duplicate_text_plan_kernel(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": duplicate-text decision must remain Mojo-owned " + call);
    }
    if (filePath === SMART_CONTEXT_POLICY_MOJO_FILE) {
      return contents.includes('@export("prodex_smart_context_rewrite_validation_reason_v1")')
        ? []
        : [filePath + ": rewrite-validation reason policy must remain Mojo-owned"];
    }
    return [];
  });
  const runtimeRepoMapViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== "crates/prodex-app/src/runtime_state_shared/line_index.rs") return [];
    const required = [
      "prodex_mojo_core::runtime_repo_map::repo_chunk_plan(",
      "prodex_mojo_core::runtime_repo_map::repo_duplicate_plan(",
      "prodex_mojo_core::runtime_repo_map::repo_path_distance(",
      "prodex_mojo_core::runtime_repo_map::repo_symbol_kind(",
      "prodex_mojo_core::runtime_repo_map::repo_entry_should_replace(",
      "prodex_mojo_core::runtime_repo_map::repo_module_from_path(",
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => `${filePath}: repo-map migration must retain Mojo call ${call}`);
    if (contents.includes("fn runtime_smart_context_repo_map_symbol_is_module_like(")) {
      violations.push(filePath + ": contains restored Rust repo-map symbol classification semantics");
    }
    if (/\bfn\s+runtime_smart_context_repo_map_declaration_keyword\s*\(|trim_start_matches\("a\/"\)|split\(\['\/',/u.test(contents)) {
      violations.push(`${filePath}: contains restored Rust repo-map module/path semantics`);
    }
    return violations;
  });
  const operationalDetailSpecViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === LOG_EVENT_SOURCE_FILE) {
      const violations = contents.includes("operational_event_source_label(")
        ? []
        : [filePath + ": operational event source rendering must retain Mojo-owned source labels"];
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (production.includes("const SOURCES:")) {
        violations.push(filePath + ": contains restored Rust operational event source label table");
      }
      return violations;
    }
    if (filePath === LOG_STREAM_FILE) {
      const violations = contents.includes("operational_detail_spec(")
        ? []
        : [filePath + ": operational log detail rendering must retain Mojo-owned detail metadata"];
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        production.includes("OPERATIONAL_DETAIL_SPECS")
        || production.includes("enum OperationalDetailFormat")
      ) {
        violations.push(filePath + ": contains restored Rust operational detail metadata table");
      }
      return violations;
    }
    if (filePath === OBSERVABILITY_ADAPTER_FILE) {
      const required = [
        "prodex_mojo_operational_detail_spec_v1(",
        "prodex_mojo_operational_event_source_label_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": observability adapter must retain Mojo ABI " + call);
    }
    return [];
  });
  const transcriptPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === LOG_TRANSCRIPT_FILE) {
      const required = [
        "prodex_mojo_core::log::classify_transcript_event(",
        "prodex_mojo_core::log::classify_transcript_item(",
        "prodex_mojo_core::log::transcript_operation_span(",
        "prodex_mojo_core::log::sanitize_transcript_tool_name(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": transcript migration must retain Mojo call " + call);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        /\bfn\s+event_msg_is_status\s*\(/u.test(production)
        || /\bmatch\s+payload\.get\("type"\).*function_call/su.test(production)
        || /\.chars\(\)\.take\(96\)/u.test(production)
        || /\.chars\(\)\.take\(192\)/u.test(production)
      ) {
        violations.push(filePath + ": contains restored Rust transcript classification/bounding semantics");
      }
      return violations;
    }
    if (filePath === LOG_ADAPTER_FILE) {
      const required = [
        "prodex_mojo_transcript_event_classify_v1(",
        "prodex_mojo_transcript_item_classify_v1(",
        "prodex_mojo_transcript_operation_span_v1(",
        "prodex_mojo_transcript_tool_name_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": transcript ABI adapter must retain " + call);
    }
    return [];
  });
  const logLoadPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === LOG_LOAD_APP_FILE) {
      const required = [
        "prodex_mojo_core::log_load::aggregate_update(",
        "prodex_mojo_core::log_load::aggregate_summary(",
        "prodex_mojo_core::log_load::is_routine_event(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => `${filePath}: load aggregation must retain Mojo call ${call}`);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        /matches!\s*\(\s*event_name/u.test(production)
        || /saturating_add\s*\(\s*1\s*\)/u.test(production)
        || /\.iter\(\)\.any\(\|current\|\s*current\s*==\s*&run_id\)/u.test(production)
        || /format!\s*\(\s*"[^"]*×/u.test(production)
        || production.includes("MAX_UNIQUE_RUNS")
      ) {
        violations.push(`${filePath}: contains restored Rust log-load classification, aggregation, or formatting semantics`);
      }
      return violations;
    }
    if (filePath === LOG_LOAD_TUI_FILE) {
      const required = [
        "LogLoadAggregate::plan_observation(",
        "plan.coalesce",
        "aggregate.apply_plan(",
        "LogLoadAggregate::from_plan(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => `${filePath}: TUI caller must apply Mojo aggregate plan ${call}`);
      const production = contents.split("#[cfg(test)]", 1)[0];
      if (
        /aggregate\.key\s*==\s*key/u.test(production)
        || /saturating_duration_since\(aggregate\.last_seen\)/u.test(production)
        || production.includes("LOG_LOAD_COALESCE_WINDOW")
      ) {
        violations.push(`${filePath}: aggregate freshness and key decisions must use Mojo`);
      }
      return violations;
    }
    if (filePath === LOG_LOAD_ADAPTER_FILE) {
      return contents.includes("prodex_mojo_log_load_semantics_v1(")
        ? []
        : [`${filePath}: load aggregate adapter must retain the required Mojo ABI call`];
    }
    return [];
  });
  const deepseekCatalogPolicyViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === "crates/prodex-app/src/runtime_deepseek_config.rs") {
      const required = [
        "deepseek_catalog_static_models(",
        "mojo_deepseek_catalog_model_metadata(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": DeepSeek static catalog hard replacement must retain " + call);
      if (
        contents.includes("DEEPSEEK_CATALOG_MODELS")
        || contents.includes("CatalogModel {")
        || contents.includes("resolve_catalog_model(&catalog, model)")
      ) {
        violations.push(filePath + ": contains restored Rust DeepSeek static-catalog semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/super_provider_config.rs") {
      const required = [
        "prodex_deepseek_catalog_model_count_v1(",
        "prodex_deepseek_catalog_model_at_v1(",
        "prodex_deepseek_catalog_model_find_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": DeepSeek static-catalog ABI adapter must retain " + call);
    }
    return [];
  });
  const replacedClassifierViolations = files.flatMap(([filePath, contents]) => {
    const forbidden = new Map([
      ["crates/prodex-domain/src/governance/inspection.rs", /\bfn\s+minimum_classification_rust\s*\(|\.any\(\|finding\|\s*classification\s*<\s*finding\.kind\.minimum_classification\(\)\)/u],
      ["crates/prodex-runtime-quota/src/selection/scoring.rs", /\bfn\s+(?:ready_profile_score_for_route_at_rust|runtime_quota_pressure_band_for_route_at_rust|schedule_ready_profile_candidates_rust)\s*\(/u],
      ["crates/prodex-runtime-quota/src/selection/scoring/profile_order.rs", /\bfn\s+provider_aware_profile_order_rust\s*\(/u],
      [SUPER_OVERRIDE_FILE, /\bfn\s+(?:scan_override_rust|scan_identity_override|scan_boolean_override|scan_runtime_override|scan_feature_value_override|scan_feature_boolean_override)\s*\(/u],
      [GEMINI_SCHEMA_FILE, /\bfn\s+(?:schema_type|supported_schema_type|sanitized_enum|sanitized_properties|sanitized_required|sanitize_schema)\s*\(/u],
      [GEMINI_TOOLS_FILE, /\bfn\s+(?:gemini_tool_config_from_request_oracle|gemini_tool_from_openai_tool)\s*\(/u],
      ["crates/prodex-provider-core/src/gemini_bridge/request/native_project.rs", /\bfn\s+gemini_provider_core_stamp_native_(?:project|metadata_project)\s*\(/u],
      ["crates/prodex-provider-core/src/gemini_bridge/request/simple.rs", /\bfn\s+gemini_simple_(?:input_item|content_item|tool_calls|tool_call)\s*\(/u],
      ["crates/prodex-provider-core/src/translators/gemini/request.rs", /\bfn\s+gemini_request_object_mut\s*\(/u],
      ["crates/prodex-provider-core/src/translators/gemini/request/tools/builtin.rs", /\bfn\s+gemini_(?:computer_use_tool|is_computer_use_tool|is_code_execution_tool|is_web_search_tool|is_url_context_tool|builtin_tool_value)\s*\(/u],
      [GEMINI_STATUS_FILE, /\bfn\s+gemini_(?:finish_reason_(?:failure|incomplete)|prompt_feedback_failure)_oracle\s*\(/u],
      [RESPONSE_FORWARDING_FILE, /\bfn\s+(?:should_skip_response_header|response_content_type_is_sse|token_usage_event_is_loggable|response_event_is_generation_start)\s*\(/u],
      [QUOTA_POOL_FILE, /\bfn\s+(?:aggregate_openai_quota|aggregate_main_quota|add_pool_window|add_ready_pool_window)\s*\(/u],
      [STATUS_SUMMARY_FILE, /\bfn\s+(?:status_quota_from_reports_rust_oracle|add_quota_window)\s*\(/u],
      [QUOTA_MODEL_CAPACITY_FILE, /\bfn\s+(?:normalized_identifier|is_luna_reserve_identifier|openai_usage_advertises_luna_reserve)\s*\(/u],
      [RUNTIME_QUOTA_FILE, /\bfn\s+runtime_proxy_quota_score_for_route_rust\s*\(/u],
      [HEALTH_ABI_TEST_FILE, /\bfn\s+(?:effective|expected)\s*\(/u],
      [DEEPSEEK_RESPONSE_FILE, /\bfn\s+deepseek_stream_event_from_chat_value_rust\s*\(|#\[cfg\(not\(feature\s*=\s*"mojo"\)\)\]\s*pub\(super\)\s+fn\s+deepseek_stream_event_from_chat_value\s*\(/u],
      [DEEPSEEK_REQUEST_FILE, /\bfn\s+(?:deepseek_request_body_from_responses_rust|deepseek_messages_from_request|deepseek_tool_choice_from_request)\s*\(/u],
      [DEEPSEEK_SIMPLE_REQUEST_FILE, /\bfn\s+(?:deepseek_provider_core_(?:function_tools|response_format|tool_choice)|deepseek_simple_input_item)\s*\(/u],
      [MODEL_SPEC_FILE, /\beq_ignore_ascii_case\s*\(/u],
      [PROMPT_CACHE_SELECTION_FILE, /selection_prompt_cache_rust|\bfn\s+runtime_prompt_cache_affinity_score\s*\(/u],
      [GEMINI_TOOL_CALLS_FILE, /\bfn\s+gemini_split_flat_namespace_tool_name\s*\(/u],
      [GEMINI_CHAT_TOOL_CALLS_FILE, /\blet\s+mut\s+item\s*=\s*json!\s*\(/u],
      ["crates/prodex-runtime-proxy/src/smart_context/normalization/static_context.rs", /\bfn\s+smart_context_(?:static_context_(?:item_order|item_order_key|order_key|noise_line_rust|noise_key|noise_key_is_volatile|noise_value_looks_volatile)|input_static_context_order_key)\s*\(/u],
    ]);
    return forbidden.get(filePath)?.test(contents)
      ? [`${filePath}: contains a replaced Rust semantic implementation`] : [];
  });
  const cliDependencyViolations = files
    .filter(([filePath, contents]) => filePath === "crates/prodex-cli/Cargo.toml" &&
      !/^prodex_mojo_core\s*=\s*\{[^\n]*features\s*=\s*\["mojo-runtime"\][^\n]*\}/mu.test(contents))
    .map(([filePath]) => `${filePath}: Super override scanning requires Mojo without a feature gate`);
  const doctorDependencyViolations = files
    .filter(([filePath, contents]) => filePath === DOCTOR_CARGO_FILE &&
      !/^prodex_mojo_core\s*=\s*\{[^\n]*features\s*=\s*\[[^\]]*"mojo-rich"[^\]]*\][^\n]*\}/mu.test(contents))
    .map(([filePath]) => `${filePath}: doctor next steps and suggestions require Mojo without a feature gate`);
  const proxyDependencyViolations = files
    .filter(([filePath, contents]) => filePath === RUNTIME_PROXY_CARGO_FILE &&
      !/^prodex_mojo_core\s*=\s*\{[^\n]*features\s*=\s*\[[^\]]*"mojo-rich"[^\]]*\][^\n]*\}/mu.test(contents))
    .map(([filePath]) => `${filePath}: Retry-After parsing requires Mojo rich without a feature gate`);
  const runtimeTuningViolations = files.flatMap(([filePath, contents]) => {
    if ([
      "crates/prodex-runtime-tuning/src/lib.rs",
      "crates/prodex-runtime-tuning/src/capacity.rs",
      "crates/prodex-runtime-tuning/src/mojo.rs",
    ].includes(filePath)) {
      return /#\[cfg\(feature\s*=\s*"mojo"\)\]/u.test(contents)
        ? [filePath + ": runtime tuning Mojo ownership must be unconditional"]
        : [];
    }
    if (filePath !== RUNTIME_TUNING_CARGO_FILE) return [];
    const dependency = contents.match(/^prodex_mojo_core\s*=.*$/mu)?.[0] ?? "";
    const violations = [];
    if (!dependency.includes('features = ["mojo-runtime"]')) {
      violations.push(filePath + ": runtime tuning requires unconditional Mojo runtime dependency");
    }
    if (dependency.includes("optional = true")) {
      violations.push(filePath + ": runtime tuning Mojo dependency must not be optional");
    }
    return violations;
  });
  const defaultFeatureViolations = files.flatMap(([filePath, contents]) => {
    const required = REQUIRED_DEFAULT_FEATURES.get(filePath);
    if (!required) return [];
    const defaults = contents.match(/^default\s*=\s*\[([^\]]*)\]/mu)?.[1];
    return defaults?.match(/"[^"]+"/gu)?.includes(`"${required}"`)
      ? [] : [`${filePath}: default features must include ${required}`];
  });
  const geminiCompactSnippetViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === GEMINI_COMPACT_SNIPPET_CONSUMER_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const required = ["format_gemini_compact_snippet(", "GeminiCompactSnippetInput {"];
      const restoredRust = ["gemini_provider_core_local_compact_tool_snippet(", "gemini_provider_core_truncate_utf8("];
      return required.every((marker) => production.includes(marker)) && restoredRust.every((marker) => !production.includes(marker))
        ? [] : [`${filePath}: Gemini compact snippet shaping must use Mojo without a Rust semantic copy`];
    }
    if (filePath === GEMINI_COMPACT_TEXT_CONSUMER_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const required = [
        "truncate_gemini_compact_utf8_edges(&text, max_bytes)",
      ];
      const restoredRust = [
        "text.is_char_boundary(",
        'const SUFFIX: &str = "\n[truncated]"',
        'const SEPARATOR: &str = "\n[... middle truncated ...]\n"',
      ];
      return required.every((marker) => production.includes(marker)) && restoredRust.every((marker) => !production.includes(marker))
        ? [] : [`${filePath}: Gemini compact UTF-8 truncation must use Mojo without a Rust semantic copy`];
    }
    if (filePath === GEMINI_COMPACT_SUMMARY_CONSUMER_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const required = ["format_gemini_local_compact_summary(", "gemini_provider_core_local_compact_snippet"];
      const restoredRust = [
        "GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPETS",
        "summary.push_str(",
        "snippets.split_off(",
        "snippet.replace(",
      ];
      return required.every((marker) => production.includes(marker)) && restoredRust.every((marker) => !production.includes(marker))
        ? [] : [`${filePath}: Gemini local compact summary formatting must use Mojo without a Rust semantic copy`];
    }
    if (filePath === GEMINI_COMPACT_SEMANTIC_CONSUMER_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const required = [
        "format_gemini_semantic_continuation_summary(",
        "gemini_provider_core_truncate_utf8_edges(",
        "plan_gemini_semantic_compact_indices(",
      ];
      const restoredRust = [
        "let mut summary = String::new()",
        "summary.push_str(",
        ".filter(|text| !text.trim().is_empty())",
        ".rposition(|item|",
        ".iter().rev().find(|item|",
      ];
      return required.every((marker) => production.includes(marker)) && restoredRust.every((marker) => !production.includes(marker))
        ? [] : [`${filePath}: Gemini semantic continuation formatting must use Mojo without a Rust semantic copy`];
    }
    if (filePath === GEMINI_COMPACT_SNIPPET_ADAPTER_FILE) {
      const required = [
        "prodex_mojo_gemini_compact_snippet_v1(",
        "pub fn format_gemini_compact_snippet(",
        "gemini_compact_snippet_formatting_is_mojo_owned",
        "prodex_mojo_gemini_compact_truncate_v1(",
        "pub fn truncate_gemini_compact_utf8(",
        "pub fn truncate_gemini_compact_utf8_edges(",
        "gemini_compact_utf8_truncation_is_mojo_owned",
        "prodex_mojo_gemini_compact_local_summary_v1(",
        "pub fn format_gemini_local_compact_summary(",
        "gemini_local_compact_summary_formatting_is_mojo_owned",
        "prodex_mojo_gemini_compact_semantic_summary_v1(",
        "pub fn format_gemini_semantic_continuation_summary(",
        "gemini_semantic_continuation_summary_formatting_is_mojo_owned",
        "prodex_mojo_gemini_compact_semantic_indices_v1(",
        "pub fn plan_gemini_semantic_compact_indices(",
        "gemini_semantic_compact_indices_are_mojo_owned",
      ];
      return required.filter((marker) => !contents.includes(marker)).map((marker) => `${filePath}: Gemini compact adapter must retain ${marker}`);
    }
    if (filePath === GEMINI_COMPACT_SNIPPET_MOJO_FILE) {
      const required = [
        '@export("prodex_mojo_gemini_compact_snippet_v1")',
        'rich_view_matches_literal["message"]',
        'rich_view_matches_literal["function_call"]',
        'rich_view_matches_literal["reasoning"]',
        'StringSlice("\\n[truncated]")',
        '@export("prodex_mojo_gemini_compact_truncate_v1")',
        'StringSlice("\\n[... middle truncated ...]\\n")',
        "gemini_compact_truncate_tail(",
        "gemini_compact_truncate_edges(",
        '@export("prodex_mojo_gemini_compact_local_summary_v1")',
        "GEMINI_COMPACT_LOCAL_MAX_SNIPPETS",
        'StringSlice("Local Prodex compact fallback summary.',
        'No parseable recent message or tool content was found.',
        '("prodex_mojo_gemini_compact_semantic_indices_v1")',
        'rich_view_matches_literal["user"]',
        'rich_view_matches_literal["function_call_output"]',
        'rich_view_matches_literal["custom_tool_call_output"]',
        'rich_view_matches_literal["local_shell_call_output"]',
      ];
      return required.filter((marker) => !contents.includes(marker)).map((marker) => `${filePath}: Gemini compact semantics must remain Mojo-owned (${marker})`);
    }
    return [];
  });
  const operationalHistogramViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === OPERATIONAL_HISTOGRAM_CALLER_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const required = [
        "histogram_bucket_bounds(name)",
        "prodex_mojo_core::operational_metrics::observe_histogram(",
      ];
      const restoredRust = /histogram\.(?:count|sum)\s*=|bucket_counts\.iter_mut\(\)|observation\s*<=\s*\*bound/u;
      return required.every((marker) => production.includes(marker)) &&
        !/\bruntime_operational_histogram_bounds\s*\(/u.test(production) &&
        !/\b120_000_000\b/u.test(production) &&
        !restoredRust.test(production)
        ? []
        : [`${filePath}: histogram bucket planning and observation must use Mojo without a Rust policy copy`];
    }
    if (filePath === OPERATIONAL_HISTOGRAM_ADAPTER_FILE) {
      const required = [
        "prodex_mojo_operational_histogram_bounds_v1(",
        "prodex_mojo_operational_histogram_observe_v1(",
        "pub fn observe_histogram(",
      ];
      return required.filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: histogram adapter must retain ${marker}`);
    }
    if (filePath === OPERATIONAL_HISTOGRAM_MOJO_FILE) {
      const required = [
        '@export("prodex_mojo_operational_histogram_bounds_v1")',
        '@export("prodex_mojo_operational_histogram_observe_v1")',
        "operational_histogram_saturating_add(",
        "observation <= bounds[index]",
      ];
      return required.filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: histogram semantics must remain Mojo-owned (${marker})`);
    }
    if (filePath === OPERATIONAL_HISTOGRAM_ABI_TEST_FILE) {
      const required = [
        "operational_histogram_bucket_plan_is_mojo_owned",
        "operational_histogram_observation_is_mojo_owned",
        "operational_histogram_observation_saturates_and_rejects_shape_mismatch",
      ];
      return required.filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: direct required-Mojo histogram coverage must retain ${marker}`);
    }
    return [];
  });
  const doctorSmartContextDecisionViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === DOCTOR_SMART_CONTEXT_DECISION_CONSUMER_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      const required = [
        "prodex_mojo_core::rich::runtime_doctor_smart_context_decision_is_fallback(",
        "prodex_mojo_core::rich::runtime_doctor_smart_context_fallback_reason_source(",
      ];
      const restoredRust = [
        "!matches!(decision,",
        'decision == "pass_through"',
        "!event.reasons.is_empty()",
        'decision == "self_check_passthrough"',
      ];
      return required.every((marker) => production.includes(marker)) &&
        restoredRust.every((marker) => !production.includes(marker))
        ? []
        : [`${filePath}: Smart Context fallback decision and reason-source policy must use Mojo without a Rust policy copy`];
    }
    if (filePath === DOCTOR_LOG_FIELDS_CONSUMER_FILE) {
      const production = contents.split("#[cfg(test)]", 1)[0];
      return production.includes("prodex_mojo_core::rich::runtime_doctor_log_value_is_ignored(value)") &&
        !production.includes('value.is_empty() || value == "-"')
        ? []
        : [`${filePath}: ignored runtime-doctor log-value policy must remain Mojo-owned`];
    }
    if (filePath === DOCTOR_SMART_CONTEXT_DECISION_ADAPTER_FILE) {
      const required = [
        "prodex_mojo_runtime_doctor_smart_context_decision_is_fallback_v1(",
        "runtime_doctor_smart_context_decision_is_fallback(",
        "runtime_doctor_smart_context_decision_fallback_classification_is_mojo_owned",
        "prodex_mojo_runtime_doctor_log_value_is_ignored_v1(",
        "runtime_doctor_log_value_is_ignored(",
        "runtime_doctor_log_value_filter_is_mojo_owned",
        "prodex_mojo_runtime_doctor_smart_context_fallback_reason_source_v1(",
        "runtime_doctor_smart_context_fallback_reason_source(",
        "runtime_doctor_smart_context_fallback_reason_source_is_mojo_owned",
      ];
      return required.filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Smart Context diagnostic adapter must retain ${marker}`);
    }
    if (filePath === DOCTOR_SMART_CONTEXT_DECISION_MOJO_FILE) {
      const required = [
        '@export("prodex_mojo_runtime_doctor_smart_context_decision_is_fallback_v1")',
        'rich_view_matches_literal["rewritten"]',
        'rich_view_matches_literal["pass_through"]',
        '@export("prodex_mojo_runtime_doctor_log_value_is_ignored_v1")',
        'rich_view_matches_literal["-"]',
        '@export("prodex_mojo_runtime_doctor_smart_context_fallback_reason_source_v1")',
        'rich_view_matches_literal["self_check_passthrough"]',
        "reason_count > 0",
      ];
      return required.filter((marker) => !contents.includes(marker))
        .map((marker) => `${filePath}: Smart Context diagnostic policy must remain Mojo-owned (${marker})`);
    }
    if (filePath === DOCTOR_SMART_CONTEXT_CALLER_TEST_FILE &&
        !contents.includes("runtime_doctor_smart_context_fallback_reason_source_is_mojo_owned_at_summary_boundary")) {
      return [`${filePath}: production fallback-reason source coverage is required`];
    }
    return [];
  });
  return [...geminiCompactSnippetViolations, ...doctorSmartContextDecisionViolations, ...smartContextCapsuleOrderViolations, ...markerViolations, ...deepseekCatalogPolicyViolations, ...featureOffViolations, ...liveLogRecordViolations, ...runtimePolicyPresetViolations, ...profileHealthCircuitViolations, ...logThroughputViolations, ...operationalDetailSpecViolations, ...transcriptPolicyViolations, ...logLoadPolicyViolations, ...routeReasonViolations, ...runtimeStateQuotaViolations, ...runtimeProxyRootViolations, ...brokerVersionGuardViolations, ...brokerContinuityViolations, ...brokerLogCacheViolations, ...codexConfigViolations, ...statePolicyViolations, ...quotaSelectionPolicyViolations, ...appSelectionPolicyMirrorViolations, ...runtimeStateBackgroundViolations, ...redactionViolations, ...profileIdentityViolations, ...governanceInspectionViolations, ...governanceInspectionOrderingViolations, ...exactnessPlannerViolations,
    ...adaptiveBudgetViolations,
    ...providerUsageViolations,
    ...auditUsageViolations,
    ...coreFilePolicyViolations,
    ...mcpStdioPolicyViolations,
    ...sharedHistoryPolicyViolations,
    ...runtimeCookiePolicyViolations,
    ...updateNoticeMigrationViolations,
    ...sharedAttachmentMigrationViolations,
    ...runtimeCookieMigrationViolations,
    ...deepseekSimpleRequestViolations,
    ...deepseekMetadataViolations,
    ...kiroChatResponseViolations,
    ...kiroFinalStreamViolations,
    ...kiroStreamPlanViolations,
    ...kiroResponseHelperViolations,
    ...kiroAcpViolations,
    ...kiroMessageShapeViolations,
    ...kiroCatalogViolations,
    ...deepseekStrictSchemaViolations,
    ...quotaModelPolicyViolations, ...quotaDisplayPolicyViolations, ...quotaPlannerViolations,
    ...anthropicResponseViolations,
    ...anthropicEnvelopeViolations, ...anthropicRequestViolations,
    ...anthropicWebSearchViolations, ...superProviderConfigViolations, ...externalProviderCatalogViolations, ...subAgentPolicyViolations, ...runtimeOverlayPolicyViolations, ...cliRuntimeFeatureViolations,
    ...superExposeProtocolViolations, ...superExposeViolations,
    ...geminiFallbackViolations, ...geminiGenerationViolations, ...geminiTranslatorHardReplacementViolations, ...geminiBridgeFallbackViolations,
    ...geminiToolResponseOrderViolations,
    ...hardReplacementViolations, ...precommitBudgetOracleViolations,
    ...deepseekRequestViolations, ...deepseekRequestRejectViolations,
    ...deepseekReasoningViolations,
    ...nativeFirstErrorClassViolations, ...providerBridgeMetadataViolations, ...websocketProxyPolicyViolations, ...transportFailurePolicyViolations, ...providerPrecommitPolicyViolations, ...providerErrorMemberViolations,
    ...deepseekResponseToolCallViolations, ...chatToolViolations,
    ...previousResponseOutcomeLabelViolations, ...affinityChainLogRenderViolations, ...previousResponseLogRenderViolations, ...structuredLogPolicyViolations, ...candidateSkipReasonViolations, ...runtimeProxyObservabilityLabelViolations, ...websocketExecutorLabelViolations, ...infoRenderViolations, ...doctorMarkerViolations, ...doctorFailureClassViolations, ...doctorMarkerSummaryCountsViolations, ...doctorCompactExitCountsViolations, ...doctorTimelineDetailViolations, ...doctorLastMarkerLineViolations, ...runtimeDoctorPlanInputViolations, ...cliDefaultRunViolations, ...responseMetadataViolations, ...doctorMarkerAbiViolations, ...statusSummaryViolations,
    ...geminiBufferedResponseViolations, ...fingerprintDeltaViolations, ...profileExportPolicyViolations, ...sessionReportViolations, ...runtimeLineageViolations, ...smartContextMarkerViolations, ...smartContextSymbolIndexViolations, ...sessionUsageLimitViolations, ...smartContextArtifactRefViolations, ...smartContextDuplicateTextViolations, ...runtimeRepoMapViolations,
    ...modelSpecViolations, ...catalogModelViolations,
    ...deepseekShapingViolations,
    ...deepseekStreamFallbackViolations,
    ...quotaWindowViolations, ...quotaResetEpochViolations,
    ...rehydrateViolations, ...budgetTierViolations, ...staticItemViolations, ...replacedClassifierViolations, ...cliDependencyViolations,
    ...doctorDependencyViolations, ...proxyDependencyViolations, ...runtimeTuningViolations,
    ...defaultFeatureViolations, ...operationalHistogramViolations];
}

async function promotedFiles() {
  const files = await Promise.all(
    PROMOTED_FILES.map(async (filePath) => [
      filePath,
      await fs.readFile(path.join(repoRoot, filePath), "utf8"),
    ]),
  );
  for (const filePath of REMOVED_ORACLE_FILES) {
    try {
      files.push([filePath, await fs.readFile(path.join(repoRoot, filePath), "utf8")]);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
  }
  files.push(...await Promise.all(
    [...REQUIRED_DEFAULT_FEATURES.keys()].map(async (filePath) => [
      filePath,
      await fs.readFile(path.join(repoRoot, filePath), "utf8"),
    ]),
  ));
  files.push(["crates/prodex-cli/Cargo.toml", await fs.readFile(path.join(repoRoot, "crates/prodex-cli/Cargo.toml"), "utf8")]);
  files.push([RUNTIME_PROXY_CARGO_FILE, await fs.readFile(path.join(repoRoot, RUNTIME_PROXY_CARGO_FILE), "utf8")]);
  files.push([RUNTIME_TUNING_CARGO_FILE, await fs.readFile(path.join(repoRoot, RUNTIME_TUNING_CARGO_FILE), "utf8")]);
  return files;
}

function selfTest() {
  assert.deepEqual(findViolations([["x.rs", "fn main() {}"]]), []);
  assert.equal(findViolations([["x.rs", "prodex_mojo_fallback();"]]).length, 1);
  const geminiCompactConsumer = "GeminiCompactSnippetInput {\nformat_gemini_compact_snippet(";
  assert.deepEqual(findViolations([[GEMINI_COMPACT_SNIPPET_CONSUMER_FILE, geminiCompactConsumer]]), []);
  assert.match(findViolations([[GEMINI_COMPACT_SNIPPET_CONSUMER_FILE, geminiCompactConsumer + "\ngemini_provider_core_truncate_utf8(text, 768);"]]).join("\n"), /Gemini compact snippet shaping must use Mojo/u);
  const geminiCompactTextConsumer = "truncate_gemini_compact_utf8_edges(&text, max_bytes)";
  assert.deepEqual(findViolations([[GEMINI_COMPACT_TEXT_CONSUMER_FILE, geminiCompactTextConsumer]]), []);
  assert.match(findViolations([[GEMINI_COMPACT_TEXT_CONSUMER_FILE, geminiCompactTextConsumer + "\ntext.is_char_boundary(end);"]]).join("\n"), /Gemini compact UTF-8 truncation must use Mojo/u);
  const geminiCompactSummaryConsumer = [
    "gemini_provider_core_local_compact_snippet",
    "format_gemini_local_compact_summary(",
  ].join("\n");
  assert.deepEqual(findViolations([[GEMINI_COMPACT_SUMMARY_CONSUMER_FILE, geminiCompactSummaryConsumer]]), []);
  assert.match(findViolations([[GEMINI_COMPACT_SUMMARY_CONSUMER_FILE, geminiCompactSummaryConsumer + "\nsummary.push_str(\"restored\");"]]).join("\n"), /Gemini local compact summary formatting must use Mojo/u);
  const geminiCompactSemanticConsumer = [
    "gemini_provider_core_truncate_utf8_edges(",
    "format_gemini_semantic_continuation_summary(",
  ].join("\n");
  assert.deepEqual(findViolations([[GEMINI_COMPACT_SEMANTIC_CONSUMER_FILE, geminiCompactSemanticConsumer]]), []);
  assert.match(findViolations([[GEMINI_COMPACT_SEMANTIC_CONSUMER_FILE, geminiCompactSemanticConsumer + "\nsummary.push_str(\"restored\");"]]).join("\n"), /Gemini semantic continuation formatting must use Mojo/u);
  const profileExportSelectionCaller = [
    "prodex_mojo_core::profile_export::profile_export_selection_plan(&available, &requested)",
    "prodex_mojo_core::profile_export::profile_export_active_profile_selected(",
    "prodex_mojo_core::profile_export::profile_import_active_profile_plan(",
  ].join("\n");
  assert.deepEqual(findViolations([[
    "crates/prodex-profile-export/src/selection.rs", profileExportSelectionCaller,
  ]]), []);
  assert.match(findViolations([[
    "crates/prodex-profile-export/src/selection.rs",
    profileExportSelectionCaller + "\nif available_names.is_empty() {}",
  ]]).join("\n"), /profile selection and active-profile resolution must use Mojo/u);
  assert.match(findViolations([[
    "crates/prodex-profile-export/src/selection.rs",
    profileExportSelectionCaller + "\nselected.iter().any(|name| name == active_profile);",
  ]]).join("\n"), /profile selection and active-profile resolution must use Mojo/u);
  const profileCopilotJsoncCaller = [
    "prodex_mojo_core::profile_export::strip_copilot_json_line_comments(raw)",
    "prodex_mojo_core::profile_export::copilot_version_triplet(raw)",
    "prodex_mojo_core::profile_export::copilot_platform_label(os, arch)",
    "prodex_mojo_core::profile_export::copilot_user_api_origin(host)",
    "prodex_mojo_core::profile_export::copilot_models_api_url(host)",
    "prodex_mojo_core::profile_export::copilot_import_state_plan(",
  ].join("\n");
  assert.deepEqual(findViolations([[
    "crates/prodex-profile-export/src/copilot.rs", profileCopilotJsoncCaller,
  ]]), []);
  assert.match(findViolations([[
    "crates/prodex-profile-export/src/copilot.rs",
    profileCopilotJsoncCaller + "\nfn strip_json_line_comments(raw: &str) {}",
  ]]).join("\n"), /JSONC\/version\/platform policy must use Mojo/u);
  assert.match(findViolations([[
    "crates/prodex-profile-export/src/copilot.rs",
    profileCopilotJsoncCaller + "\nlet mut parts = raw.split('.');",
  ]]).join("\n"), /JSONC\/version\/platform policy must use Mojo/u);
  assert.match(findViolations([[
    "crates/prodex-profile-export/src/copilot.rs",
    profileCopilotJsoncCaller + "\nmatch (os, arch) { _ => {} }",
  ]]).join("\n"), /JSONC\/version\/platform policy must use Mojo/u);
  const copilotPolicyCaller = [
    "prodex_mojo_core::profile_export::strip_copilot_json_line_comments(raw)",
    "prodex_mojo_core::profile_export::copilot_version_triplet(raw)",
    "prodex_mojo_core::profile_export::copilot_platform_label(os, arch)",
    "prodex_mojo_core::profile_export::copilot_user_api_origin(host)",
    "prodex_mojo_core::profile_export::copilot_models_api_url(host)",
    "prodex_mojo_core::profile_export::copilot_import_state_plan(",
  ].join("\n");
  assert.deepEqual(findViolations([[
    "crates/prodex-profile-export/src/copilot.rs", copilotPolicyCaller,
  ]]), []);
  assert.match(findViolations([[
    "crates/prodex-profile-export/src/copilot.rs",
    copilotPolicyCaller + "\nfn authority_host_and_port(authority: &str) {}",
  ]]).join("\n"), /Copilot JSONC\/version\/platform policy must use Mojo/u);
  assert.match(findViolations([[
    "crates/prodex-profile-export/src/copilot.rs",
    copilotPolicyCaller + "\nlet activate = !has_active_profile || activate_requested;",
  ]]).join("\n"), /Copilot JSONC\/version\/platform policy must use Mojo/u);
  const operationalHistogramCaller = [
    "histogram_bucket_bounds(name)",
    "prodex_mojo_core::operational_metrics::observe_histogram(",
  ].join("\n");
  const operationalHistogramFiles = [
    [OPERATIONAL_HISTOGRAM_CALLER_FILE, operationalHistogramCaller],
    [OPERATIONAL_HISTOGRAM_ADAPTER_FILE, [
      "prodex_mojo_operational_histogram_bounds_v1(",
      "prodex_mojo_operational_histogram_observe_v1(",
      "pub fn observe_histogram(",
    ].join("\n")],
    [OPERATIONAL_HISTOGRAM_MOJO_FILE, [
      '@export("prodex_mojo_operational_histogram_bounds_v1")',
      '@export("prodex_mojo_operational_histogram_observe_v1")',
      "operational_histogram_saturating_add(",
      "observation <= bounds[index]",
    ].join("\n")],
    [OPERATIONAL_HISTOGRAM_ABI_TEST_FILE, [
      "operational_histogram_bucket_plan_is_mojo_owned",
      "operational_histogram_observation_is_mojo_owned",
      "operational_histogram_observation_saturates_and_rejects_shape_mismatch",
    ].join("\n")],
  ];
  assert.deepEqual(findViolations(operationalHistogramFiles), []);
  assert.match(findViolations([[OPERATIONAL_HISTOGRAM_CALLER_FILE,
    operationalHistogramCaller + "\nhistogram.count = histogram.count.saturating_add(1);",
  ]]).join("\n"), /histogram bucket planning and observation must use Mojo/u);
  const doctorSmartContextConsumer = [
    "prodex_mojo_core::rich::runtime_doctor_smart_context_decision_is_fallback(",
    "prodex_mojo_core::rich::runtime_doctor_smart_context_fallback_reason_source(",
  ].join("\n");
  const doctorLogFieldsConsumer =
    "prodex_mojo_core::rich::runtime_doctor_log_value_is_ignored(value)";
  assert.deepEqual(findViolations([
    [DOCTOR_SMART_CONTEXT_DECISION_CONSUMER_FILE, doctorSmartContextConsumer],
    [DOCTOR_LOG_FIELDS_CONSUMER_FILE, doctorLogFieldsConsumer],
  ]), []);
  assert.match(findViolations([[DOCTOR_SMART_CONTEXT_DECISION_CONSUMER_FILE,
    doctorSmartContextConsumer + "\nif !event.reasons.is_empty() {}",
  ]]).join("\n"), /fallback decision and reason-source policy must use Mojo/u);
  assert.match(findViolations([[DOCTOR_LOG_FIELDS_CONSUMER_FILE,
    doctorLogFieldsConsumer + '\nvalue.is_empty() || value == "-"',
  ]]).join("\n"), /ignored runtime-doctor log-value policy must remain Mojo-owned/u);
  const smartContextSymbolConsumer = [
    "fn runtime_smart_context_artifact_semantic_line_index() {",
    "  prodex_mojo_core::smart_context_symbols::index(text, remaining, max_excerpt_bytes);",
    "}",
  ].join("\n");
  const smartContextSymbolFiles = [
    [SMART_CONTEXT_SYMBOLS_CONSUMER_FILE, smartContextSymbolConsumer],
    [SMART_CONTEXT_SYMBOLS_ADAPTER_FILE,
      "const ABI_VERSION: i64 = 1; const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024; fn prodex_smart_context_symbol_index_v1() {} pub fn index() {}"],
    [SMART_CONTEXT_SYMBOLS_MOJO_FILE,
      '@export("prodex_smart_context_symbol_index_v1")\ncomptime SYMBOL_ABI_VERSION: Int64 = 1\ncomptime SYMBOL_MAX_INPUT_BYTES: Int64 = 64 * 1024 * 1024\ndef symbol_parse_declaration(): pass\ndef symbol_brace_end(): pass\ndef symbol_python_end(): pass'],
    ["crates/prodex-mojo-core/src/lib.rs", "pub mod smart_context_symbols;"],
    ["crates/prodex-mojo-core/build.rs", "../../mojo/prodex_core/smart_context_symbols.mojo"],
    [SMART_CONTEXT_SYMBOLS_TEST_FILE,
      "source_symbol_ranges_cover_rust_python_and_javascript source_symbol_ranges_preserve_function_type_and_fallback_names symbol_index_reports_capacity_and_excerpt_truncation symbol_index_accepts_large_multi_line_artifacts_with_bounded_lines index(text, 16, 16 * 1024)"],
    [SMART_CONTEXT_SYMBOLS_CALLER_TEST_FILE,
      "runtime_smart_context_artifact_symbol_index_uses_mojo_ranges symbol_ranges unicode 雪 content_hash"],
  ];
  assert.deepEqual(findViolations(smartContextSymbolFiles), []);
  assert.match(findViolations([[SMART_CONTEXT_SYMBOLS_CONSUMER_FILE,
    smartContextSymbolConsumer.replace("prodex_mojo_core::smart_context_symbols::index", "runtime_smart_context_parse_symbol_line")]]).join("\n"),
  /source symbol indexing must use the Mojo planner/u);
  assert.match(findViolations([[SMART_CONTEXT_SYMBOLS_CONSUMER_FILE,
    smartContextSymbolConsumer + "\nruntime_smart_context_line_excerpt(lines, start, end);\n"]]).join("\n"),
  /symbol range excerpt validity must remain Mojo-owned/u);
  assert.match(findViolations([[SMART_CONTEXT_SYMBOLS_RUST_FILE,
    "fn runtime_smart_context_parse_symbol_line() {}"]]).join("\n"),
  /replaced Rust source-symbol parser must remain deleted/u);
  assert.match(findViolations([[SMART_CONTEXT_SYMBOLS_MOJO_FILE,
    "def symbol_parse_declaration(): pass"]]).join("\n"),
  /source-symbol semantics must remain in the versioned Mojo kernel/u);
  assert.match(findViolations([[SMART_CONTEXT_SYMBOLS_CALLER_TEST_FILE,
    "fn unrelated_test() {}"]]).join("\n"),
  /Smart Context artifact caller coverage must retain/u);
  const updateNoticeVersionCaller = [
    "update_notice_policy::should_emit_notice(",
    "update_notice_policy::install_channel(",
    "update_notice_policy::cache_is_fresh(",
    "release_version_is_valid(",
    "map_update_notice_mojo(",
    "Err(error) if is_update_notice_mojo_error(&error) => return Err(error)",
  ].join("\n");
  const updateNoticeVersionUpdater = [
    "update_notice_policy::update_decision(",
    "Err(error) if is_update_notice_mojo_error(&error) => return Err(error)",
  ].join("\n");
  const updateNoticeVersionAdapter = [
    "prodex_update_notice_policy_v1(",
    "release_version_is_valid(",
    "compare_release_versions(",
    "update_decision(",
  ].join("\n");
  const updateNoticeVersionAdapterModule = [
    "update_notice_policy::release_version_is_valid(",
    "update_notice_policy::compare_release_versions(",
    "ReleaseVersionOrder::Total",
  ].join("\n");
  const updateNoticeVersionMojo = [
    '@export("prodex_update_notice_policy_v1")',
    "update_notice_parse_release_version(",
    "update_notice_release_version_compare(",
    "UPDATE_NOTICE_RELEASE_VERSION_VALID",
    "UPDATE_NOTICE_RELEASE_VERSION_COMPARE",
    "UPDATE_NOTICE_UPDATE_DECISION",
  ].join("\n");
  const updateNoticeVersionFiles = [
    [UPDATE_NOTICE_VERSION_CALLER_FILE, updateNoticeVersionCaller],
    [UPDATE_NOTICE_VERSION_UPDATER_FILE, updateNoticeVersionUpdater],
    [UPDATE_NOTICE_VERSION_ADAPTER_MODULE_FILE, updateNoticeVersionAdapterModule],
    [UPDATE_NOTICE_VERSION_ADAPTER_FILE, updateNoticeVersionAdapter],
    [UPDATE_NOTICE_VERSION_MOJO_FILE, updateNoticeVersionMojo],
  ];
  assert.deepEqual(findViolations(updateNoticeVersionFiles), []);
  assert.match(findViolations([[UPDATE_NOTICE_VERSION_CALLER_FILE,
    updateNoticeVersionCaller + "\nVersion::parse(text)"]]).join("\n"),
  /restored Rust update-notice semantics/u);
  assert.match(findViolations([[UPDATE_NOTICE_VERSION_UPDATER_FILE,
    updateNoticeVersionUpdater.replace(
      "Err(error) if is_update_notice_mojo_error(&error) => return Err(error)",
      "Err(_) => return Ok(None)",
    )]]).join("\n"),
  /release-version caller must retain/u);
  assert.match(findViolations([[UPDATE_NOTICE_VERSION_ADAPTER_MODULE_FILE,
    updateNoticeVersionAdapterModule.replace("compare_release_versions(", "old_compare(")]]).join("\n"),
  /release-version adapter must retain/u);
  assert.match(findViolations([[UPDATE_NOTICE_VERSION_MOJO_FILE,
    updateNoticeVersionMojo.replace("update_notice_parse_release_version(", "old_rust_parser(")]]).join("\n"),
  /Mojo release-version owner must retain/u);
  const liveLogRecordConsumer = [
    "fn bounded_live_log_line(line: &str) -> Result<String, MojoError> {",
    "record_exceeds_bound(line.len())?",
    "nested_string_clip_end(text)?",
    "json_plan(serialized.len())?",
    "truncate_plain_text(line)",
    "let line = bounded_live_log_line(line)?;",
  ].join("\n");
  assert.deepEqual(findViolations([[LIVE_LOG_RECORD_FILE, liveLogRecordConsumer]]), []);
  assert.match(findViolations([[LIVE_LOG_RECORD_FILE,
    liveLogRecordConsumer.replace("nested_string_clip_end(text)?", "clip_json_strings(value, 8192);")]])
    .join("\n"), /restored Rust live-log clipping or truncation policy|must propagate through Mojo/u);
  assert.deepEqual(findViolations([
    [LOG_LOAD_APP_FILE,
      "prodex_mojo_core::log_load::is_routine_event(name); prodex_mojo_core::log_load::aggregate_update(input); prodex_mojo_core::log_load::aggregate_summary(1, 0, false);"],
    [LOG_LOAD_TUI_FILE,
      "LogLoadAggregate::plan_observation(previous, name, key, run, now); plan.coalesce; aggregate.apply_plan(...); LogLoadAggregate::from_plan(...);"],
    [LOG_LOAD_ADAPTER_FILE, "prodex_mojo_log_load_semantics_v1("],
  ]), []);
  assert.match(findViolations([[LOG_LOAD_APP_FILE,
    "fn is_routine_load_event(event_name: &str) { matches!(event_name, \"busy\"); }"],
  ]).join("\n"), /restored Rust log-load classification/u);
  assert.match(findViolations([[LOG_LOAD_TUI_FILE,
    "aggregate.key == key && now.saturating_duration_since(aggregate.last_seen) <= window"],
  ]).join("\n"), /freshness and key decisions must use Mojo/u);
  assert.match(findViolations([[LOG_LOAD_ADAPTER_FILE, "fn adapter() {}"]]).join("\n"),
    /must retain the required Mojo ABI call/u);
  const sessionUsageConsumer = `fn goal_resume_line_has_usage_limit(line: &str) -> bool {
    runtime_session_usage_limit_marker(&input, format)
}`;
  assert.deepEqual(findViolations([
    [SESSION_USAGE_LIMIT_CONSUMER_FILE, sessionUsageConsumer],
    [SESSION_USAGE_LIMIT_ADAPTER_FILE,
      "RUNTIME_ERROR_MODE_SESSION_USAGE_LIMIT: i64 = 15\npub fn runtime_session_usage_limit_marker(\nRuntimeUsageLimitInputFormat::Json\nRUNTIME_ERROR_SESSION_USAGE_LIMIT_MAX_BYTES"],
    [SESSION_USAGE_LIMIT_FACADE_FILE, "runtime_session_usage_limit_marker"],
    [SESSION_USAGE_LIMIT_MOJO_FILE,
      "comptime RUNTIME_ERROR_MODE_SESSION_USAGE_LIMIT: Int64 = 15\nRUNTIME_ERROR_SESSION_USAGE_LIMIT_MAX_NODES: Int64 = 2_048\ndef runtime_error_session_usage_limit_marker(\ndef runtime_error_session_usage_json_matches(\ndef runtime_error_session_usage_queue_children("],
  ]), []);
  assert.match(findViolations([[SESSION_USAGE_LIMIT_CONSUMER_FILE,
    sessionUsageConsumer + "\nfn goal_resume_structured_usage_limit() {}"]]).join("\n"),
  /session usage-limit detection must use Mojo without a Rust semantic copy/u);
  assert.match(findViolations([["crates/prodex-domain/src/secrets.rs",
    "pub fn is_well_formed(&self) -> bool { true }\nfn secret_ref_part_is_well_formed() {}"]]).join("\n"),
  /SecretRef::is_well_formed must retain Mojo validation/u);
  const secretPolicyConsumer = [
    "pub fn is_well_formed(&self) -> bool {",
    "    prodex_mojo_core::secret_policy::secret_reference_is_well_formed(provider, name, version)",
    "    }",
    "pub fn validate(&self) -> Result<(), SecretRotationPolicyError> {",
    "    prodex_mojo_core::secret_policy::secret_rotation_policy_decision(self.max_age_seconds, self.overlap_seconds)",
    "    }",
  ].join("\n");
  assert.deepEqual(findViolations([["crates/prodex-domain/src/secrets.rs", secretPolicyConsumer]]), []);
  assert.match(findViolations([["crates/prodex-domain/src/secrets.rs",
    secretPolicyConsumer.replace(
      "prodex_mojo_core::secret_policy::secret_rotation_policy_decision(self.max_age_seconds, self.overlap_seconds)",
      "if self.max_age_seconds == 0 { return Err(ZeroMaxAge); }",
    )]]).join("\n"), /SecretRotationPolicy::validate must retain Mojo policy/u);
  const governanceOrderConsumer = [
    "governance_finding_minimum_classification(",
    "governance_findings_exceed_classification(",
    "governance_classification_label(",
    "governance_coverage_combine(",
    "governance_coverage_label(",
    "governance_content_location_path_valid(",
    "governance_inspection_token_valid(",
    "governance_inspection_limits_valid(",
    "prodex_mojo_core::policy::governance_inspection_order(",
    "apply_mojo_order(findings, &order.finding_indices)",
    "apply_mojo_order(tags, &order.tag_indices)",
    "apply_mojo_order(reason_codes, &order.reason_code_indices)",
  ].join("\n");
  assert.deepEqual(findViolations([
    [GOVERNANCE_INSPECTION_CONSUMER_FILE, governanceOrderConsumer],
    [GOVERNANCE_INSPECTION_ADAPTER_FILE,
      "fn prodex_mojo_governance_inspection_order_v1() {}\npub fn governance_inspection_order() {}\nfn validate_governance_order() {}"],
    [GOVERNANCE_INSPECTION_MOJO_FILE,
      '@export("prodex_mojo_governance_inspection_order_v1")\ndef governance_finding_order_sort(): pass\ndef governance_view_order_sort(): pass\ndef governance_view_order_deduplicate(): pass'],
    [GOVERNANCE_INSPECTION_TEST_FILE,
      "fn inspection_result_ordering_uses_mojo_key_and_deduplicates() {}"],
  ]), []);
  assert.match(findViolations([[GOVERNANCE_INSPECTION_CONSUMER_FILE,
    governanceOrderConsumer + "\ntags.sort(); tags.dedup();"]]).join("\n"),
  /contains restored Rust inspection ordering or deduplication/u);
  assert.match(findViolations([[GOVERNANCE_INSPECTION_ADAPTER_FILE,
    "pub fn governance_inspection_order() {}"]]).join("\n"),
  /inspection ordering adapter must use the bounded Mojo ABI/u);
  assert.match(findViolations([[GOVERNANCE_INSPECTION_MOJO_FILE,
    "def governance_finding_order_sort(): pass"]]).join("\n"),
  /finding order and metadata deduplication must stay in the governance Mojo kernel/u);
  assert.match(findViolations([[GOVERNANCE_INSPECTION_TEST_FILE,
    "fn unrelated_test() {}"]]).join("\n"),
  /production governance ordering needs a caller-boundary regression test/u);
  const runtimeLogRetentionCalls = [
    "mojo_retention::bounded_text_policy_value(raw, default, min, max);",
    "selection::remove_expired_runtime_logs(logs, oldest_allowed);",
    "selection::remove_over_budget_runtime_logs(logs, policy);",
  ].join("\n");
  const runtimeLogSelectionCalls = [
    "mojo_retention::log_expired_candidate_plan(&candidates, oldest_allowed);",
    "mojo_retention::log_over_budget_candidate_plan(&candidates, count, max_files, bytes, budget);",
  ].join("\n");
  const runtimeLogAdapterCalls = [
    "prodex_log_throughput_sample_plan_v1();",
    "prodex_log_throughput_completed_rate_v1();",
    "prodex_log_throughput_stream_rate_v1();",
    "prodex_log_retention_policy_v1();",
    "prodex_log_retention_candidates_v1();",
  ].join("\n");
  assert.deepEqual(findViolations([
    [RUNTIME_LOG_RETENTION_FILE, runtimeLogRetentionCalls],
    [RUNTIME_LOG_RETENTION_SELECTION_FILE, runtimeLogSelectionCalls],
    ["crates/prodex-mojo-core/src/log_throughput_policy.rs", runtimeLogAdapterCalls],
  ]), []);
  assert.match(findViolations([[RUNTIME_LOG_RETENTION_SELECTION_FILE,
    runtimeLogSelectionCalls + "\nlogs.sort_by(|left, right| left.modified_epoch_seconds.cmp(&right.modified_epoch_seconds));"]]).join("\n"),
  /contains restored Rust runtime-log retention selection semantics/u);
  assert.match(findViolations([[RUNTIME_LOG_RETENTION_SELECTION_FILE,
    "mojo_retention::log_expired_candidate_plan(&candidates, oldest_allowed);"]]).join("\n"),
  /must retain Mojo call mojo_retention::log_over_budget_candidate_plan/u);
  assert.match(findViolations([[RUNTIME_LOG_RETENTION_FILE,
    "mojo_retention::bounded_text_policy_value(raw, default, min, max);\nfn remove_expired_runtime_logs() {}"]]).join("\n"),
  /contains restored Rust runtime-log retention selection semantics/u);
  assert.match(findViolations([[RUNTIME_LOG_RETENTION_FILE,
    "mojo_retention::bounded_text_policy_value(raw, default, min, max);\nselection::remove_expired_runtime_logs(logs, cutoff);"]]).join("\n"),
  /must retain selection::remove_over_budget_runtime_logs/u);
  assert.match(findViolations([[
    "crates/prodex-mojo-core/src/log_throughput_policy.rs",
    runtimeLogAdapterCalls.split("\n")
      .filter((call) => !call.includes("prodex_log_retention_candidates_v1("))
      .join("\n"),
  ]]).join("\n"), /must retain prodex_log_retention_candidates_v1/u);
  assert.deepEqual(findViolations([[PROFILE_HEALTH_CIRCUIT_FILE,
    "fn runtime_profile_circuit_half_open_probe_seconds(score: u32) -> i64 { runtime_proxy_crate::runtime_profile_circuit_half_open_probe_seconds(score) }"]]), []);
  assert.match(findViolations([[PROFILE_HEALTH_CIRCUIT_FILE,
    "fn runtime_profile_circuit_half_open_probe_seconds(score: u32) -> i64 { let multiplier = 1_i64.checked_shl(score.saturating_sub(4).min(3)).unwrap_or(i64::MAX); 5_i64.saturating_mul(multiplier).min(60) }"]]).join("\n"),
  /must delegate to the Mojo runtime-health adapter/u);
  const routeAffinityLogFile = "crates/prodex-runtime-proxy/src/route_affinity_log.rs";
  const routeAffinityLogCalls = "prodex_mojo_core::log::render_route_affinity_log(); prodex_mojo_core::log::render_route_affinity_owner_logs();";
  assert.deepEqual(findViolations([[routeAffinityLogFile, routeAffinityLogCalls]]), []);
  assert.match(findViolations([[routeAffinityLogFile,
    "prodex_mojo_core::log::render_route_affinity_log();"]]).join("\n"),
  /route-affinity log rendering must use Mojo/u);
  assert.match(findViolations([[routeAffinityLogFile,
    routeAffinityLogCalls + '\n" route_affinity_recompute reason=";']]).join("\n"),
  /restored Rust route-affinity log semantic/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/chain_log.rs",
    "fn runtime_proxy_chain_retried_owner_log_message() {}"]]).join("\n"),
  /chain log rendering must use Mojo/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/chain_log.rs",
    'prodex_mojo_core::log::render_chain_log();\n" chain_retried_owner profile=";']]).join("\n"),
  /restored Rust chain-log semantic/u);
  assert.match(findViolations([["crates/prodex-mojo-core/src/log.rs",
    "prodex_mojo_route_affinity_log_render_v1(); prodex_mojo_chain_log_render_v1();"]]).join("\n"),
  /runtime log adapter must retain Mojo ABI/u);
  assert.equal(
    findViolations([[
      "crates/prodex-runtime-quota/src/selection/scoring/profile_order.rs",
      '#[cfg(not(feature = "mojo"))] fn rust_order() {}',
    ]]).length,
    1,
  );
  assert.equal(
    findViolations([[
      "crates/prodex-runtime-doctor/src/parsing/selection.rs",
      '#[cfg(not(feature = "runtime-log-mojo"))] fn rust_selection() {}',
    ]]).length,
    1,
  );
  const responseViolations = (contents) => findViolations([[ANTHROPIC_RESPONSE_FILE, contents]]);
  assert.deepEqual(responseViolations(`
    fn response_plan_with_mojo() {
      let kind = classified.kind;
      ResponsePlanItem { kind: match item.kind { _ => ResponsePlanKind::Message } }
    }
  `), []);
  assert.deepEqual(responseViolations(
    "#[cfg(test)] mod tests { assert_eq!(plan, ResponsePlanItem { kind: ResponsePlanKind::Message }); }",
  ), []);
  assert.match(responseViolations("fn anthropic_response_block_input() {}")[0], /Rust response block classifier/u);
  assert.match(responseViolations("enum ResponseBlockKind { Text }")[0], /duplicate Rust response block kind/u);
  assert.match(responseViolations("enum ResponsePlanKind { Message }")[0], /duplicate Rust response plan kind/u);
  assert.match(responseViolations("struct ResponsePlanItem { kind: usize }")[0], /duplicate Rust response plan item/u);
  assert.match(responseViolations("struct ResponseBlockInput { value: usize }")[0], /duplicate Rust response block input/u);
  assert.match(responseViolations("fn plan_with_rust() {}")[0], /Rust response planner/u);
  assert.match(responseViolations("#[cfg(test)] fn plan_with_rust() {}")[0], /Rust response planner/u);
  assert.match(responseViolations('match kind { Some("tool_use") => (), _ => () }')[0],
    /Rust response block classification/u);
  assert.match(responseViolations("ResponsePlanItem { kind: ResponsePlanKind::Message }")[0],
    /Rust response plan construction/u);
  assert.match(responseViolations('#[cfg(not(feature = "mojo"))] fn fallback() {}')[0],
    /feature-off Rust response path/u);
  assert.match(findViolations([[ANTHROPIC_MESSAGES_FILE,
    "fn anthropic_response_envelope_rust() {}"]])[0], /Rust response envelope/u);
  assert.match(findViolations([[ANTHROPIC_REQUEST_FALLBACK_FILE, "fn fallback() {}"]])[0],
    /Rust fallback or oracle/u);
  assert.match(findViolations([[ANTHROPIC_REQUEST_ORACLE_FILE, "fn oracle() {}"]])[0],
    /Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-context/src/critical_signal/rust_oracle.rs",
    "fn count_critical_signals() {}"]])[0], /Rust fallback or oracle/u);
  const subAgentRendering = (contents) => findViolations([[SUB_AGENT_RENDERING_FILE, contents]]);
  const subAgentValidation = (contents) => findViolations([[SUB_AGENT_CLI_VALIDATION_FILE, contents]]);
  assert.deepEqual(subAgentValidation(
    "prodex_mojo_core::sub_agent_policy::provider_url_violation(local, url_present);",
  ), []);
  assert.match(subAgentValidation("fn validate_sub_agent_flags() {} ").join("\n"),
    /must call Mojo provider URL policy/u);
  for (const predicate of [
    "args.sub_agent_url.is_some() && provider != ProviderId::Local",
    "args.sub_agent && provider == ProviderId::Local && args.sub_agent_url.is_none()",
  ]) {
    assert.match(subAgentValidation(
      "prodex_mojo_core::sub_agent_policy::provider_url_violation(local, url_present);\n" + predicate,
    ).join("\n"), /restored Rust sub-agent provider URL predicates/u);
  }
  assert.deepEqual(subAgentRendering(`
    prodex_mojo_core::sub_agent_policy::render_overlay(&input);
    prodex_mojo_core::sub_agent_policy::render_enabled_dry_run_report(&input);
    prodex_mojo_core::sub_agent_policy::render_disabled_dry_run_report(false);
  `), []);
  assert.match(subAgentRendering("fn render_sub_agent_overlay() { SUB_AGENT_RULES: [] } ").join("\n"),
    /replaced Rust sub-agent text template/u);
  assert.match(subAgentRendering("fn render_sub_agent_overlay() {}\nfn render_report() {} ").join("\n"),
    /must use Mojo renderer/u);
  assert.deepEqual(findViolations([[SUB_AGENT_RENDER_ADAPTER_FILE,
    "const RENDER_ABI_VERSION: i64 = 1; prodex_sub_agent_render_v1(); fn sub_agent_render_v1_has_exact_output_and_checked_edges() {}"]]), []);
  assert.match(findViolations([[SUB_AGENT_RENDER_ADAPTER_FILE,
    "const RENDER_ABI_VERSION: i64 = 1; fn render_template_rust() {}"]]).join("\n"),
    /must retain versioned Mojo ABI contract/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/smart_context/token_accounting/pressure.rs",
    "fn smart_context_pressure_snapshot_rust() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([[SMART_CONTEXT_CORE_FILE,
    "fn smart_context_exactness_guard_rust() {}"]]).join("\n"),
    /Rust semantic oracle or copy/u);
  assert.match(findViolations([[SMART_CONTEXT_CORE_FILE,
    "fn smart_context_exactness_guard() {}"]]).join("\n"),
    /exactness must use the Mojo plan/u);
  assert.deepEqual(findViolations([[SMART_CONTEXT_CORE_FILE,
    "prodex_mojo_core::runtime::smart_context_exactness_plan()"]]), []);
  assert.match(findViolations([[ADAPTIVE_BUDGET_FILE,
    "fn smart_context_adaptive_budget_policy() {}"]]).join("\n"),
    /adaptive budget must use the Mojo plan/u);
  assert.deepEqual(findViolations([[ADAPTIVE_BUDGET_FILE,
    "prodex_mojo_core::runtime::smart_context_adaptive_budget_plan()"]]), []);
  const duplicateTextPlannerCalls = [
    "smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Rewrite)",
    "smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Probe)",
  ].join("; ");
  assert.deepEqual(findViolations([[SMART_CONTEXT_DUPLICATE_VALIDATION_FILE,
    duplicateTextPlannerCalls]]), []);
  assert.match(findViolations([[SMART_CONTEXT_DUPLICATE_VALIDATION_FILE,
    duplicateTextPlannerCalls + "; fn runtime_smart_context_value_has_duplicate_text() {}"]]).join("\n"),
  /restored Rust duplicate-text decision logic/u);
  assert.match(findViolations([[SMART_CONTEXT_DUPLICATE_VALIDATION_FILE,
    duplicateTextPlannerCalls.replace("SmartContextDuplicateTextMode::Probe", "RustProbe")]]).join("\n"),
  /duplicate-text path must use Mojo planner/u);
  assert.match(findViolations([[SMART_CONTEXT_DUPLICATE_ADAPTER_FILE,
    "fn adapter() {}"]]).join("\n"), /duplicate-text adapter must retain/u);
  assert.deepEqual(findViolations([[SMART_CONTEXT_DUPLICATE_ADAPTER_FILE,
    "fn prodex_smart_context_duplicate_text_plan_v1() {}"]]), []);
  assert.deepEqual(findViolations([[SMART_CONTEXT_DUPLICATE_MOJO_FILE,
    '@export("prodex_smart_context_duplicate_text_plan_v1") def smart_context_duplicate_text_plan_kernel() {}']]), []);
  assert.match(findViolations([[SMART_CONTEXT_DUPLICATE_MOJO_FILE,
    "def smart_context_duplicate_text_plan_kernel() {}"]]).join("\n"),
  /duplicate-text decision must remain Mojo-owned/u);
  assert.match(findViolations([[DEEPSEEK_SIMPLE_REQUEST_FILE,
    "fn deepseek_provider_core_simple_request() {}"]]).join("\n"),
  /DeepSeek simple-request eligibility must use Mojo/u);
  assert.deepEqual(findViolations([[DEEPSEEK_SIMPLE_REQUEST_FILE,
    "DeepSeekRequestPolicyOperation::SimpleRequest; prodex_mojo_core::rich::deepseek_request_policy()"]]), []);
  assert.match(findViolations([[DEEPSEEK_METADATA_FILE, "fn metadata() {}"]]).join("\n"),
    /DeepSeek metadata\/format migration must retain/u);
  const deepseekMetadataMojoMarkers = "DeepSeekKernelOperation::RequestMetadata; DeepSeekKernelOperation::ResponseFormat; DeepSeekRequestPolicyOperation::ResponseFormatShape; DeepSeekRequestPolicyOperation::MetadataShape; DeepSeekRequestPolicyOperation::JsonGuidance";
  assert.deepEqual(findViolations([[DEEPSEEK_METADATA_FILE, deepseekMetadataMojoMarkers]]), []);
  assert.match(findViolations([[DEEPSEEK_METADATA_FILE, deepseekMetadataMojoMarkers + "; fn deepseek_provider_core_message_has_json_guidance() {}"]]).join("\n"),
    /restored Rust DeepSeek metadata\/format semantics/u);
  const kiroChatResponseViolations = (contents) => findViolations([[KIRO_CHAT_RESPONSE_FILE, contents]]);
  assert.deepEqual(kiroChatResponseViolations(`
    pub fn kiro_provider_core_chat_completion_value_from_response(value: &Value, id: u64) -> Value {
      kiro_provider_core_try_chat_completion_value_from_response(value, id)
    }
    pub(super) fn kiro_provider_core_try_chat_completion_value_from_response(value: &Value, id: u64) {
      prodex_mojo_core::rich::kiro_rewrite_chat_response_json(value, id)
    }
    KiroKernelOperation::ModelList;
    KiroKernelOperation::ModelNotFound;
    KiroKernelOperation::InvalidRequestError;
    KiroKernelOperation::UnsupportedPathError;
    KiroKernelOperation::FinishReason;
    pub fn kiro_provider_core_anthropic_message_value_from_response(value: &Value, model: &str) -> Value {
      prodex_mojo_core::rich::kiro_rewrite_anthropic_response_json(value, model)
    }
    pub fn kiro_provider_core_apply_response_runtime_metadata() {}
  `), []);
  assert.match(kiroChatResponseViolations(`
    pub fn kiro_provider_core_chat_completion_value_from_response(response: &Value, _: u64) -> Value {
      response.get("output").cloned().unwrap_or_default()
    }
    pub fn kiro_provider_core_apply_response_runtime_metadata() {}
  `)[0], /Kiro chat response mapping must use Mojo/u);
  const kiroFinalStreamViolations = (contents) => findViolations([[KIRO_FINAL_STREAM_FILE, contents]]);
  assert.deepEqual(kiroFinalStreamViolations(`
    pub(super) fn runtime_kiro_send_final_stream() {
      prodex_provider_core::kiro_provider_core_response_final_event();
    }
  `), []);
  assert.match(kiroFinalStreamViolations(`
    pub(super) fn runtime_kiro_send_final_stream() {
      match response.get("status").and_then(Value::as_str) { _ => () }
    }
  `).join("\n"), /restored Rust Kiro terminal status selection/u);
  assert.match(kiroFinalStreamViolations(`
    pub(super) fn runtime_kiro_send_final_stream() {}
  `).join("\n"), /must use Mojo final event plan/u);
  const kiroStreamPlanViolations = (contents) => findViolations([[KIRO_STREAM_FILE, contents]]);
  assert.deepEqual(kiroStreamPlanViolations(`
    pub fn kiro_provider_core_response_final_event() {
      let operation = KiroKernelOperation::ResponseFinalEvent;
      kiro_mojo_value(input)
    }
  `), []);
  assert.match(kiroStreamPlanViolations(`
    pub fn kiro_provider_core_response_final_event() {
      if status == "failed" { rust_event() }
    }
  `).join("\n"), /must use Mojo final event plan/u);
  assert.match(findViolations([[KIRO_REQUEST_FILE,
    'fn kiro_provider_core_chat_completions_request_body() { kiro_rewrite_chat_request_json(); object.remove("functions"); }']]).join("\n"),
  /restored Rust Kiro legacy chat-tool rewrite semantics/u);
  assert.deepEqual(findViolations([[KIRO_REQUEST_FILE,
    "fn kiro_provider_core_chat_completions_request_body() { kiro_rewrite_chat_request_json(); }"]]), []);

  const kiroPromptMarkers = `
    KiroKernelOperation::PromptFromChatMessages;
    KiroKernelOperation::LegacyFunctionTool;
    KiroKernelOperation::LegacyToolChoice;
  `;
  const kiroPromptLocalRewrite = `
    kiro_provider_core_prompt_from_chat_messages as runtime_kiro_prompt_from_messages;
    runtime_kiro_prompt_from_messages(&translated.messages);
  `;
  const kiroPromptAbiTests = `
    KiroKernelOperation::PromptFromChatMessages;
    kiro_prompt_recursively_applies_text_content_output_precedence_and_unicode_trim;
    kiro_prompt_joins_only_nonempty_sections_and_uses_empty_fallback;
  `;
  assert.deepEqual(findViolations([
    [KIRO_MESSAGES_FILE, kiroPromptMarkers],
    [KIRO_LOCAL_REWRITE_FILE, kiroPromptLocalRewrite],
    [KIRO_PROMPT_ABI_TEST_FILE, kiroPromptAbiTests],
  ]), []);
  assert.match(findViolations([[KIRO_MESSAGES_FILE,
    kiroPromptMarkers + "\nmessage.get(\"content\").and_then(Value::as_str).trim()"]]).join("\n"),
  /contains replaced Rust Kiro prompt or legacy-function semantics/u);
  assert.match(findViolations([[KIRO_LOCAL_REWRITE_FILE,
    "fn local_rewrite_kiro() {}"]]).join("\n"),
  /must reach the Mojo-backed prompt adapter/u);
  const kiroCatalogAdapter = `
    kiro_model_catalog_plan(nodes, raw, PROVIDER_MODEL_CATALOG_HARD_LIMIT);
    KiroModelCatalogPlan::Ready;
    merge_catalog_ids(&[], &ids);
    merge_provider_model_catalog_json(ProviderId::Kiro, &models);
  `;
  assert.deepEqual(findViolations([[KIRO_CATALOG_NORMALIZER_FILE, kiroCatalogAdapter]]), []);
  assert.match(findViolations([[KIRO_CATALOG_NORMALIZER_FILE,
    kiroCatalogAdapter + " first_nonempty_string(model, keys); model.trim()"]]).join("\n"),
  /contains replaced Rust Kiro model-catalog decisions/u);
  assert.deepEqual(findViolations([[KIRO_CATALOG_APP_ADAPTER_FILE, `
    prodex_provider_core::normalize_kiro_model_catalog(value);
    prodex_provider_core::normalize_kiro_model_catalog_models(models);
  `]]), []);
  assert.match(findViolations([[KIRO_CATALOG_APP_ADAPTER_FILE,
    `prodex_provider_core::normalize_kiro_model_catalog(value); first_models_array(value);`]]).join("\n"),
  /contains replaced Rust Kiro model-catalog decisions/u);
  assert.deepEqual(findViolations([[KIRO_CATALOG_ABI_FILE,
    `fn prodex_mojo_kiro_catalog_normalize_v1(
     pub fn kiro_model_catalog_plan(
     prodex_runtime_response_metadata_v1(
     prodex_session_report_metadata_v1(
     pub fn session_report_metadata(`]]), []);
  assert.deepEqual(findViolations([[KIRO_CATALOG_MOJO_FILE,
    "KIRO_CATALOG_ABI_VERSION: Int64 = 1\ndef kiro_model_catalog_normalize_v1("]]), []);
  assert.deepEqual(findViolations([[KIRO_CATALOG_ABI_TEST_FILE,
    "kiro_model_catalog_plan_uses_alias_precedence_unicode_trim_and_stable_source_order\nkiro_catalog_plan_returns_typed_missing_empty_and_limit_issues"]]), []);
  assert.match(findViolations([[DEEPSEEK_STRICT_TOOLS_FILE, "fn strict_schema() {}"]]).join("\n"),
    /strict schema normalization must use Mojo/u);
  assert.match(findViolations([[DEEPSEEK_STRICT_SCHEMA_FILE,
    "fn deepseek_provider_core_validate_strict_schema() {}"]]).join("\n"),
  /replaced Rust strict-schema behavior/u);
  assert.match(findViolations([[DEEPSEEK_STRICT_TOOLS_FILE,
    "DeepSeekKernelOperation::StrictFunctionSchema; deepseek_provider_core_validate_strict_schema()"]]).join("\n"),
  /contains a Rust strict-schema validator/u);
  assert.match(findViolations([[DEEPSEEK_SIMPLE_REQUEST_FILE,
    "fn deepseek_provider_core_function_tools() {}"]]).join("\n"),
  /replaced Rust semantic implementation/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/deepseek_bridge/request_probe/input.rs",
    "fn deepseek_simple_input_item() {}"]]).join("\n"),
  /retained Rust fallback or oracle/u);
  assert.match(findViolations([[RUNTIME_QUOTA_FILE,
    "fn runtime_proxy_quota_summary_from_usage_snapshot_at() {}"]]).join("\n"),
    /quota snapshot and gate decisions must use Mojo/u);
  assert.deepEqual(findViolations([[RUNTIME_QUOTA_FILE,
    "mojo::quota_snapshot_plan(); mojo::quota_gate_plan(); runtime_precommit_quota_block_reason_label(self as i64); runtime_quota_pressure_band_reason_label(band as i64); runtime_quota_window_status_reason_label(status as i64); runtime_quota_source_label(source as i64);"]]), []);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/quota/rust_oracles.rs",
    "fn summary_from_usage_snapshot_at() {}"]]).join("\n"),
    /retained Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-runtime-launch/src/args_oracle.rs",
    "fn normalize_run_codex_args() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-runtime-launch/src/args_resume.rs",
    "fn retarget_codex_tui_resume_args() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-runtime-doctor/Cargo.toml",
    '[features]\ndefault = []\nstate-summary-mojo = []']]).join("\n"), /default features must include state-summary-mojo/u);
  assert.match(findViolations([[RUNTIME_PROXY_CARGO_FILE,
    'prodex_mojo_core = { workspace = true, features = ["mojo-runtime"] }']]).join("\n"),
    /Retry-After parsing requires Mojo rich/u);
  assert.match(findViolations([["crates/prodex-runtime-doctor/src/state_summary/profiles.rs",
    '#[cfg(not(feature = "state-summary-mojo"))] fn rust_summary() {}']])[0],
    /feature-off Rust path/u);
  for (const filePath of [
    "crates/prodex-runtime-doctor/src/state_summary/quota.rs",
    "crates/prodex-runtime-doctor/src/state_summary/routes.rs",
  ]) {
    assert.match(findViolations([[filePath,
      '#[cfg(not(feature = "mojo"))] fn old_path() {}']])[0],
      /feature-off Rust path/u);
  }
  assert.match(findViolations([[QUOTA_WINDOWS_FILE,
    "prodex_mojo_core::quota::quota_blocked_status_label(0);\nfn quota_error_summary_basic(lower: &str) {}"]]).join("\n"),
    /Rust quota error classifier/u);
  assert.match(findViolations([[QUOTA_WINDOWS_FILE,
    'prodex_mojo_core::quota::quota_blocked_status_label(0);\nfn format_blocked_quota_status() {\n    #[cfg(not(feature = "mojo"))] rust();\n}']]).join("\n"),
    /feature-off Rust classifier/u);
  const quotaResetEpochConsumer = [
    "prodex_mojo_core::quota::reset_epoch::quota_reset_epoch_precedence(",
    'quota_json_i64_path(&value, &["resets_at"]);',
    'quota_json_i64_path(&value, &["reset_at"]);',
    'quota_json_i64_path(&value, &["error", "resets_at"]);',
    'quota_json_i64_path(&value, &["error", "reset_at"]);',
  ].join("\n");
  const quotaResetEpochAdapter = [
    "pub struct QuotaResetEpochInput",
    "pub fn quota_reset_epoch_precedence(",
    "prodex_quota_reset_epoch_v1(",
  ].join("\n");
  const quotaDisplayAdapter = [
    "prodex_quota_display_label_v1(",
    "prodex_quota_window_label_plan_v1(",
    "prodex_quota_copilot_feature_key_v1(",
    "prodex_quota_copilot_display_v1(",
    "prodex_quota_copilot_main_remaining_percent_v1(",
    "prodex_quota_ready_pool_remaining_v1(",
    "prodex_quota_info_pool_remaining_v1(",
    "prodex_quota_gemini_bucket_label_v1(",
    "prodex_quota_gemini_bucket_summary_v1(",
    "prodex_quota_gemini_display_v1(",
    "prodex_quota_report_compare_v1(",
    "prodex_quota_workspace_label_v1(",
  ].join("\n");
  const quotaResetEpochMojo = [
    '@export("prodex_quota_reset_epoch_v1")',
    "while index < 4:",
    "primary_used >= 100",
    "secondary_used >= 100",
    "fields[unsafe_offset=9] == 1",
    "fields[unsafe_offset=11] == 1",
  ].join("\n");
  const quotaResetEpochMojoTest = [
    "quota_reset_epoch_prefers_valid_candidates_in_declared_order",
    "quota_reset_epoch_applies_used_percent_gates_and_missing_values",
  ].join("\n");
  const quotaResetEpochCallerTest = [
    "quota_reset_json_uses_top_level_then_nested_candidate_order",
    "quota_reset_json_uses_used_percent_gates_and_header_fallback_order",
    "quota_reset_json_preserves_serde_duplicate_key_behavior",
    "quota_reset_json_ignores_malformed_and_non_object_values",
  ].join("\n");
  assert.deepEqual(findViolations([
    [QUOTA_WINDOWS_FILE,
      quotaResetEpochConsumer + "\nprodex_mojo_core::quota::quota_blocked_status_label(0);"],
    [QUOTA_ADAPTER_FILE, quotaDisplayAdapter],
    [QUOTA_RESET_EPOCH_ADAPTER_FILE, quotaResetEpochAdapter],
    [QUOTA_RESET_EPOCH_MOJO_FILE, quotaResetEpochMojo],
    [QUOTA_RESET_EPOCH_TEST_FILE, quotaResetEpochMojoTest],
    [QUOTA_RESET_EPOCH_CALLER_TEST_FILE, quotaResetEpochCallerTest],
  ]), []);
  assert.match(findViolations([[QUOTA_WINDOWS_FILE,
    quotaResetEpochConsumer + "\nprodex_mojo_core::quota::quota_blocked_status_label(0);\nif primary_used { return primary_reset; }"]]).join("\n"),
  /contains Rust reset-epoch precedence policy/u);
  assert.match(findViolations([[QUOTA_RESET_EPOCH_MOJO_FILE,
    quotaResetEpochMojo.replace("primary_used >= 100", "primary_used > 100")]]).join("\n"),
  /quota reset-epoch precedence must remain Mojo-owned/u);
  assert.match(findViolations([[QUOTA_RESET_EPOCH_CALLER_TEST_FILE,
    quotaResetEpochCallerTest.replace("quota_reset_json_preserves_serde_duplicate_key_behavior", "")]]).join("\n"),
  /caller-boundary tests must retain quota_reset_json_preserves_serde_duplicate_key_behavior/u);
  assert.match(findViolations([["crates/prodex-quota/src/capacity.rs",
    '#[cfg(not(feature = "mojo"))] fn old_capacity() {}']]).join("\n"),
    /feature-off Rust path/u);
  for (const filePath of [
    "crates/prodex-runtime-proxy/src/websocket_message.rs",
    "crates/prodex-runtime-proxy/src/websocket_response_tracking.rs",
  ]) {
    assert.match(findViolations([[filePath,
      '#[cfg(not(feature = "mojo"))] fn old_websocket_policy() {}']]).join("\n"),
      /feature-off Rust path/u);
  }
  assert.match(findViolations([[REHYDRATE_FILE,
    'pub fn smart_context_auto_rehydrate_plan() {\n    #[cfg(not(feature = "mojo"))] rust();\n}\nfn smart_context_auto_rehydrate_plan_mojo() {}']])[0],
    /feature-off Rust planner/u);
  assert.match(findViolations([[REHYDRATE_FILE,
    'pub fn smart_context_token_budget_tier() {\n    match tokens { 0 => 0, _ => 1 }\n}']]).join("\n"),
    /budget tier must use the Mojo policy/u);
  assert.match(findViolations([[SUPER_OVERRIDE_FILE, "fn scan_override_rust() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([[SUPER_EXPOSE_FILE, "fn rewrite_super_expose_alias() {}"]]).join("\n"),
    /Super expose alias scan must use Mojo/u);
  assert.deepEqual(findViolations([[SUPER_EXPOSE_FILE,
    'fn reassemble_super_expose_alias() { prodex_mojo_core::launch::find_super_expose_alias_index(); }']]), []);
  assert.match(findViolations([[GEMINI_SCHEMA_FILE, "fn sanitized_required() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([[GEMINI_TOOLS_FILE, "fn gemini_tool_config_from_request_oracle() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/gemini_bridge/request/simple.rs",
    "fn gemini_simple_input_item() {}"]]).join("\n"), /replaced Rust semantic implementation/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/request/tools/builtin.rs",
    "fn gemini_is_web_search_tool() {}"]]).join("\n"), /replaced Rust semantic implementation/u);
  assert.match(findViolations([[RESPONSE_FORWARDING_FILE, "fn response_content_type_is_sse() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([[QUOTA_POOL_FILE, "fn aggregate_openai_quota() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([[STATUS_SUMMARY_FILE, "fn add_quota_window() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([[STATUS_SUMMARY_FILE, "fn status_quota_from_reports() {}"]]).join("\n"),
    /status quota summary must use the Mojo kernel/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/stream.rs",
    '#[cfg(not(feature = "mojo"))] fn old_stream() {}']])[0], /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/response/status.rs",
    '#[cfg(not(feature = "mojo"))] fn old_status() {}']])[0], /feature-off Rust path/u);
  assert.match(findViolations([[GEMINI_STATUS_FILE,
    "fn gemini_finish_reason_failure_oracle() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/response/metadata.rs",
    "fn response_usage_rust() {}"]]).join("\n"), /Rust semantic oracle or copy/u);
  assert.match(findViolations([[GEMINI_TOOL_CALLS_FILE,
    "fn gemini_split_flat_namespace_tool_name() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([[GEMINI_CHAT_TOOL_CALLS_FILE,
    'let mut item = json!({"type":"function"});']]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/smart_context/token_accounting/estimation.rs",
    '#[cfg(not(feature = "mojo"))] fn old_estimator() {}']])[0], /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/health/score.rs",
    "fn effective_score_rust() {}"]]).join("\n"), /Rust semantic oracle or copy/u);
  assert.match(findViolations([["crates/prodex-runtime-store/src/profile_backoff/score.rs",
    '#[cfg(not(feature = "mojo"))] fn runtime_profile_health_sort_key() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-observability/src/metric_label.rs",
    '#[cfg(not(feature = "mojo"))] fn validate_telemetry_metric_label_rust() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/implementation_registry.rs",
    '#[cfg(not(feature = "mojo"))] mod rust_registry;']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/deepseek_bridge/messages.rs",
    '#[cfg(not(feature = "mojo"))] fn normalize_messages_rust() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/payload_detection/sse.rs",
    '#[cfg(not(feature = "mojo"))] mod rust_oracle;']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/log_event.rs",
    '#[cfg(not(feature = "mojo"))] fn runtime_proxy_parse_log_message_rust() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-quota/src/render/gemini.rs",
    '#[cfg(not(feature = "mojo"))] fn gemini_bucket_numeric_rust() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/error_policy/retry_after.rs",
    '#[cfg(not(feature = "mojo"))] fn runtime_retry_after_number_rust() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/error_policy.rs",
    '#[cfg(not(feature = "mojo"))] fn runtime_http_error_policy_rust() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/compatibility_surface.rs",
    '#[cfg(not(feature = "mojo"))] mod rust_oracle;']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/attempt_outcome.rs",
    '#[cfg(not(feature = "mojo"))] mod rust_oracle;']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/error_policy/signal.rs",
    '#[cfg(not(feature = "mojo"))] fn runtime_error_signal_message_from_text_rust() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([[QUOTA_MODEL_CAPACITY_FILE,
    "fn normalized_identifier() {}"]]).join("\n"), /replaced Rust semantic implementation/u);
  assert.match(findViolations([[RUNTIME_QUOTA_FILE,
    "fn runtime_proxy_quota_score_for_route_rust() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([[HEALTH_ABI_TEST_FILE,
    "fn effective() {}"]]).join("\n"), /replaced Rust semantic implementation/u);
  assert.match(findViolations([[DEEPSEEK_RESPONSE_FILE,
    '#[cfg(not(feature = "mojo"))] pub(super) fn deepseek_stream_event_from_chat_value() {}']]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([[DEEPSEEK_RESPONSE_TOOL_CALLS_FILE,
    "fn deepseek_split_flat_namespace_tool_name() {}"]]).join("\n"),
    /response tool-call shaping must use the Mojo kernel/u);
  assert.deepEqual(findViolations([[DEEPSEEK_RESPONSE_TOOL_CALLS_FILE,
    "fn shape() { DeepSeekKernelOperation::ResponseToolCallItem; }"]]), []);
  assert.match(findViolations([[CHAT_TOOLS_BRIDGE_FILE,
    "mod tools; pub use self::tools::*;"]]).join("\n"), /chat-tool operations must use the Mojo entrypoints/u);
  assert.match(findViolations([[CHAT_TOOLS_MOJO_FILE,
    "fn transform_bytes() {}"]]).join("\n"), /chat-tool operations must invoke the Mojo kernel/u);
  assert.match(findViolations([[RUNTIME_DOCTOR_MARKERS_FILE,
    "macro_rules! runtime_doctor_marker_registry {}"]]).join("\n"),
  /marker recognition must use the Mojo classifier/u);
  assert.deepEqual(findViolations([[RUNTIME_DOCTOR_FAILURE_CLASS_FILE,
    "fn runtime_doctor_failure_class_counts(summary: &RuntimeDoctorSummary) {\n  prodex_mojo_core::rich::runtime_doctor_marker_semantics(marker);\n}"]]), []);
  assert.match(findViolations([[RUNTIME_DOCTOR_FAILURE_CLASS_FILE,
    "fn runtime_doctor_failure_class_counts(summary: &RuntimeDoctorSummary) {\n  let classes = [(\"admission\", \"runtime_proxy_queue_overloaded\")];\n  prodex_mojo_core::rich::runtime_doctor_marker_semantics(marker);\n}"]]).join("\n"),
  /failure-class counts must use Mojo tags without Rust marker lists/u);
  assert.match(findViolations([[DOCTOR_MARKER_SUMMARY_COUNTS_CONSUMER_FILE,
    "pub fn runtime_doctor_finalize_log_summary(summary: &mut RuntimeDoctorSummary) {\n}\n"]]).join("\n"),
  /fixed selection and failure totals must use the Mojo batch reducer/u);
  assert.match(findViolations([[DOCTOR_MARKER_SUMMARY_COUNTS_CONSUMER_FILE,
    "fn runtime_doctor_failure_class_counts() {\n  counts.failure_admission; counts.failure_auth; counts.failure_continuation; counts.failure_persistence; counts.failure_quota; counts.failure_transport;\n}\nruntime_doctor_facet_count(summary);\n"]]).join("\n"),
  /fixed selection and failure totals must use the Mojo batch reducer/u);
  assert.match(findViolations([[DOCTOR_MARKER_SUMMARY_COUNTS_SELECTION_FILE,
    "pub(super) fn runtime_doctor_record_selection_summary(summary: &mut RuntimeDoctorSummary) {\n  summary.selection_summary.picked += 1;\n}\n"]]).join("\n"),
  /selection bucket totals must be reduced from marker counts/u);
  assert.match(findViolations([[DOCTOR_MARKER_ABI_ADAPTER_FILE, "fn marker_counts() {}"]]).join("\n"),
  /marker summary adapter must use the versioned bounded Mojo ABI/u);
  assert.match(findViolations([[DOCTOR_MARKER_SUMMARY_COUNTS_MOJO_FILE, "fn marker_counts() {}"]]).join("\n"),
  /fixed marker totals must retain their versioned bounded Mojo reducer/u);
  assert.match(findViolations([[DOCTOR_MARKER_ABI_TEST_FILE, "fn unrelated_test() {}"]]).join("\n"),
  /direct ABI coverage must exercise fixed selection and failure totals/u);
  assert.match(findViolations([[DOCTOR_MARKER_SUMMARY_COUNTS_CALLER_TEST_FILE, "fn unrelated_test() {}"]]).join("\n"),
  /runtime-doctor caller coverage must protect reducer caps, synthesis order, and Unicode truncation/u);
  assert.deepEqual(findViolations([[DOCTOR_COMPACT_EXIT_COUNTS_CONSUMER_FILE,
    "pub(super) fn runtime_doctor_compact_exit_counts(summary: &Summary) {\n  prodex_mojo_core::rich::runtime_doctor_compact_exit_counts(input);\n}\n"]]), []);
  assert.match(findViolations([[DOCTOR_COMPACT_EXIT_COUNTS_CONSUMER_FILE,
    "pub(super) fn runtime_doctor_compact_exit_counts(summary: &Summary) {\n  let aliases = [\"compact_candidate_exhausted\"];\n}\n"]]).join("\n"),
  /compact-exit alias grouping must use the Mojo reducer without Rust marker tables/u);
  assert.match(findViolations([[DOCTOR_MARKER_ABI_ADAPTER_FILE, "fn compact_exit_counts() {}"]]).join("\n"),
  /compact-exit adapter must use the versioned Mojo reducer and fixed output slots/u);
  assert.match(findViolations([[DOCTOR_MARKER_SUMMARY_COUNTS_MOJO_FILE, "fn compact_exit_counts() {}"]]).join("\n"),
  /compact-exit alias policy must remain in the production Mojo reducer/u);
  assert.match(findViolations([[DOCTOR_MARKER_ABI_TEST_FILE, "fn unrelated_test() {}"]]).join("\n"),
  /direct ABI coverage must assert all compact-exit marker buckets and invalid inputs/u);
  assert.match(findViolations([[DOCTOR_COMPACT_EXIT_COUNTS_CALLER_TEST_FILE, "fn unrelated_test() {}"]]).join("\n"),
  /production diagnosis caller must protect compact-exit alias totals and order/u);
  assert.match(findViolations([[DOCTOR_TIMELINE_DETAIL_CONSUMER_FILE,
    "pub(super) fn runtime_doctor_request_timeline_detail(fields: &Fields) -> String {\n  fields.iter().map(|(key, value)| format!(\"{key}={value}\")).collect()\n}\n"]]).join("\n"),
  /request-timeline field ordering, cap, truncation, and joining must use the Mojo renderer/u);
  assert.match(findViolations([[DOCTOR_RENDER_ADAPTER_FILE, "fn runtime_doctor_render() {}"]]).join("\n"),
  /request-timeline rendering must retain its bounded real-Mojo operation and ABI test/u);
  assert.match(findViolations([[DOCTOR_RENDER_MOJO_FILE, "def runtime_doctor_render_value(): pass"]]).join("\n"),
  /request-timeline detail semantics must stay in the production Mojo renderer/u);
  assert.match(findViolations([[DOCTOR_LAST_MARKER_LINE_CONSUMER_FILE,
    "pub(super) fn runtime_doctor_truncate_line(line: &str) -> String { line.chars().take(159).collect() }\n"]]).join("\n"),
  /last-marker line truncation must use the Mojo renderer with caller coverage/u);
  assert.match(findViolations([[DOCTOR_RENDER_ADAPTER_FILE, "fn runtime_doctor_render() {}"]]).join("\n"),
  /last-marker line truncation must retain bounded real-Mojo operation 23 and its ABI boundary test/u);
  assert.match(findViolations([[DOCTOR_RENDER_MOJO_FILE, "def runtime_doctor_render_value(): pass"]]).join("\n"),
  /bounded Unicode truncation for timeline details and last-marker lines must stay in the production Mojo renderer/u);
  assert.match(findViolations([[RUNTIME_DOCTOR_PLAN_ADAPTER_FILE,
    "fn input_is_valid(input: &Input) -> bool { true }\n"]]).join("\n"),
  /runtime-doctor fixed-layout input contracts must be validated by Mojo and covered through the real Rust caller/u);
  assert.match(findViolations([[RUNTIME_DOCTOR_PLAN_MOJO_FILE,
    "def runtime_doctor_summary_validate_input(): pass\n"]]).join("\n"),
  /all runtime-doctor plan ABI input contracts, including the bounded marker arena, must be validated in Mojo/u);
  assert.match(findViolations([[CLI_DEFAULT_RUN_CONSUMER_FILE,
    "fn reassemble_super_expose_alias() { prodex_mojo_core::launch::find_super_expose_alias_index(); }\npub fn should_default_cli_invocation_to_run(args: &[OsString]) -> bool {\n  matches!(args.first(), Some(_))\n}\n"]]).join("\n"),
  /CLI default-run decision must use the Mojo launch-arguments policy/u);
  assert.match(findViolations([[CLI_DEFAULT_RUN_CONSUMER_FILE,
    "fn parse_cli_command_from() {}\n"]]).join("\n"),
  /CLI default-run decision must use the Mojo launch-arguments policy/u);
  assert.match(findViolations([[CLI_DEFAULT_RUN_ABI_FILE, "fn default_cli_invocation_to_run() {}"]]).join("\n"),
  /CLI default-run adapter must retain launch Mojo operation 13/u);
  assert.match(findViolations([[CLI_DEFAULT_RUN_MOJO_FILE, "def launch_cli_default_run_policy(): pass"]]).join("\n"),
  /CLI default-run classification must remain in the launch-arguments Mojo kernel/u);
  assert.match(findViolations([[CLI_DEFAULT_RUN_ABI_TEST_FILE, "fn unrelated_test() {}"]]).join("\n"),
  /direct real-Mojo CLI default-run coverage is required/u);
  assert.match(findViolations([[CLI_DEFAULT_RUN_CALLER_TEST_FILE, "fn unrelated_test() {}"]]).join("\n"),
  /CLI default-run behavior must retain caller-boundary coverage/u);
  assert.deepEqual(findViolations([[RESPONSE_METADATA_FILE,
    "fn runtime_response_metadata_from_value(value: &Value) -> Plan {\n  prodex_mojo_core::json::runtime_response_metadata(&nodes, &number_texts)\n}"]]), []);
  assert.match(findViolations([[RESPONSE_METADATA_FILE,
    "fn runtime_response_metadata_from_value(value: &Value) -> Plan {\n  extract_runtime_token_usage_candidate(value)\n}"]]).join("\n"),
  /response metadata decisions must use the Mojo plan without Rust copies/u);
  assert.match(findViolations([[DOCTOR_MARKER_ABI_ADAPTER_FILE,
    "const RUNTIME_DOCTOR_MARKER_ABI_VERSION: i64 = 1; fn prodex_mojo_runtime_doctor_marker_semantics_v1() {}"]]).join("\n"),
  /four-slot marker semantics must use the version-2 ABI/u);
  assert.match(findViolations([[DOCTOR_MARKER_ABI_MOJO_FILE,
    "@export(\"prodex_mojo_runtime_doctor_marker_semantics_v1\") fn old() {}"]]).join("\n"),
  /four-slot marker output must be exported under the version-2 ABI/u);
  assert.match(findViolations([[DOCTOR_MARKER_ABI_TEST_FILE,
    "fn test() { prodex_mojo_runtime_doctor_marker_semantics_v1(); }"]]).join("\n"),
  /marker ABI tests must exercise version 2 and its fourth slot/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/chat_tools_bridge/tools.rs",
    "fn provider_core_chat_tools_from_responses_request() {}"]]).join("\n"),
  /Rust fallback or oracle/u);
  assert.match(findViolations([[GEMINI_BUFFERED_RESPONSE_FILE,
    "fn gemini_insert_response_message() {}"]]).join("\n"),
    /buffered response assembly must use the Mojo kernel/u);
  assert.match(findViolations([[GEMINI_BUFFERED_RESPONSE_FILE,
    '#[cfg(not(feature = "mojo"))] fn old_response() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.deepEqual(findViolations([[GEMINI_BUFFERED_RESPONSE_FILE,
    "fn shape() { gemini_buffered_response_kernel(input); }"]]), []);
  assert.match(findViolations([[FINGERPRINT_ARTIFACTS_FILE,
    "fn smart_context_fingerprint_map() {}"]]).join("\n"),
    /replaced Rust fingerprint map/u);
  assert.match(findViolations([[FINGERPRINT_DELTA_FILE,
    "pub fn smart_context_fingerprint_delta() { smart_context_fingerprint_delta_rust() }"]]).join("\n"),
    /fingerprint delta must use the Mojo plan/u);
  assert.match(findViolations([[DEEPSEEK_REQUEST_FILE,
    "fn deepseek_request_body_from_responses_rust() {}"]]).join("\n"),
    /Rust semantic oracle or copy/u);
  assert.match(findViolations([[DEEPSEEK_REQUEST_REJECT_FILE,
    "fn deepseek_provider_core_validate_modalities(value: &Value) {}"]]).join("\n"),
    /without Rust copies or cfg routing/u);
  assert.match(findViolations([[DEEPSEEK_REQUEST_REJECT_FILE,
    "pub fn deepseek_provider_core_reject_unsupported_request_fields(value: &Value, label: &str) -> Result<(), String> { rust_compat::reject(value, label) }"]]).join("\n"),
    /without Rust copies or cfg routing/u);
  assert.match(findViolations([[DEEPSEEK_REQUEST_REJECT_FILE,
    '#[cfg(not(feature = "mojo"))] fn fallback() {}']]).join("\n"),
    /without Rust copies or cfg routing/u);
  assert.deepEqual(findViolations([[DEEPSEEK_REQUEST_REJECT_FILE,
    `pub fn deepseek_provider_core_reject_unsupported_request_fields(value: &Value, label: &str) -> Result<(), String> {\n  request_policy::try_plan_value(value, DeepSeekRequestPolicyOperation::RequestFields);\n}\npub fn deepseek_provider_core_reject_beta_completion_fields(value: &Value, label: &str) -> Result<(), String> {\n  request_policy::try_plan_value(value, DeepSeekRequestPolicyOperation::BetaFields);\n}`]]), []);
  assert.match(findViolations([[DEEPSEEK_REASONING_FILE,
    "mod rust_compat { fn deepseek_provider_core_reasoning_effort() {} }"]]).join("\n"),
    /reasoning must use Mojo without Rust copies or cfg routing/u);
  assert.deepEqual(findViolations([[DEEPSEEK_REASONING_FILE,
    "request_policy::try_plan_value(DeepSeekRequestPolicyOperation::ReasoningShape); DeepSeekKernelOperation::ReasoningParameters"]]), []);
  assert.match(findViolations([[DEEPSEEK_REQUEST_FILE,
    "fn deepseek_request_body_from_responses() {}"]]).join("\n"),
    /must use the Mojo raw kernel/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/deepseek_bridge/request_params.rs",
    '#[cfg(not(feature = "mojo"))] fn validate_primitive_request_fields_rust() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/deepseek_bridge/request_tools/shape.rs",
    "fn old_validator() {}"]]).join("\n"), /Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_shape.rs",
    '#[cfg(not(feature = "mojo"))] fn old_validator() {}']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/deepseek_bridge/input_items.rs",
    '#[cfg(not(feature = "mojo"))] mod push;']]).join("\n"),
    /feature-off Rust path/u);
  assert.match(findViolations([[DEEPSEEK_INPUT_HISTORY_FILE,
    "pub fn deepseek_provider_core_first_function_call_output_call_id(value: &Value) { value.get(\"input\").find_map(|item| item.get(\"type\")); }\n" +
    "pub fn deepseek_provider_core_history_has_tool_call(history: &[Value], call_id: &str) { history.iter().any(|message| message.get(\"tool_calls\").any(|call| call.get(\"id\") == call_id)); }\n" +
    "pub fn deepseek_provider_core_tool_call_ids() {}"]]).join("\n"),
    /DeepSeek input\/history replay decisions must use the Mojo kernel/u);
  assert.deepEqual(findViolations([[DEEPSEEK_INPUT_HISTORY_FILE,
    "pub fn deepseek_provider_core_first_function_call_output_call_id(value: &Value) { DeepSeekKernelOperation::ResponsesHistoryCallId; }\n" +
    "pub fn deepseek_provider_core_history_has_tool_call(history: &[Value], call_id: &str) { DeepSeekKernelOperation::ResponsesHistoryContainsCallId; }\n" +
    "pub fn deepseek_provider_core_tool_call_ids() {}"]]), []);
  assert.match(findViolations([["crates/prodex-provider-core/src/deepseek_bridge/request_params_tests.rs",
    "fn oracle() {}"]]).join("\n"), /Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/deepseek/request.rs",
    "fn deepseek_request_body_from_responses() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([[MODEL_SPEC_FILE,
    "fn matches_id_or_alias() { self.id.eq_ignore_ascii_case(model) }"]]).join("\n"),
    /model matcher must use the Mojo catalog kernel/u);
  assert.match(findViolations([[
    "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs",
    "let static_models = provider.models();\nfn external_catalog_model_indices_rust() {}",
  ]]).join("\n"), /Rust semantic oracle or copy/u);
  assert.match(findViolations([[
    "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs",
    "let static_models = provider.models();\npub(super) fn external_catalog_model_indices(ids: &[&str]) -> Vec<usize> { ids.iter().map(|id| id.to_ascii_lowercase()).collect() }",
  ]]).join("\n"), /catalog dedup must use Mojo/u);
  assert.match(findViolations([[PROMPT_CACHE_SELECTION_FILE,
    '#[path = "selection_prompt_cache_rust.rs"] mod prompt_cache;']]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/request/schema/composition.rs",
    "fn collapse_schema_union() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/kiro/request/validation.rs",
    "fn old_kiro_validation() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/openai_chat_compat_request.rs",
    "fn old_chat_request() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([[SUPER_OVERRIDE_FILE,
    '#[cfg(not(feature = "mojo-core"))] fn old_scan() {}']])[0], /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-cli/Cargo.toml",
    'prodex_mojo_core = { workspace = true, optional = true }']])[0], /requires Mojo/u);
  assert.match(findViolations([[DOCTOR_CARGO_FILE,
    'prodex_mojo_core = { workspace = true, optional = true }']])[0], /require Mojo/u);
  assert.match(findViolations([["crates/prodex-runtime-doctor/src/diagnosis/next_steps.rs",
    '#[cfg(not(feature = "mojo"))] fn old_next_step() {}']])[0], /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-runtime-doctor/src/suggestions/compatibility.rs",
    "fn old_suggestion() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-runtime-doctor/src/diagnosis/final_summary/default_diagnosis.rs",
    "fn old_diagnosis() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/kiro/stream.rs",
    '#[cfg(not(feature = "mojo"))] fn old_stream() {}']])[0], /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/errors.rs",
    'fn classify_provider_error_rust() {}']])[0], /Rust semantic oracle or copy/u);
  assert.match(findViolations([[PROVIDER_ERROR_FILE,
    'pub fn provider_error_rejects_request_member() { fn mentions_member() {} }']]).join("\n"),
  /request-member rejection must use Mojo/u);
  assert.match(findViolations([[PROVIDER_ERROR_FILE,
    'pub fn provider_error_rejects_request_member() { false }']]).join("\n"),
  /request-member rejection must use Mojo/u);
  assert.deepEqual(findViolations([[PROVIDER_ERROR_FILE,
    '#[repr(i64)]\npub enum ProviderErrorClass { Other }\npub fn provider_error_rejects_request_member() {\n  prodex_mojo_core::json::provider_error_rejects_member(nodes, raw, member);\n}']]), []);
  assert.match(findViolations([["crates/prodex-runtime-tuning/src/capacity.rs",
    "fn runtime_proxy_worker_count_default_rust() {}"]])[0], /Rust semantic oracle or copy/u);
  assert.match(findViolations([["crates/prodex-runtime-tuning/src/lib.rs",
    '#[cfg(feature = "mojo")] mod mojo;']]).join("\n"), /must be unconditional/u);
  assert.match(findViolations([[RUNTIME_TUNING_CARGO_FILE,
    'prodex_mojo_core = { workspace = true, optional = true }']]).join("\n"), /unconditional Mojo runtime dependency|must not be optional/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/smart_context/rollout.rs",
    "fn smart_context_rollout_decision_rust() {}"]])[0], /Rust semantic oracle or copy/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/smart_context/normalization.rs",
    "fn smart_context_normalize_volatile_command_output_rust() {}"]])[0],
    /Rust semantic oracle or copy/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/selection_plan.rs",
    "fn runtime_optimistic_current_candidate_decision_rust() {}"]])[0],
    /Rust semantic oracle or copy/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/selection_plan.rs",
    "pub enum RuntimeOptimisticCurrentCandidateSkipReason { AuthFailureBackoff }\nOPTIMISTIC_CANDIDATE_AUTH_FAILURE"]]).join("\n"),
    /restored Rust candidate skip-reason semantics/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/smart_context/normalization/static_context.rs",
    "fn smart_context_static_context_noise_key(key: &str) {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/smart_context/static_context.rs",
    'fn smart_context_stabilize_static_context_items_bounded() { #[cfg(not(feature = "mojo"))] old_sort(); }\nfn smart_context_reduce_static_context_items_mojo(']]).join("\n"),
    /static-item selection must use Mojo/u);
  assert.match(findViolations([["crates/prodex-runtime-policy/src/types/runtime_proxy_preset.rs",
    "runtime_tuning_proxy_preset_plan(\n); fn resolve_rust() {}"]])[0], /Rust semantic oracle or copy/u);
  const presetConsumer = "runtime_tuning_proxy_preset_plan(\n);";
  assert.deepEqual(findViolations([[RUNTIME_POLICY_PRESET_CONSUMER_FILE, presetConsumer]]), []);
  assert.match(findViolations([[RUNTIME_POLICY_PRESET_CONSUMER_FILE,
    presetConsumer.replace("runtime_tuning_proxy_preset_plan(\n);", "apply_non_preset_overrides();")]]).join("\n"),
  /must use the Mojo plan|restored Rust preset precedence or override merging/u);
  assert.match(findViolations([[PRECOMMIT_BUDGET_FILE,
    "fn runtime_proxy_precommit_budget_for_profile_count_rust() {}"]])[0],
    /Rust semantic oracle or copy/u);
  assert.match(findViolations([[PRECOMMIT_BUDGET_TEST_FILE,
    "fn precommit_budget_matches_rust_oracle() {}"]])[0],
    /Rust pre-commit budget oracle/u);
  assert(findViolations([[DEEPSEEK_SHAPING_FILE,
    'pub fn deepseek_provider_core_response_created_event() { #[cfg(not(feature = "mojo"))] fallback(); }']])
    .some((violation) => violation.includes("deepseek_provider_core_response_created_event contains a feature-off Rust path")));
  const deepseekChoiceDelta = `pub fn deepseek_provider_core_stream_choice_delta(choice: &Value) -> DeepSeekProviderCoreStreamChoiceDelta {
    deepseek_provider_core_stream_projection(DeepSeekKernelOperation::StreamChoiceDelta, choice)
}`;
  assert(!findViolations([[DEEPSEEK_SHAPING_FILE, deepseekChoiceDelta]])
    .some((violation) => violation.includes("stream choice empty-text filtering must stay in Mojo")));
  assert(findViolations([[DEEPSEEK_SHAPING_FILE,
    deepseekChoiceDelta.replace("choice)", 'choice); if value.as_deref() == Some("") { None }')]])
    .some((violation) => violation.includes("stream choice empty-text filtering must stay in Mojo")));
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/openai_chat_compat_response/stream.rs",
    "fn translate_chat_stream_value_to_responses_rust() {}"]])[0], /Rust semantic oracle or copy/u);
  assert.match(findViolations([[ANTHROPIC_MESSAGES_FILE,
    "fn anthropic_tool_choice() {}"]])[0], /Rust Anthropic request semantics/u);
  assert.match(findViolations([[ANTHROPIC_WEB_SEARCH_FILE,
    "fn anthropic_web_search_tool() {}"]])[0], /Rust Anthropic request web-search semantics/u);
  assert.match(findViolations([[ANTHROPIC_SSE_FILE,
    "fn anthropic_web_search_stream_sources() {}"]])[0], /web-search shaping must use Mojo/u);
  assert.match(findViolations([[ANTHROPIC_MESSAGES_FILE,
    "Anthropic Messages web-search result translation requires Mojo support"]])[0],
  /feature-off rejection/u);
  assert.match(findViolations([[SUPER_EXPOSE_PROTOCOL_FILE,
    "enum ExposeMethod { Ping }\nenum ExposeTool { Exec }\nprodex_mojo_core::rich::super_expose_protocol_version_supported(\"x\");"]]).join("\n"),
    /restored Rust Super-expose route or label mirror|must retain Mojo-owned surface/u);
  assert.match(findViolations([[CLI_RUNTIME_FEATURE_FILE, "fn rust_plan() {}"]])[0],
    /Rust runtime-feature planner or oracle/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/fallback/chains/gemini.rs",
    "fn provider_gemini_model_fallback_alias_chain() {}"]])[0], /Rust Gemini model fallback table/u);
  assert(findViolations([[GEMINI_BRIDGE_REQUEST_FILE,
    'pub fn gemini_provider_core_generate_content_body_value() {\n#[cfg(not(feature = "mojo"))]\nold_body();\n}']])
    .some((violation) => violation.includes("gemini_provider_core_generate_content_body_value must use Mojo")));
  for (const symbol of ["stream_chunk_metadata", "stream_tool_call_ids", "stream_tool_call_added_item"]) {
    assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/stream/shaping.rs",
      `pub fn gemini_provider_core_${symbol}() {\n old_rust_selection();\n}`]]).join("\n"),
      /must stay Mojo-owned without Rust selection/u);
  }
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/stream/shaping.rs",
    '#[cfg(feature = "mojo")] fn gated_shape() {}']]).join("\n"),
    /stream shaping Mojo kernel must be unconditional/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/stream/shaping.rs",
    "pub fn gemini_provider_core_stream_completed_tool_call_item() {\n  GeminiResponseKernelOperation::ToolSearchCallItem\n}"]]).join("\n"),
  /replaced Rust completed tool-call shaping branches/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/stream/shaping.rs",
    "pub fn gemini_provider_core_function_call_arguments_delta_event_with_thought_signature(event: Value) {\n  event[\"thought_signature\"] = Value::String(\"x\".into());\n}"]]).join("\n"),
  /thought-signature shaping must use the Gemini Mojo kernel|restored Rust function-call delta thought-signature shaping/u);
  assert.match(findViolations([[GEMINI_GENERATION_CONFIG_FILE,
    "fn gemini_generation_config_from_request() {}"]])[0], /duplicate Gemini generation-config adapter/u);
  assert.match(findViolations([[GEMINI_TOOL_RESPONSE_ORDER_FILE,
    "fn gemini_provider_core_refine_tool_response_order() {\n  response_parts.sort_by_key(rank);\n}"]]).join("\n"),
  /Gemini tool-response ordering must use Mojo|restored Rust Gemini tool-response ordering policy/u);
  assert.deepEqual(findViolations([[GEMINI_TOOL_RESPONSE_ORDER_FILE,
    "fn gemini_provider_core_refine_tool_response_order() {\n  gemini_tool_response_part_order(ids, parts);\n}"]]), []);
  assert.match(findViolations([[GEMINI_TOOL_RESPONSE_ORDER_ADAPTER_FILE,
    "fn gemini_tool_response_part_order() {}"]]).join("\n"), /must retain the Mojo ABI call/u);
  assert.match(findViolations([[GEMINI_TOOL_RESPONSE_ORDER_MOJO_FILE,
    "fn unrelated_gemini_policy() {}"]]).join("\n"), /must be implemented in Mojo/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/request/optional_fields.rs",
    "fn gemini_apply_optional_request_fields() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([[ANTHROPIC_MESSAGES_FILE,
    '#[cfg(not(feature = "mojo"))] fn existing_path() { Some("text") => () }',
  ]]).join("\n"), /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/anthropic/messages/stream.rs",
    '#[cfg(not(feature = "mojo"))] fn existing_path() { Some("text") => () }',
  ]])[0], /Mojo-owned operation cannot have a feature-off Rust path/u);
  const providerUsageFile = "crates/prodex-provider-core/src/usage.rs";
  const providerUsageCalls = [
    "prodex_mojo_core::provider_usage::extract_json(",
    "prodex_mojo_core::provider_usage::calculate_cost(",
    "prodex_mojo_core::provider_usage::merged_total(",
    "prodex_mojo_core::provider_usage::merge_latest_present(",
  ].join("\n");
  assert.deepEqual(findViolations([[providerUsageFile, providerUsageCalls]]), []);
  assert.match(
    findViolations([[providerUsageFile,
      providerUsageCalls + "\nif usage.input_tokens.is_some() { merged.input_tokens = usage.input_tokens; }",
    ]]).join("\n"),
    /restored Rust SSE usage merge policy/u,
  );
  const capsuleOrderBody = [
    "pub(in crate::smart_context) fn smart_context_select_memory_capsules_impl(input: Vec<Capsule>) {",
    "    let order = prodex_mojo_core::rich::order_smart_context_capsules(&inputs);",
    "    for batch in capsules.chunks(65_536) {}",
    "}",
  ].join("\n");
  assert.deepEqual(findViolations([[SMART_CONTEXT_CAPSULE_ORDER_FILE, capsuleOrderBody]]), []);
  assert.match(findViolations([[SMART_CONTEXT_CAPSULE_ORDER_FILE,
    capsuleOrderBody.replace("order_smart_context_capsules", "capsules.sort_by") + "\n    capsules.sort_by(order);",
  ]]).join("\n"), /restored Rust memory-capsule ordering policy/u);
  assert.match(findViolations([[SMART_CONTEXT_CAPSULE_ORDER_FILE,
    capsuleOrderBody.replace(
      "let order = prodex_mojo_core::rich::order_smart_context_capsules(&inputs);",
      '#[cfg(not(feature = "mojo"))] let order = rust_order(&inputs);',
    ),
  ]]).join("\n"), /feature-off Rust path/u);
  assert.match(findViolations([[SMART_CONTEXT_CAPSULE_ORDER_ADAPTER_FILE,
    "pub fn order_smart_context_capsules() {}"]]).join("\n"), /versioned Mojo ABI/u);
  const capsuleOrderAdapter = [
    "const SMART_CONTEXT_CAPSULE_ORDER_MAX_COUNT: usize = 65_537;",
    "id_addresses.push(mojo_pointer_address(input.id.as_ptr()));",
    "id_lengths.push(i64::try_from(input.id.len()).map_err(|_| MojoError::InvalidInput)?);",
    "prodex_mojo_smart_context_capsule_order_v1(",
  ].join("\n");
  assert.deepEqual(findViolations([[SMART_CONTEXT_CAPSULE_ORDER_ADAPTER_FILE, capsuleOrderAdapter]]), []);
  assert.match(findViolations([[SMART_CONTEXT_CAPSULE_ORDER_ADAPTER_FILE,
    capsuleOrderAdapter.replace("input.id.as_ptr()", "input.id.as_bytes().as_ptr()"),
  ]]).join("\n"), /must use count-bounded per-ID pointers and checked lengths/u);
  assert.match(findViolations([[SMART_CONTEXT_CAPSULE_ORDER_ADAPTER_FILE,
    `${capsuleOrderAdapter}\nconst SMART_CONTEXT_CAPSULE_ORDER_MAX_ID_BYTES = 4 * 1024 * 1024;`,
  ]]).join("\n"), /must not cap or copy aggregate ID bytes/u);
  const capsuleOrderMojo = [
    '@export("prodex_mojo_smart_context_capsule_order_v1")',
    "SMART_CONTEXT_CAPSULE_ORDER_MAX_COUNT: Int64 = 65_537",
    "def smart_context_capsule_order_before(",
    "id_addresses_address: UInt",
    "id_lengths_address: UInt",
    "length < 0",
    "(length > 0 and address == 0)",
  ].join("\n");
  assert.deepEqual(findViolations([[SMART_CONTEXT_CAPSULE_ORDER_MOJO_FILE, capsuleOrderMojo]]), []);
  assert.match(findViolations([[SMART_CONTEXT_CAPSULE_ORDER_MOJO_FILE,
    `${capsuleOrderMojo}\ncomptime SMART_CONTEXT_CAPSULE_ORDER_MAX_ID_BYTES = 4 * 1024 * 1024`,
  ]]).join("\n"), /must not cap aggregate ID bytes/u);
  assert.match(findViolations([[SMART_CONTEXT_CAPSULE_ORDER_MOJO_FILE,
    '@export("prodex_mojo_smart_context_capsule_order_v1")']]).join("\n"), /must remain Mojo-owned/u);
}

async function main() {
  if (process.argv.includes("--self-test")) selfTest();
  const violations = findViolations(await promotedFiles());
  if (violations.length > 0) throw new Error(violations.join("\n"));
  process.stdout.write("mojo no-fallback guard: ok\n");
}

main().catch((error) => {
  process.stderr.write(`mojo no-fallback guard: ${error.message}\n`);
  process.exitCode = 1;
});
