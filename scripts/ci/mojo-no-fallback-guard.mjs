#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const PRECOMMIT_BUDGET_FILE = "crates/prodex-runtime-proxy/src/failure_response.rs";
const PRECOMMIT_BUDGET_TEST_FILE = "crates/prodex-runtime-proxy/tests/src/failure_response.rs";
const PROMOTED_FILES = [
  "crates/prodex-app/src/app_commands/log_throughput_state.rs",
  "crates/prodex-mojo-core/src/log_throughput_policy.rs",
  "crates/prodex-runtime-broker/src/version_guard.rs",
  "crates/prodex-mojo-core/src/super_provider_config.rs",
  "crates/prodex-app/src/runtime_deepseek_config.rs",
  "crates/prodex-cli/src/runtime_args.rs",
  "crates/prodex-app/src/super_expose/protocol/dispatch.rs",
  "crates/prodex-app/src/app_commands/log_transcript.rs",
  "crates/prodex-mojo-core/src/sub_agent_policy.rs",
  "crates/prodex-cli/src/sub_agent.rs",
  "crates/prodex-app/src/runtime_tools/sub_agents.rs",
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
  "crates/prodex-update-notice/src/lib.rs",
  "crates/prodex-shared-codex-fs/src/image_attachments.rs",
  "crates/prodex-runtime-cookies/src/lib.rs",
  "crates/prodex-mojo-core/src/runtime_cookie_policy.rs",
  "crates/prodex-mojo-core/src/shared_attachment_policy.rs",
  "crates/prodex-mojo-core/src/update_notice_policy.rs",
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
  "crates/prodex-mojo-core/src/smart_context_markers.rs",
  "crates/prodex-app/src/runtime_state_shared/semantic_index/markers.rs",
  "crates/prodex-mojo-core/src/smart_context_artifact_ref.rs",
  "crates/prodex-app/src/runtime_proxy/smart_context/artifact_manifest.rs",
  "crates/prodex-app/src/runtime_proxy/smart_context/artifact_refs.rs",
  "crates/prodex-mojo-core/src/json.rs",
  "crates/prodex-session-store/src/session_selector.rs",
  "crates/prodex-session-store/src/report.rs",
  "crates/prodex-mojo-core/src/runtime_lineage.rs",
  "crates/prodex-runtime-state/src/lineage.rs",
  "crates/prodex-mojo-core/src/runtime_repo_map.rs",
  "crates/prodex-app/src/runtime_state_shared/line_index.rs",
  "crates/prodex-mojo-core/src/profile_export.rs",
  "crates/prodex-profile-export/src/envelope.rs",
  "crates/prodex-profile-export/src/data_model.rs",
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
  "crates/prodex-quota/src/models.rs",
  "crates/prodex-quota/src/auth.rs",
  "crates/prodex-quota/src/render/time.rs",
  "crates/prodex-profile-identity/src/lib.rs",
  "crates/prodex-mojo-core/src/profile_identity.rs",
  "crates/prodex-domain/src/governance/inspection.rs",
  "crates/prodex-mojo-core/build.rs",
  "crates/prodex-mojo-core/src/lib.rs",
  "crates/prodex-mojo-core/src/quota.rs",
  "crates/prodex-mojo-core/src/routing.rs",
  "crates/prodex-mojo-core/src/runtime.rs",
  "crates/prodex-mojo-core/src/runtime/candidate_plan.rs",
  "crates/prodex-mojo-core/src/runtime/auto_redeem.rs",
  "crates/prodex-mojo-core/src/runtime_decisions.rs",
  "crates/prodex-mojo-core/tests/profile_health.rs",
  "crates/prodex-mojo-core/src/provider_constraints.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline_dispatch/provider_precommit.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream/error_class.rs",
  "crates/prodex-mojo-core/src/policy.rs",
  "crates/prodex-mojo-core/src/context.rs",
  "crates/prodex-mojo-core/src/rich.rs",
  "crates/prodex-mojo-core/src/rich/catalog.rs",
  "crates/prodex-mojo-core/src/rich/catalog_planner.rs",
  "crates/prodex-mojo-core/src/rich/context_plan.rs",
  "crates/prodex-mojo-core/src/log.rs",
  "crates/prodex-mojo-core/src/rich/routing.rs",
  "crates/prodex-context/src/critical_signal.rs",
  "crates/prodex-quota/src/render/gemini.rs",
  "crates/prodex-quota/src/capacity.rs",
  "crates/prodex-quota/src/render/windows.rs",
  "crates/prodex-quota/src/render/model_capacity.rs",
  "crates/prodex-context/src/critical_signal.rs",
  "crates/prodex-app/src/app_commands/status.rs",
  "crates/prodex-terminal-ui/src/info.rs",
  "crates/prodex-mojo-core/src/info_render.rs",
  "crates/prodex-app/src/runtime_external_provider_config.rs",
  "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs",
  "crates/prodex-app/src/super_expose/protocol.rs",
  "crates/prodex-app/src/super_expose/openai_tunnel.rs",
  "crates/prodex-app/src/app_commands/log_event_source.rs",
  "crates/prodex-app/src/app_commands/log_stream.rs",
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
  "crates/prodex-runtime-policy/src/types/runtime_proxy_preset.rs",
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
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/strict_schema.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo_tests.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items.rs",
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
  "crates/prodex-provider-core/src/translators/kiro/request/semantics_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/stream.rs",
  "crates/prodex-provider-core/src/translators/kiro/response.rs",
  "crates/prodex-provider-core/src/translators/kiro/acp.rs",
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
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/strict_schema.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_choice.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_shape.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/web_search.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/entry.rs",
  "crates/prodex-provider-core/src/chat_tools_bridge/mojo.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/request.rs",
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
  "crates/prodex-provider-core/src/deepseek_bridge/request_tools/strict_schema.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo_tests.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items.rs",
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
const SUB_AGENT_POLICY_ADAPTER_FILE = "crates/prodex-mojo-core/src/sub_agent_policy.rs";
const SUB_AGENT_CHILD_FILE = "crates/prodex-app/src/runtime_tools/sub_agents.rs";
const RUNTIME_OVERLAY_POLICY_FILE = "crates/prodex-app/src/runtime_tools/overlay.rs";
const RUNTIME_OVERLAY_POLICY_ADAPTER_FILE = "crates/prodex-mojo-core/src/runtime_overlay_policy.rs";
const CLI_RUNTIME_FEATURE_FILE = "crates/prodex-cli/src/runtime_features.rs";
const DOCTOR_CARGO_FILE = "crates/prodex-runtime-doctor/Cargo.toml";
const RUNTIME_PROXY_CARGO_FILE = "crates/prodex-runtime-proxy/Cargo.toml";
const RUNTIME_TUNING_CARGO_FILE = "crates/prodex-runtime-tuning/Cargo.toml";
const QUOTA_MODELS_FILE = "crates/prodex-quota/src/models.rs";
const QUOTA_AUTH_FILE = "crates/prodex-quota/src/auth.rs";
const QUOTA_TIME_FILE = "crates/prodex-quota/src/render/time.rs";
const QUOTA_ADAPTER_FILE = "crates/prodex-mojo-core/src/quota.rs";
const QUOTA_WINDOWS_FILE = "crates/prodex-quota/src/render/windows.rs";
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
const LOG_STREAM_FILE = "crates/prodex-app/src/app_commands/log_stream.rs";
const LOG_EVENT_SOURCE_FILE = "crates/prodex-app/src/app_commands/log_event_source.rs";
const OBSERVABILITY_ADAPTER_FILE = "crates/prodex-mojo-core/src/observability.rs";
const GEMINI_SCHEMA_FILE = "crates/prodex-provider-core/src/translators/gemini/request/schema.rs";
const GEMINI_TOOLS_FILE = "crates/prodex-provider-core/src/translators/gemini/request/tools.rs";
const GEMINI_STATUS_FILE = "crates/prodex-provider-core/src/translators/gemini/response/status.rs";
const GEMINI_BUFFERED_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/gemini/response/build.rs";
const RESPONSE_FORWARDING_FILE = "crates/prodex-runtime-proxy/src/response_forwarding.rs";
const QUOTA_POOL_FILE = "crates/prodex-quota/src/render/pool.rs";
const STATUS_SUMMARY_FILE = "crates/prodex-app/src/app_commands/status.rs";
const QUOTA_MODEL_CAPACITY_FILE = "crates/prodex-quota/src/render/model_capacity.rs";
const RUNTIME_QUOTA_FILE = "crates/prodex-runtime-proxy/src/quota.rs";
const SELECTION_POLICY_FILE = "crates/prodex-runtime-proxy/src/selection_policy.rs";
const HEALTH_ABI_TEST_FILE = "crates/prodex-mojo-core/tests/profile_health.rs";
const DEEPSEEK_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/deepseek/response.rs";
const DEEPSEEK_RESPONSE_TOOL_CALLS_FILE = "crates/prodex-provider-core/src/translators/deepseek/tooling/response_tool_calls.rs";
const DEEPSEEK_REQUEST_FILE = "crates/prodex-provider-core/src/translators/deepseek/request_transform.rs";
const DEEPSEEK_REQUEST_REJECT_FILE = "crates/prodex-provider-core/src/deepseek_bridge/request_params/reject.rs";
const DEEPSEEK_REASONING_FILE = "crates/prodex-provider-core/src/deepseek_bridge/request_params/reasoning.rs";
const DEEPSEEK_METADATA_FILE = "crates/prodex-provider-core/src/deepseek_bridge/request_params/metadata.rs";
const DEEPSEEK_SIMPLE_REQUEST_FILE = "crates/prodex-provider-core/src/deepseek_bridge/request_probe.rs";
const KIRO_CHAT_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/kiro/response.rs";
const KIRO_ACP_FILE = "crates/prodex-provider-core/src/translators/kiro/acp.rs";
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
const PROVIDER_CONSTRAINTS_ADAPTER_FILE = "crates/prodex-mojo-core/src/provider_constraints.rs";
const PROVIDER_PRECOMMIT_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline_dispatch/provider_precommit.rs";
const LOCAL_REWRITE_UPSTREAM_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream.rs";
const NATIVE_FIRST_ERROR_CLASS_FILE = "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream/error_class.rs";
const MODEL_SPEC_FILE = "crates/prodex-provider-core/src/surface/models.rs";
const PROMPT_CACHE_SELECTION_FILE = "crates/prodex-runtime-proxy/src/selection_plan.rs";
const FINGERPRINT_DELTA_FILE = "crates/prodex-runtime-proxy/src/smart_context/static_context.rs";
const FINGERPRINT_ARTIFACTS_FILE = "crates/prodex-runtime-proxy/src/smart_context/normalization/artifacts.rs";
const SMART_CONTEXT_CORE_FILE = "crates/prodex-runtime-proxy/src/smart_context/core.rs";
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
  const markerViolations = files.flatMap(([filePath, contents]) =>
    FORBIDDEN_MARKERS.filter((marker) => contents.includes(marker)).map(
      (marker) => `${filePath}: promoted Mojo code contains ${marker}`,
    ),
  );
  const featureOffViolations = files
    .filter(([filePath, contents]) =>
      UNCONDITIONAL_MOJO_FILES.has(filePath) && FEATURE_OFF_RUST_PATH.test(contents),
    )
    .map(([filePath]) => `${filePath}: Mojo-owned operation cannot have a feature-off Rust path`);
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
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/provider_usage.rs") {
      const required = [
        "prodex_provider_usage_extract_v1(",
        "prodex_provider_usage_cost_v1(",
        "prodex_provider_usage_merged_total_v1(",
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
        "prodex_mojo_core::audit_log_policy::summarize_usage(",
        "prodex_mojo_core::audit_log_policy::budget_flags(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": audit usage hard replacement must retain " + call);
      if (
        /summary\.(?:requests|total_tokens|cost_micros)\s*>=/u.test(contents)
        || /self\.input_tokens\s*\.saturating_add/u.test(contents)
        || /\.chars\(\)[^;]{0,300}ch\.is_ascii_alphanumeric/u.test(contents)
      ) {
        violations.push(filePath + ": contains restored Rust audit-usage semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/audit_log_policy.rs") {
      const required = [
        "prodex_audit_usage_token_normalize_v1(",
        "prodex_audit_usage_total_v1(",
        "prodex_audit_usage_summary_v1(",
        "prodex_audit_budget_flags_v1(",
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
    if (filePath === "crates/prodex-update-notice/src/lib.rs") {
      const required = [
        "update_notice_policy::should_emit_notice(",
        "update_notice_policy::install_channel(",
        "update_notice_policy::cache_is_fresh(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": update-notice hard replacement must retain " + call);
      if (
        /replace\('\\\\',\s*"\/"\)/u.test(contents)
        || /normalized_path\.contains/u.test(contents)
        || /now\.saturating_sub\(cached_checked_at\)/u.test(contents)
      ) {
        violations.push(filePath + ": contains restored Rust update-notice semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/update_notice_policy.rs" &&
        !contents.includes("prodex_update_notice_policy_v1(")) {
      return [filePath + ": update-notice ABI adapter must retain prodex_update_notice_policy_v1("];
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
  const deepseekStrictSchemaViolations = files.flatMap(([filePath, contents]) => {
    if (filePath === DEEPSEEK_STRICT_TOOLS_FILE &&
      !contents.includes("DeepSeekKernelOperation::StrictFunctionSchema")) {
      return [`${filePath}: strict schema normalization must use Mojo`];
    }
    if (filePath === DEEPSEEK_STRICT_SCHEMA_FILE &&
      /\bfn\s+deepseek_provider_core_sanitize_strict_(?:schema|object_schema)\s*\(/u.test(contents)) {
      return [`${filePath}: contains replaced Rust strict schema normalization`];
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
    if (filePath === "crates/prodex-app/src/app_commands/log_throughput_state.rs") {
      const required = [
        "prodex_mojo_core::log_throughput_policy::sample_plan(",
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
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": log-throughput ABI adapter must retain " + call);
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
    ];
    const violations = required
      .filter((call) => !contents.includes(call))
      .map((call) => `${filePath}: Codex config migration must retain Mojo call ${call}`);
    if (/\bfn\s+(?:parse_config_override_string|parse_config_override_exact_string)\s*\(/u.test(contents) ||
        /while\s+index\s*<\s*args\.len\(\)/u.test(contents)) {
      violations.push(`${filePath}: contains retired Rust Codex config argument scanning semantics`);
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
    if (filePath !== REDACTION_FILE) return [];
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
  });
  const governanceInspectionViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== "crates/prodex-domain/src/governance/inspection.rs") return [];
    const required = [
      "governance_finding_minimum_classification(",
      "governance_findings_exceed_classification(",
    ];
    return required
      .filter((call) => !contents.includes(call))
      .map((call) => `${filePath}: governance classification must retain Mojo call ${call}`);
  });
  const profileIdentityViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== PROFILE_IDENTITY_FILE) return [];
    const functions = [
      ["find_matching_profile_identity", "mojo_profile_identity::find_matching_profile_identity("],
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
        "external_catalog_static_models(",
        "external_catalog_model_metadata(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": external-provider catalog migration must retain Mojo call " + call);
      if (
        /fn\s+models\(self\)\s*->\s*&'static\s*\[/u.test(contents)
        || /Self::Anthropic\s*=>\s*&\[/u.test(contents)
        || /Self::Copilot\s*=>\s*&\[/u.test(contents)
        || /Self::Kiro\s*=>\s*&\[/u.test(contents)
        || /resolve_catalog_model_exact\(&catalog,\s*model\)/u.test(contents)
      ) {
        violations.push(filePath + ": contains restored Rust external-provider static catalog semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs") {
      return contents.includes("let static_models = provider.models();")
        ? []
        : [filePath + ": external-provider catalog builder must consume Mojo-owned static models"];
    }
    return [];
  });
  const subAgentPolicyViolations = files.flatMap(([filePath, contents]) => {
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
    if (filePath === SUB_AGENT_POLICY_ADAPTER_FILE) {
      const required = [
        "prodex_sub_agent_policy_v1(",
        "prodex_sub_agent_child_argv_plan_v1(",
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": sub-agent policy adapter must retain Mojo ABI call " + call);
    }
    if (filePath === SUB_AGENT_CHILD_FILE) {
      const violations = contents.includes("child_argv_plan(")
        ? []
        : [filePath + ": sub-agent child argv construction must retain Mojo planner"];
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const retired of [
        "let mut args = vec![OsString::from(\"s\"), OsString::from(\"--no-sub-agent\")];",
        "args.push(OsString::from(if spec.presidio_enabled {",
        "match spec.provider {",
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
      const required = "prodex_mojo_core::rich::super_expose_protocol_version_supported(";
      const violations = contents.includes(required)
        ? [] : [filePath + ": Super-expose initialize version policy must retain Mojo call " + required];
      if (contents.includes("MCP_PROTOCOL_VERSIONS.contains(&version)")) {
        violations.push(filePath + ": contains restored Rust MCP protocol-version membership semantics");
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
    if (filePath === "crates/prodex-provider-core/src/translators/gemini/stream/shaping.rs" &&
        /#\[cfg\(feature = "mojo"\)\]/u.test(contents)) {
      return [filePath + ": Gemini stream shaping Mojo kernel must be unconditional"];
    }
    if (filePath === GEMINI_BRIDGE_ROOT_FILE &&
        /#\[cfg\(feature = "mojo"\)\]\s*pub\(crate\) use self::request::\{/u.test(contents)) {
      return [`${filePath}: Gemini translator bridge export must be unconditional`];
    }
    if (filePath === GEMINI_BRIDGE_REQUEST_CONTENTS_FILE &&
        /#\[cfg\(feature = "mojo"\)\]\s*(?:#\[[^\]]+\]\s*)?(?:pub\(crate\) struct GeminiTranslatorValidationPlan|pub\(crate\) fn gemini_bridge_(?:validate_translator|raw_translator_request))/u.test(contents)) {
      return [`${filePath}: Gemini translator bridge helpers must be unconditional`];
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
    if (filePath === "crates/prodex-mojo-core/src/info_render.rs" &&
        !contents.includes("prodex_terminal_info_render_v1(")) {
      return [filePath + ": terminal info ABI adapter must retain prodex_terminal_info_render_v1("];
    }
    return [];
  });
  const doctorMarkerViolations = files
    .filter(([filePath, contents]) => filePath === RUNTIME_DOCTOR_MARKERS_FILE &&
      (!contents.includes("runtime_doctor_marker_known(") ||
        /\b(?:runtime_doctor_marker_registry|RuntimeDoctorMarker|RUNTIME_DOCTOR_MARKERS)\b/u.test(contents)))
    .map(([filePath]) => `${filePath}: marker recognition must use the Mojo classifier`);
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
    return DEEPSEEK_SHAPING_COMPLETED_FNS.flatMap((name) => {
      const start = contents.indexOf(`pub fn ${name}(`);
      if (start < 0) return [`${filePath}: missing Mojo-owned ${name}`];
      const next = contents.indexOf("\npub fn ", start + 1);
      return FEATURE_OFF_RUST_PATH.test(contents.slice(start, next < 0 ? undefined : next))
        ? [`${filePath}: ${name} contains a feature-off Rust path`] : [];
    });
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
    if (filePath === DEEPSEEK_RESPONSE_METADATA_FILE &&
        !contents.includes("DeepSeekKernelOperation::ResponseMetadata")) {
      return [`${filePath}: DeepSeek response metadata must use Mojo`];
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
        "prodex_mojo_core::runtime_route_reason::normalize_unknown(",
      ];
      const violations = required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": route-decision reason migration must retain Mojo call " + call);
      if (
        contents.includes("VALUES.iter().copied().find(|value| value.as_str() == label)")
        || contents.includes("Self::AuthFailureBackoff | Self::AuthNotQuotaCompatible")
        || contents.includes("ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'")
      ) {
        violations.push(filePath + ": contains restored Rust route-reason semantics");
      }
      return violations;
    }
    if (filePath === "crates/prodex-mojo-core/src/runtime_route_reason.rs") {
      const required = [
        "prodex_runtime_route_reason_lookup_v1(",
        "prodex_runtime_route_reason_stage_v1(",
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
      ];
      return required
        .filter((call) => !contents.includes(call))
        .map((call) => filePath + ": runtime lineage ABI adapter must retain " + call);
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
  return [...markerViolations, ...deepseekCatalogPolicyViolations, ...featureOffViolations, ...logThroughputViolations, ...operationalDetailSpecViolations, ...transcriptPolicyViolations, ...routeReasonViolations, ...runtimeStateQuotaViolations, ...runtimeProxyRootViolations, ...brokerVersionGuardViolations, ...brokerContinuityViolations, ...brokerLogCacheViolations, ...codexConfigViolations, ...statePolicyViolations, ...quotaSelectionPolicyViolations, ...runtimeStateBackgroundViolations, ...redactionViolations, ...profileIdentityViolations, ...governanceInspectionViolations, ...exactnessPlannerViolations,
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
    ...kiroResponseHelperViolations,
    ...kiroAcpViolations,
    ...deepseekStrictSchemaViolations,
    ...quotaModelPolicyViolations, ...quotaDisplayPolicyViolations, ...quotaPlannerViolations,
    ...anthropicResponseViolations,
    ...anthropicEnvelopeViolations, ...anthropicRequestViolations,
    ...anthropicWebSearchViolations, ...superProviderConfigViolations, ...externalProviderCatalogViolations, ...subAgentPolicyViolations, ...runtimeOverlayPolicyViolations, ...cliRuntimeFeatureViolations,
    ...superExposeProtocolViolations, ...superExposeViolations,
    ...geminiFallbackViolations, ...geminiGenerationViolations, ...geminiTranslatorHardReplacementViolations, ...geminiBridgeFallbackViolations,
    ...hardReplacementViolations, ...precommitBudgetOracleViolations,
    ...deepseekRequestViolations, ...deepseekRequestRejectViolations,
    ...deepseekReasoningViolations,
    ...nativeFirstErrorClassViolations, ...providerPrecommitPolicyViolations, ...providerErrorMemberViolations,
    ...deepseekResponseToolCallViolations, ...chatToolViolations,
    ...infoRenderViolations, ...doctorMarkerViolations, ...statusSummaryViolations,
    ...geminiBufferedResponseViolations, ...fingerprintDeltaViolations, ...profileExportPolicyViolations, ...sessionReportViolations, ...runtimeLineageViolations, ...smartContextMarkerViolations, ...smartContextArtifactRefViolations, ...runtimeRepoMapViolations,
    ...modelSpecViolations, ...catalogModelViolations,
    ...deepseekShapingViolations,
    ...deepseekStreamFallbackViolations,
    ...quotaWindowViolations,
    ...rehydrateViolations, ...budgetTierViolations, ...staticItemViolations, ...replacedClassifierViolations, ...cliDependencyViolations,
    ...doctorDependencyViolations, ...proxyDependencyViolations, ...runtimeTuningViolations,
    ...defaultFeatureViolations];
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
  assert.match(findViolations([[DEEPSEEK_STRICT_TOOLS_FILE, "fn strict_schema() {}"]]).join("\n"),
    /strict schema normalization must use Mojo/u);
  assert.match(findViolations([[DEEPSEEK_STRICT_SCHEMA_FILE,
    "fn deepseek_provider_core_sanitize_strict_schema() {}"]]).join("\n"),
  /replaced Rust strict schema normalization/u);
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
    "mojo::quota_snapshot_plan(); mojo::quota_gate_plan();"]]), []);
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
    "fn quota_error_summary_basic(lower: &str) {}"]])[0], /Rust quota error classifier/u);
  assert.match(findViolations([[QUOTA_WINDOWS_FILE,
    'fn format_blocked_quota_status() {\n    #[cfg(not(feature = "mojo"))] rust();\n}']]).join("\n"),
    /feature-off Rust classifier/u);
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
  assert.match(findViolations([["crates/prodex-provider-core/src/deepseek_bridge/request_params_tests.rs",
    "fn oracle() {}"]]).join("\n"), /Rust fallback or oracle/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/deepseek/request.rs",
    "fn deepseek_request_body_from_responses() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([[MODEL_SPEC_FILE,
    "fn matches_id_or_alias() { self.id.eq_ignore_ascii_case(model) }"]]).join("\n"),
    /model matcher must use the Mojo catalog kernel/u);
  assert.match(findViolations([[
    "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs",
    "fn external_catalog_model_indices_rust() {}",
  ]])[0], /Rust semantic oracle or copy/u);
  assert.match(findViolations([[
    "crates/prodex-app/src/runtime_external_provider_config/catalog_model.rs",
    "pub(super) fn external_catalog_model_indices(ids: &[&str]) -> Vec<usize> { ids.iter().map(|id| id.to_ascii_lowercase()).collect() }",
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
    'pub fn provider_error_rejects_request_member() {\n  prodex_mojo_core::json::provider_error_rejects_member(nodes, raw, member);\n}']]), []);
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
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/smart_context/normalization/static_context.rs",
    "fn smart_context_static_context_noise_key(key: &str) {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/smart_context/static_context.rs",
    'fn smart_context_stabilize_static_context_items_bounded() { #[cfg(not(feature = "mojo"))] old_sort(); }\nfn smart_context_reduce_static_context_items_mojo(']]).join("\n"),
    /static-item selection must use Mojo/u);
  assert.match(findViolations([["crates/prodex-runtime-policy/src/types/runtime_proxy_preset.rs",
    "fn resolve_rust() {}"]])[0], /Rust semantic oracle or copy/u);
  assert.match(findViolations([[PRECOMMIT_BUDGET_FILE,
    "fn runtime_proxy_precommit_budget_for_profile_count_rust() {}"]])[0],
    /Rust semantic oracle or copy/u);
  assert.match(findViolations([[PRECOMMIT_BUDGET_TEST_FILE,
    "fn precommit_budget_matches_rust_oracle() {}"]])[0],
    /Rust pre-commit budget oracle/u);
  assert(findViolations([[DEEPSEEK_SHAPING_FILE,
    'pub fn deepseek_provider_core_response_created_event() { #[cfg(not(feature = "mojo"))] fallback(); }']])
    .some((violation) => violation.includes("deepseek_provider_core_response_created_event contains a feature-off Rust path")));
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
  assert.match(findViolations([[CLI_RUNTIME_FEATURE_FILE, "fn rust_plan() {}"]])[0],
    /Rust runtime-feature planner or oracle/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/fallback/chains/gemini.rs",
    "fn provider_gemini_model_fallback_alias_chain() {}"]])[0], /Rust Gemini model fallback table/u);
  assert(findViolations([[GEMINI_BRIDGE_REQUEST_FILE,
    'pub fn gemini_provider_core_generate_content_body_value() {\n#[cfg(not(feature = "mojo"))]\nold_body();\n}']])
    .some((violation) => violation.includes("gemini_provider_core_generate_content_body_value must use Mojo")));
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/stream/shaping.rs",
    '#[cfg(feature = "mojo")] fn gated_shape() {}']]).join("\n"),
    /stream shaping Mojo kernel must be unconditional/u);
  assert.match(findViolations([[GEMINI_GENERATION_CONFIG_FILE,
    "fn gemini_generation_config_from_request() {}"]])[0], /duplicate Gemini generation-config adapter/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/gemini/request/optional_fields.rs",
    "fn gemini_apply_optional_request_fields() {}"]])[0], /Rust fallback or oracle/u);
  assert.match(findViolations([[ANTHROPIC_MESSAGES_FILE,
    '#[cfg(not(feature = "mojo"))] fn existing_path() { Some("text") => () }',
  ]]).join("\n"), /feature-off Rust path/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/anthropic/messages/stream.rs",
    '#[cfg(not(feature = "mojo"))] fn existing_path() { Some("text") => () }',
  ]])[0], /Mojo-owned operation cannot have a feature-off Rust path/u);
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
