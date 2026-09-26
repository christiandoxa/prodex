#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const PROMOTED_FILES = [
  "crates/prodex-mojo-core/build.rs",
  "crates/prodex-mojo-core/src/lib.rs",
  "crates/prodex-mojo-core/src/quota.rs",
  "crates/prodex-mojo-core/src/routing.rs",
  "crates/prodex-mojo-core/src/runtime.rs",
  "crates/prodex-mojo-core/src/runtime/auto_redeem.rs",
  "crates/prodex-mojo-core/src/runtime_decisions.rs",
  "crates/prodex-mojo-core/tests/profile_health.rs",
  "crates/prodex-mojo-core/src/provider_constraints.rs",
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
  "crates/prodex-quota/src/render.rs",
  "crates/prodex-quota/src/render/remaining_percent.rs",
  "crates/prodex-quota/src/render/quota_policy.rs",
  "crates/prodex-runtime-proxy/src/mojo.rs",
  "crates/prodex-runtime-proxy/src/quota.rs",
  "crates/prodex-runtime-proxy/src/quota/mojo.rs",
  "crates/prodex-runtime-proxy/src/selection_plan.rs",
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
  "crates/prodex-runtime-proxy/src/selection_policy/mojo.rs",
  "crates/prodex-runtime-proxy/tests/src/selection_policy.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/token_budget.rs",
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
  "crates/prodex-provider-core/src/fallback/chains.rs",
  "crates/prodex-provider-core/src/fallback/chains/gemini.rs",
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
  "crates/prodex-provider-core/src/translators/gemini/response/metadata.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls/chat.rs",
  "crates/prodex-provider-core/src/translators/deepseek/response.rs",
  "crates/prodex-provider-core/src/translators/deepseek.rs",
  "crates/prodex-provider-core/src/translators/deepseek/request_transform.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo_tests.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping_tests.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/request.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/semantics_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/stream.rs",
];

const UNCONDITIONAL_MOJO_FILES = new Set([
  "crates/prodex-quota/src/render/gemini.rs",
  "crates/prodex-quota/src/capacity.rs",
  "crates/prodex-quota/src/render/windows.rs",
  "crates/prodex-observability/src/lib.rs",
  "crates/prodex-observability/src/metric_label.rs",
  "crates/prodex-runtime-store/src/profile_backoff/score.rs",
  "crates/prodex-quota/src/render.rs",
  "crates/prodex-quota/src/render/remaining_percent.rs",
  "crates/prodex-quota/src/render/quota_policy.rs",
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
  "crates/prodex-provider-core/src/translators/gemini/request/schema.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools/builtin.rs",
  "crates/prodex-runtime-proxy/src/response_forwarding.rs",
  "crates/prodex-runtime-proxy/src/log_event.rs",
  "crates/prodex-runtime-proxy/src/payload_detection/sse.rs",
  "crates/prodex-runtime-proxy/src/payload_detection/error_messages.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/estimation.rs",
  "crates/prodex-runtime-proxy/src/smart_context/normalization/token_budget.rs",
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
  "crates/prodex-provider-core/src/translators/gemini/response/metadata.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls/chat.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream.rs",
  "crates/prodex-provider-core/src/translators/deepseek.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/request.rs",
  "crates/prodex-provider-core/src/translators/kiro/stream.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_params.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request_mojo.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request_mojo_tests.rs",
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
  "crates/prodex-runtime-launch/src/args_oracle.rs",
  "crates/prodex-runtime-launch/src/args_resume.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/next_steps/compatibility.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/final_summary/default_diagnosis.rs",
  "crates/prodex-runtime-doctor/src/diagnosis/final_summary/pressure.rs",
  "crates/prodex-runtime-doctor/src/suggestions/compatibility.rs",
  "crates/prodex-cli/src/runtime_args/super_tail_extract/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/schema/composition.rs",
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
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/estimation.rs",
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
  "crates/prodex-provider-core/src/translators/gemini/response/metadata.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls.rs",
  "crates/prodex-provider-core/src/translators/gemini/response_tool_calls/chat.rs",
  "crates/prodex-provider-core/src/translators/deepseek/response.rs",
  "crates/prodex-provider-core/src/translators/deepseek.rs",
  "crates/prodex-provider-core/src/translators/deepseek/request_transform.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/request_params.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/messages/mojo_tests.rs",
  "crates/prodex-provider-core/src/deepseek_bridge/input_items.rs",
  "crates/prodex-provider-core/src/translators/deepseek/tooling.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping_tests.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/request.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/semantics_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/stream.rs",
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
const CLI_RUNTIME_FEATURE_FILE = "crates/prodex-cli/src/runtime_features.rs";
const DOCTOR_CARGO_FILE = "crates/prodex-runtime-doctor/Cargo.toml";
const RUNTIME_PROXY_CARGO_FILE = "crates/prodex-runtime-proxy/Cargo.toml";
const QUOTA_WINDOWS_FILE = "crates/prodex-quota/src/render/windows.rs";
const REHYDRATE_FILE = "crates/prodex-runtime-proxy/src/smart_context/token_accounting.rs";
const SUPER_OVERRIDE_FILE = "crates/prodex-cli/src/runtime_args/super_tail_extract.rs";
const GEMINI_SCHEMA_FILE = "crates/prodex-provider-core/src/translators/gemini/request/schema.rs";
const GEMINI_TOOLS_FILE = "crates/prodex-provider-core/src/translators/gemini/request/tools.rs";
const GEMINI_STATUS_FILE = "crates/prodex-provider-core/src/translators/gemini/response/status.rs";
const RESPONSE_FORWARDING_FILE = "crates/prodex-runtime-proxy/src/response_forwarding.rs";
const QUOTA_POOL_FILE = "crates/prodex-quota/src/render/pool.rs";
const QUOTA_MODEL_CAPACITY_FILE = "crates/prodex-quota/src/render/model_capacity.rs";
const RUNTIME_QUOTA_FILE = "crates/prodex-runtime-proxy/src/quota.rs";
const HEALTH_ABI_TEST_FILE = "crates/prodex-mojo-core/tests/profile_health.rs";
const DEEPSEEK_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/deepseek/response.rs";
const DEEPSEEK_REQUEST_FILE = "crates/prodex-provider-core/src/translators/deepseek/request_transform.rs";
const MODEL_SPEC_FILE = "crates/prodex-provider-core/src/surface/models.rs";
const PROMPT_CACHE_SELECTION_FILE = "crates/prodex-runtime-proxy/src/selection_plan.rs";
const DEEPSEEK_SHAPING_FILE = "crates/prodex-provider-core/src/translators/deepseek/stream/shaping.rs";
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
  const cliRuntimeFeatureViolations = files
    .filter(([filePath, contents]) => filePath === CLI_RUNTIME_FEATURE_FILE &&
      /\bfn\s+(?:rust_plan|rollout_budget_reminders|to_codex_config_args_rust|mojo_feature_plan_matches_rust_oracle_for_seeded_inputs)\s*\(/u.test(contents))
    .map(([filePath]) => `${filePath}: contains a Rust runtime-feature planner or oracle`);
  const geminiFallbackViolations = files
    .filter(([filePath, contents]) =>
      filePath === "crates/prodex-provider-core/src/fallback/chains/gemini.rs" &&
      /\bfn\s+provider_gemini_model_fallback_alias_chain\s*\(/u.test(contents))
    .map(([filePath]) => `${filePath}: contains a Rust Gemini model fallback table`);
  const hardReplacementViolations = files
    .filter(([filePath, contents]) => HARD_REPLACED_RUST_FILES.has(filePath) &&
      /\b(?:rust_oracle|fn\s+[A-Za-z0-9_]+_rust\s*\()/u.test(contents))
    .map(([filePath]) => `${filePath}: contains a Rust semantic oracle or copy`);
  const deepseekRequestViolations = files
    .filter(([filePath, contents]) => filePath === DEEPSEEK_REQUEST_FILE &&
      !contents.includes("DeepSeekKernelOperation::RawCommonRequest"))
    .map(([filePath]) => `${filePath}: DeepSeek request body must use the Mojo raw kernel`);
  const modelSpecViolations = files
    .filter(([filePath, contents]) => filePath === MODEL_SPEC_FILE &&
      /\bfn\s+matches_id_or_alias\s*\(/u.test(contents) &&
      !contents.includes("resolve_catalog_model("))
    .map(([filePath]) => `${filePath}: model matcher must use the Mojo catalog kernel`);
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
  const quotaWindowViolations = files.flatMap(([filePath, contents]) => {
    if (filePath !== QUOTA_WINDOWS_FILE) return [];
    const violations = [];
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
  const replacedClassifierViolations = files.flatMap(([filePath, contents]) => {
    const forbidden = new Map([
      [SUPER_OVERRIDE_FILE, /\bfn\s+(?:scan_override_rust|scan_identity_override|scan_boolean_override|scan_runtime_override|scan_feature_value_override|scan_feature_boolean_override)\s*\(/u],
      [GEMINI_SCHEMA_FILE, /\bfn\s+(?:schema_type|supported_schema_type|sanitized_enum|sanitized_properties|sanitized_required)\s*\(/u],
      [GEMINI_TOOLS_FILE, /\bfn\s+gemini_tool_config_from_request_oracle\s*\(/u],
      ["crates/prodex-provider-core/src/gemini_bridge/request/native_project.rs", /\bfn\s+gemini_provider_core_stamp_native_(?:project|metadata_project)\s*\(/u],
      ["crates/prodex-provider-core/src/gemini_bridge/request/simple.rs", /\bfn\s+gemini_simple_(?:input_item|content_item|tool_calls|tool_call)\s*\(/u],
      ["crates/prodex-provider-core/src/translators/gemini/request.rs", /\bfn\s+gemini_request_object_mut\s*\(/u],
      ["crates/prodex-provider-core/src/translators/gemini/request/tools/builtin.rs", /\bfn\s+gemini_(?:computer_use_tool|is_computer_use_tool|is_code_execution_tool|is_web_search_tool|is_url_context_tool|builtin_tool_value)\s*\(/u],
      [GEMINI_STATUS_FILE, /\bfn\s+gemini_(?:finish_reason_(?:failure|incomplete)|prompt_feedback_failure)_oracle\s*\(/u],
      [RESPONSE_FORWARDING_FILE, /\bfn\s+(?:should_skip_response_header|response_content_type_is_sse|token_usage_event_is_loggable|response_event_is_generation_start)\s*\(/u],
      [QUOTA_POOL_FILE, /\bfn\s+(?:aggregate_openai_quota|aggregate_main_quota|add_pool_window|add_ready_pool_window)\s*\(/u],
      [QUOTA_MODEL_CAPACITY_FILE, /\bfn\s+(?:normalized_identifier|is_luna_reserve_identifier|openai_usage_advertises_luna_reserve)\s*\(/u],
      [RUNTIME_QUOTA_FILE, /\bfn\s+runtime_proxy_quota_score_for_route_rust\s*\(/u],
      [HEALTH_ABI_TEST_FILE, /\bfn\s+(?:effective|expected)\s*\(/u],
      [DEEPSEEK_RESPONSE_FILE, /\bfn\s+deepseek_stream_event_from_chat_value_rust\s*\(|#\[cfg\(not\(feature\s*=\s*"mojo"\)\)\]\s*pub\(super\)\s+fn\s+deepseek_stream_event_from_chat_value\s*\(/u],
      [DEEPSEEK_REQUEST_FILE, /\bfn\s+(?:deepseek_request_body_from_responses_rust|deepseek_messages_from_request|deepseek_tool_choice_from_request)\s*\(/u],
      [MODEL_SPEC_FILE, /\beq_ignore_ascii_case\s*\(/u],
      [PROMPT_CACHE_SELECTION_FILE, /selection_prompt_cache_rust|\bfn\s+runtime_prompt_cache_affinity_score\s*\(/u],
      [GEMINI_TOOL_CALLS_FILE, /\bfn\s+gemini_split_flat_namespace_tool_name\s*\(/u],
      [GEMINI_CHAT_TOOL_CALLS_FILE, /\blet\s+mut\s+item\s*=\s*json!\s*\(/u],
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
  const defaultFeatureViolations = files.flatMap(([filePath, contents]) => {
    const required = REQUIRED_DEFAULT_FEATURES.get(filePath);
    if (!required) return [];
    const defaults = contents.match(/^default\s*=\s*\[([^\]]*)\]/mu)?.[1];
    return defaults?.match(/"[^"]+"/gu)?.includes(`"${required}"`)
      ? [] : [`${filePath}: default features must include ${required}`];
  });
  return [...markerViolations, ...featureOffViolations, ...anthropicResponseViolations,
    ...anthropicEnvelopeViolations, ...anthropicRequestViolations, ...cliRuntimeFeatureViolations,
    ...geminiFallbackViolations, ...hardReplacementViolations, ...deepseekRequestViolations,
    ...modelSpecViolations,
    ...deepseekShapingViolations,
    ...quotaWindowViolations,
    ...rehydrateViolations, ...budgetTierViolations, ...replacedClassifierViolations, ...cliDependencyViolations,
    ...doctorDependencyViolations, ...proxyDependencyViolations,
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
  assert.match(findViolations([[DEEPSEEK_REQUEST_FILE,
    "fn deepseek_request_body_from_responses_rust() {}"]]).join("\n"),
    /Rust semantic oracle or copy/u);
  assert.match(findViolations([[DEEPSEEK_REQUEST_FILE,
    "fn deepseek_request_body_from_responses() {}"]]).join("\n"),
    /must use the Mojo raw kernel/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/deepseek_bridge/request_params.rs",
    '#[cfg(not(feature = "mojo"))] fn validate_primitive_request_fields_rust() {}']]).join("\n"),
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
  assert.match(findViolations([["crates/prodex-runtime-tuning/src/capacity.rs",
    "fn runtime_proxy_worker_count_default_rust() {}"]])[0], /Rust semantic oracle or copy/u);
  assert.match(findViolations([["crates/prodex-runtime-proxy/src/smart_context/rollout.rs",
    "fn smart_context_rollout_decision_rust() {}"]])[0], /Rust semantic oracle or copy/u);
  assert(findViolations([[DEEPSEEK_SHAPING_FILE,
    'pub fn deepseek_provider_core_response_created_event() { #[cfg(not(feature = "mojo"))] fallback(); }']])
    .some((violation) => violation.includes("deepseek_provider_core_response_created_event contains a feature-off Rust path")));
  assert.match(findViolations([["crates/prodex-provider-core/src/translators/openai_chat_compat_response/stream.rs",
    "fn translate_chat_stream_value_to_responses_rust() {}"]])[0], /Rust semantic oracle or copy/u);
  assert.match(findViolations([[ANTHROPIC_MESSAGES_FILE,
    "fn anthropic_tool_choice() {}"]])[0], /Rust Anthropic request semantics/u);
  assert.match(findViolations([[ANTHROPIC_WEB_SEARCH_FILE,
    "fn anthropic_web_search_tool() {}"]])[0], /Rust Anthropic request web-search semantics/u);
  assert.match(findViolations([[CLI_RUNTIME_FEATURE_FILE, "fn rust_plan() {}"]])[0],
    /Rust runtime-feature planner or oracle/u);
  assert.match(findViolations([["crates/prodex-provider-core/src/fallback/chains/gemini.rs",
    "fn provider_gemini_model_fallback_alias_chain() {}"]])[0], /Rust Gemini model fallback table/u);
  for (const filePath of [
    "crates/prodex-provider-core/src/translators/anthropic/messages.rs",
    "crates/prodex-provider-core/src/translators/anthropic/messages/stream.rs",
  ]) {
    assert.deepEqual(findViolations([[
      filePath,
      '#[cfg(not(feature = "mojo"))] fn existing_path() { Some("text") => () }',
    ]]), []);
  }
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
