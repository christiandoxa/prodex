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
  "crates/prodex-runtime-proxy/src/mojo.rs",
  "crates/prodex-runtime-proxy/src/quota.rs",
  "crates/prodex-runtime-proxy/src/selection_plan.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting.rs",
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
  "crates/prodex-provider-core/src/fallback/chains.rs",
  "crates/prodex-provider-core/src/fallback/chains/gemini.rs",
  "crates/prodex-provider-core/src/catalog.rs",
  "crates/prodex-provider-core/src/models.rs",
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
  "crates/prodex-runtime-proxy/src/health/backoff.rs",
  "crates/prodex-runtime-proxy/src/health/score.rs",
  "crates/prodex-runtime-proxy/src/health/latency.rs",
  "crates/prodex-runtime-proxy/src/health/inflight.rs",
  "crates/prodex-runtime-proxy/src/health/health_decisions.rs",
  "crates/prodex-cli/src/runtime_args/super_tail_extract.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/schema.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools.rs",
  "crates/prodex-provider-core/src/gemini_bridge/request/tools.rs",
  "crates/prodex-runtime-proxy/src/response_forwarding.rs",
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
  "crates/prodex-provider-core/src/translators/deepseek/stream.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping_tests.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/request.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/semantics_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/stream.rs",
];

const UNCONDITIONAL_MOJO_FILES = new Set([
  "crates/prodex-runtime-quota/src/selection/scoring.rs",
  "crates/prodex-runtime-quota/src/selection/scoring/profile_order.rs",
  "crates/prodex-runtime-doctor/src/parsing/log_line.rs",
  "crates/prodex-runtime-doctor/src/parsing/request_timeline.rs",
  "crates/prodex-runtime-doctor/src/parsing/route_profile.rs",
  "crates/prodex-runtime-doctor/src/parsing/selection.rs",
  "crates/prodex-runtime-doctor/src/state_summary/profiles.rs",
  "crates/prodex-cli/src/runtime_args/super_tail_extract.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/schema.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools.rs",
  "crates/prodex-runtime-proxy/src/response_forwarding.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/estimation.rs",
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
  "crates/prodex-provider-core/src/translators/deepseek/stream/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/request.rs",
  "crates/prodex-provider-core/src/translators/kiro/stream.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_params.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request_mojo.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request_mojo_tests.rs",
]);
const FEATURE_OFF_RUST_PATH = /\bnot\s*\(\s*feature\s*=\s*"(?:mojo|mojo-core|runtime-log-mojo|state-summary-mojo)"\s*\)/u;
const ANTHROPIC_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages/response.rs";
const ANTHROPIC_MESSAGES_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages.rs";
const ANTHROPIC_WEB_SEARCH_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages/web_search.rs";
const ANTHROPIC_REQUEST_FALLBACK_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages/request_fallback.rs";
const ANTHROPIC_REQUEST_ORACLE_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages/mojo_request_tests.rs";
const REMOVED_ORACLE_FILES = [
  ANTHROPIC_REQUEST_FALLBACK_FILE,
  ANTHROPIC_REQUEST_ORACLE_FILE,
  "crates/prodex-context/src/critical_signal/rust_oracle.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/oracle.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/pressure.rs",
  "crates/prodex-runtime-launch/src/args_oracle.rs",
  "crates/prodex-runtime-launch/src/args_resume.rs",
  "crates/prodex-cli/src/runtime_args/super_tail_extract/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/schema/composition.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/controls.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/validation.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request/validation.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_request/validation/input_content.rs",
  "crates/prodex-provider-core/src/translators/openai_chat_compat_util.rs",
];
const HARD_REPLACED_RUST_FILES = new Set([
  "crates/prodex-context/src/critical_signal.rs",
  "crates/prodex-quota/src/render/windows.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/estimation.rs",
  "crates/prodex-runtime-proxy/src/health/score.rs",
  "crates/prodex-runtime-proxy/src/health/latency.rs",
  "crates/prodex-runtime-proxy/src/health/inflight.rs",
  "crates/prodex-runtime-proxy/src/health/health_decisions.rs",
  "crates/prodex-quota/src/render/model_capacity.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rewrite_policy/adaptive.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/calibration.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/observed.rs",
  "crates/prodex-runtime-proxy/src/smart_context/safety.rs",
  "crates/prodex-runtime-proxy/src/smart_context/rollout.rs",
  "crates/prodex-runtime-proxy/src/smart_context/regression.rs",
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
  "crates/prodex-cli/src/runtime_args/super_tail_extract.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/schema.rs",
  "crates/prodex-provider-core/src/translators/gemini/request/tools.rs",
  "crates/prodex-runtime-proxy/src/response_forwarding.rs",
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
  "crates/prodex-provider-core/src/translators/deepseek/stream.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/shaping_tests.rs",
  "crates/prodex-provider-core/src/translators/deepseek/stream/mojo_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/request.rs",
  "crates/prodex-provider-core/src/translators/kiro/request/semantics_tests.rs",
  "crates/prodex-provider-core/src/translators/kiro/stream.rs",
]);
const REQUIRED_DEFAULT_FEATURES = new Map([
  ["crates/prodex-app/Cargo.toml", "mojo-core"],
  ["crates/prodex-quota/Cargo.toml", "mojo"],
  ["crates/prodex-runtime-doctor/Cargo.toml", "state-summary-mojo"],
  ["crates/prodex-runtime-launch/Cargo.toml", "mojo"],
]);
const CLI_RUNTIME_FEATURE_FILE = "crates/prodex-cli/src/runtime_features.rs";
const QUOTA_WINDOWS_FILE = "crates/prodex-quota/src/render/windows.rs";
const REHYDRATE_FILE = "crates/prodex-runtime-proxy/src/smart_context/token_accounting.rs";
const SUPER_OVERRIDE_FILE = "crates/prodex-cli/src/runtime_args/super_tail_extract.rs";
const GEMINI_SCHEMA_FILE = "crates/prodex-provider-core/src/translators/gemini/request/schema.rs";
const GEMINI_TOOLS_FILE = "crates/prodex-provider-core/src/translators/gemini/request/tools.rs";
const GEMINI_STATUS_FILE = "crates/prodex-provider-core/src/translators/gemini/response/status.rs";
const RESPONSE_FORWARDING_FILE = "crates/prodex-runtime-proxy/src/response_forwarding.rs";
const QUOTA_POOL_FILE = "crates/prodex-quota/src/render/pool.rs";
const QUOTA_MODEL_CAPACITY_FILE = "crates/prodex-quota/src/render/model_capacity.rs";
const HEALTH_ABI_TEST_FILE = "crates/prodex-mojo-core/tests/profile_health.rs";
const DEEPSEEK_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/deepseek/response.rs";
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
  const replacedClassifierViolations = files.flatMap(([filePath, contents]) => {
    const forbidden = new Map([
      [SUPER_OVERRIDE_FILE, /\bfn\s+(?:scan_override_rust|scan_identity_override|scan_boolean_override|scan_runtime_override|scan_feature_value_override|scan_feature_boolean_override)\s*\(/u],
      [GEMINI_SCHEMA_FILE, /\bfn\s+(?:schema_type|supported_schema_type|sanitized_enum|sanitized_properties|sanitized_required)\s*\(/u],
      [GEMINI_TOOLS_FILE, /\bfn\s+gemini_tool_config_from_request_oracle\s*\(/u],
      [GEMINI_STATUS_FILE, /\bfn\s+gemini_(?:finish_reason_(?:failure|incomplete)|prompt_feedback_failure)_oracle\s*\(/u],
      [RESPONSE_FORWARDING_FILE, /\bfn\s+(?:should_skip_response_header|response_content_type_is_sse|token_usage_event_is_loggable|response_event_is_generation_start)\s*\(/u],
      [QUOTA_POOL_FILE, /\bfn\s+(?:aggregate_openai_quota|aggregate_main_quota|add_pool_window|add_ready_pool_window)\s*\(/u],
      [QUOTA_MODEL_CAPACITY_FILE, /\bfn\s+(?:normalized_identifier|is_luna_reserve_identifier|openai_usage_advertises_luna_reserve)\s*\(/u],
      [HEALTH_ABI_TEST_FILE, /\bfn\s+(?:effective|expected)\s*\(/u],
      [DEEPSEEK_RESPONSE_FILE, /\bfn\s+deepseek_stream_event_from_chat_value_rust\s*\(|#\[cfg\(not\(feature\s*=\s*"mojo"\)\)\]\s*pub\(super\)\s+fn\s+deepseek_stream_event_from_chat_value\s*\(/u],
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
  const defaultFeatureViolations = files.flatMap(([filePath, contents]) => {
    const required = REQUIRED_DEFAULT_FEATURES.get(filePath);
    if (!required) return [];
    const defaults = contents.match(/^default\s*=\s*\[([^\]]*)\]/mu)?.[1];
    return defaults?.match(/"[^"]+"/gu)?.includes(`"${required}"`)
      ? [] : [`${filePath}: default features must include ${required}`];
  });
  return [...markerViolations, ...featureOffViolations, ...anthropicResponseViolations,
    ...anthropicEnvelopeViolations, ...anthropicRequestViolations, ...cliRuntimeFeatureViolations,
    ...geminiFallbackViolations, ...hardReplacementViolations, ...deepseekShapingViolations,
    ...quotaWindowViolations,
    ...rehydrateViolations, ...replacedClassifierViolations, ...cliDependencyViolations,
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
    '[features]\ndefault = []\nstate-summary-mojo = []']])[0], /default features must include state-summary-mojo/u);
  assert.match(findViolations([["crates/prodex-runtime-doctor/src/state_summary/profiles.rs",
    '#[cfg(not(feature = "state-summary-mojo"))] fn rust_summary() {}']])[0],
    /feature-off Rust path/u);
  assert.match(findViolations([[QUOTA_WINDOWS_FILE,
    "fn quota_error_summary_basic(lower: &str) {}"]])[0], /Rust quota error classifier/u);
  assert.match(findViolations([[QUOTA_WINDOWS_FILE,
    'fn format_blocked_quota_status() {\n    #[cfg(not(feature = "mojo"))] rust();\n}']])[0],
    /feature-off Rust classifier/u);
  assert.match(findViolations([[REHYDRATE_FILE,
    'pub fn smart_context_auto_rehydrate_plan() {\n    #[cfg(not(feature = "mojo"))] rust();\n}\nfn smart_context_auto_rehydrate_plan_mojo() {}']])[0],
    /feature-off Rust planner/u);
  assert.match(findViolations([[SUPER_OVERRIDE_FILE, "fn scan_override_rust() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([[GEMINI_SCHEMA_FILE, "fn sanitized_required() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
  assert.match(findViolations([[GEMINI_TOOLS_FILE, "fn gemini_tool_config_from_request_oracle() {}"]]).join("\n"),
    /replaced Rust semantic implementation/u);
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
  assert.match(findViolations([[QUOTA_MODEL_CAPACITY_FILE,
    "fn normalized_identifier() {}"]]).join("\n"), /replaced Rust semantic implementation/u);
  assert.match(findViolations([[HEALTH_ABI_TEST_FILE,
    "fn effective() {}"]]).join("\n"), /replaced Rust semantic implementation/u);
  assert.match(findViolations([[DEEPSEEK_RESPONSE_FILE,
    '#[cfg(not(feature = "mojo"))] pub(super) fn deepseek_stream_event_from_chat_value() {}']]).join("\n"),
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
