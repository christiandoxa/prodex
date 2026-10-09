#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const DEFAULT_BASELINE_PATH = path.join(repoRoot, "scripts/compat/upstream-baseline.json");

const REQUIRED_CRITICAL_FILES = [
  "codex-rs/core/src/client.rs",
  "codex-rs/core/src/compact_remote_v2.rs",
  "codex-rs/core/src/compact_remote_v2_attempt.rs",
  "codex-rs/core/src/turn_metadata.rs",
  "codex-rs/core/src/responses_metadata.rs",
  "codex-rs/models-manager/models.json",
  "codex-rs/models-manager/src/cache.rs",
  "codex-rs/model-provider/src/auth.rs",
  "codex-rs/model-provider/src/models_identity.rs",
  "codex-rs/codex-mcp/src/tools.rs",
  "codex-rs/codex-mcp/src/connection_manager_tests.rs",
  "codex-rs/model-provider-info/src/lib.rs",
  "codex-rs/model-provider-info/src/capabilities.rs",
  "codex-rs/model-provider/src/amazon_bedrock/catalog.rs",
  "codex-rs/model-provider/src/capabilities.rs",
  "codex-rs/model-provider/src/provider.rs",
  "codex-rs/core/src/realtime_conversation.rs",
  "codex-rs/codex-api/src/endpoint/realtime_call.rs",
  "codex-rs/codex-api/src/safety_buffering.rs",
  "codex-rs/codex-api/src/sse/responses.rs",
  "codex-rs/codex-api/src/sse/responses_error.rs",
  "codex-rs/codex-api/src/endpoint/responses.rs",
  "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
  "codex-rs/core/src/context/base_instructions.rs",
  "codex-rs/core/src/context/world_state/base_instructions.rs",
  "codex-rs/core/src/context/world_state/top_level_tools.rs",
  "codex-rs/core/src/tools/spec_plan.rs",
  "codex-rs/protocol/src/turn_input.rs",
  "codex-rs/protocol/src/environment.rs",
  "codex-rs/rollout-trace/src/reducer/conversation/normalize.rs",
  "codex-rs/core/src/config/mod.rs",
  "codex-rs/config/src/project_trust.rs",
  "codex-rs/features/src/lib.rs",
  "codex-rs/http-client/src/outbound_proxy.rs",
  "codex-rs/http-client/src/outbound_proxy/macos.rs",
  "codex-rs/http-client/src/outbound_proxy/windows.rs",
  "codex-rs/core-plugins/src/manifest.rs",
  "codex-rs/plugin/src/manifest.rs",
  "codex-rs/ext/web-search/src/extension.rs",
  "codex-rs/ext/web-search/src/tool.rs",
  "codex-rs/codex-api/src/endpoint/search.rs",
  "codex-rs/tools/src/json_schema.rs",
  "codex-rs/tools/src/json_schema/compaction.rs",
  "codex-rs/exec/src/cli.rs",
  "codex-rs/exec/src/lib.rs",
  "codex-rs/protocol/src/protocol.rs",
  "codex-rs/app-server/README.md",
  "codex-rs/app-server-protocol/src/export.rs",
  "codex-rs/app-server-protocol/src/rpc.rs",
  "codex-rs/app-server-protocol/src/protocol/common.rs",
  "codex-rs/app-server-protocol/src/protocol/v2/mcp.rs",
  "codex-rs/app-server-protocol/src/protocol/v2/thread.rs",
  "codex-rs/app-server-protocol/src/protocol/v2/turn.rs",
  "codex-rs/app-server-protocol/src/protocol/v2/environment.rs",
  "codex-rs/app-server/src/message_processor.rs",
  "codex-rs/app-server/src/request_processors/initialize_processor.rs",
  "codex-rs/app-server/src/request_processors/thread_processor.rs",
  "codex-rs/app-server/src/request_processors/turn_processor.rs",
  "codex-rs/app-server/src/request_processors/environment_processor.rs",
  "codex-rs/app-server/src/request_processors/mcp_event_stream.rs",
  "codex-rs/app-server/src/request_processors/account_processor/workspace_routing.rs",
  "codex-rs/app-server/src/request_serialization.rs",
  "codex-rs/utils/process/src/lib.rs",
  "codex-rs/core/src/spawn.rs",
  "codex-rs/tui/src/security_setup.rs",
  "codex-rs/models-manager/src/manager.rs",
  "codex-rs/tui/src/app_server_session/provider_selection.rs",
  "codex-rs/tui/src/projectless.rs",
  "codex-rs/tui/src/resume_permissions.rs",
  "codex-rs/core/src/session/environment.rs",
  "codex-rs/core/src/agent/control/spawn.rs",
  "codex-rs/rmcp-client/src/stdio_server_launcher.rs",
  "codex-rs/core/src/responses_retry.rs",
  "codex-rs/codex-api/src/common.rs",
  "codex-rs/core/src/cyber_access_program.rs",
];

const REQUIRED_FILE_CONTAINS = {
  "codex-rs/core/src/client.rs": [
    "build_responses_headers",
    "build_responses_compatibility_headers",
    "build_ws_client_metadata",
    "build_session_headers",
    "CodexResponsesMetadata",
    "response_create_client_metadata",
    "previous_response_id",
    "X_CODEX_INSTALLATION_ID_HEADER",
    "x-codex-installation-id",
    "X_CODEX_TURN_STATE_HEADER",
    "x-codex-turn-state",
    "X_CODEX_TURN_METADATA_HEADER",
    "x-codex-turn-metadata",
    "X_CODEX_PARENT_THREAD_ID_HEADER",
    "x-codex-parent-thread-id",
    "X_CODEX_WINDOW_ID_HEADER",
    "x-codex-window-id",
    "X_OPENAI_MEMGEN_REQUEST_HEADER",
    "x-openai-memgen-request",
    "X_OPENAI_SUBAGENT_HEADER",
    "x-openai-subagent",
    "X_RESPONSESAPI_INCLUDE_TIMING_METRICS_HEADER",
    "x-responsesapi-include-timing-metrics",
    "X_OPENAI_INTERNAL_CODEX_RESPONSES_LITE_HEADER",
    "x-openai-internal-codex-responses-lite",
    "WS_REQUEST_HEADER_RESPONSES_LITE_CLIENT_METADATA_KEY",
    "ws_request_header_x_openai_internal_codex_responses_lite",
    "X_CODEX_WS_STREAM_REQUEST_START_MS_CLIENT_METADATA_KEY",
    "x-codex-ws-stream-request-start-ms",
    "x-codex-beta-features",
    "OPENAI_BETA_HEADER",
    "responses_websockets=2026-02-06",
    "x-client-request-id",
    "stream_responses_websocket",
    "auth_owner_generation",
    "take_cached_websocket_session",
    "build_responses_request",
    "client_metadata",
    "prepare_response_items_for_request",
    "responses_request_properties_match",
    "responses_session_id",
    "session_source.is_non_root_agent()",
    "prompt_cache_key(metadata)",
    "clear_tool_result_metadata",
    "include_internal",
    ".include_internal_metadata(&client_setup.api_provider)",
    "if !include_internal",
    "responses_metadata.client_metadata(include_internal)",
    "let mut prefix = Vec::new();",
    "create_tools_json_for_responses_lite(&prompt.tools)?",
    "create_tools_raw_json_for_responses_api(&prompt.tools)?.into()",
    "BaseInstructionsFragment(",
    "input.splice(0..0, prefix);",
  ],
  "codex-rs/core/src/compact_remote_v2.rs": [
    "run_remote_compact_task",
    "run_inline_remote_auto_compact_task",
    "run_remote_compaction_request_v2",
    "RequestEffortUsage::Compaction",
    "CompactionImplementation::ResponsesCompactionV2",
    "Feature::CompactionImageBudget",
    "RetainedImageBudget::Enabled",
    "truncate_retained_messages",
    "images::truncate_message_to_token_budget",
    "remaining = 0",
  ],
  "codex-rs/core/src/compact_remote_v2_attempt.rs": [
    "ResponseItem::CompactionTrigger {}",
    "CompactionTurnMetadata",
    "compaction_responses_metadata",
    "responses_metadata",
    "run_remote_compaction_request_v2",
  ],
  "codex-rs/core/src/turn_metadata.rs": [
    "detached_memory_responses_metadata",
    "request_kind",
    "window_id",
    "Turn",
    "Memory",
    "CodexResponsesRequestKind::Memory",
    "ThreadSource",
    "thread_source",
    "ThreadSource::MemoryConsolidation",
    "CodexResponsesMetadata",
    "CodexResponsesRequestKind",
    "to_responses_metadata",
    "responses_metadata_template",
    "set_responsesapi_client_metadata",
  ],
  "codex-rs/core/src/responses_metadata.rs": [
    "CodexResponsesMetadata",
    "CodexResponsesRequestKind",
    "COMPACTION_KEY",
    "compaction",
    "WINDOW_ID_KEY",
    "window_id",
    "CompactionTurnMetadata",
    "CompactionTrigger",
    "CompactionReason",
    "CompactionImplementation",
    "CompactionPhase",
    "CompactionStrategy",
    "Turn",
    "Prewarm",
    "Compaction",
    "Memory",
    "ThreadSource",
    "THREAD_SOURCE_KEY",
    "thread_source",
    "LEGACY_CODE_MODE_TOOL_NAMES_KEY",
    "code_mode_tool_names",
    "tool_namespaces_info: None",
    "compatibility_headers",
    "client_metadata",
    "turn_metadata_payload",
    "X_CODEX_TURN_METADATA_HEADER",
    "to_ascii_json_string",
  ],
  "codex-rs/models-manager/src/cache.rs": [
    "ModelsCacheEntry",
    "identity: Option<String>",
    "refresh_ttl",
    "entry.identity",
    "client_version",
    "etag",
  ],
  "codex-rs/model-provider/src/models_identity.rs": [
    "models-cache-v1",
    "provider.name",
    "provider.base_url",
    "provider.query_params",
    "requires_openai_auth",
    "get_account_id",
    "get_chatgpt_user_id",
    "get_account_email",
    "account_plan_type",
    "resolve_provider_auth",
    "has_stable_account",
  ],
  "codex-rs/models-manager/models.json": [
    "\"slug\": \"gpt-6.1-sol\"",
    "\"display_name\": \"GPT-6.1-Sol\"",
    "\"slug\": \"gpt-6-sol\"",
    "\"display_name\": \"GPT-6-Sol\"",
    "\"slug\": \"gpt-6-luna\"",
    "\"display_name\": \"GPT-6-Luna\"",
    "\"slug\": \"gpt-5.6-sol\"",
    "\"slug\": \"gpt-5.6-terra\"",
    "\"slug\": \"gpt-5.6-luna\"",
    "\"context_window\": 272000",
    "\"max_context_window\": 872000",
    "\"effort\": \"max\"",
    "\"effort\": \"ultra\"",
    "\"model_messages\":",
    "\"instructions_template\":",
  ],
  "codex-rs/model-provider/src/auth.rs": [
    "resolve_provider_auth",
    "!provider.requires_openai_auth && provider.auth.is_none()",
    "unauthenticated_auth_provider",
    "custom_provider_does_not_inherit_ambient_auth_headers",
    "custom_provider_uses_explicit_bearer_instead_of_ambient_auth",
    "openai_provider_preserves_ambient_auth_headers",
  ],
  "codex-rs/codex-mcp/src/tools.rs": [
    "normalize_tools_for_model_with_prefix",
    "MAX_TOOL_NAME_LENGTH: usize = 128",
    "append_hash_suffix",
    "fit_callable_parts_with_hash",
    "unique_callable_parts",
    "used_names",
  ],
  "codex-rs/codex-mcp/src/connection_manager_tests.rs": [
    "test_normalize_tools_respects_responses_api_name_length_boundaries",
    "test_normalize_tools_long_names_same_server",
    "test_normalize_tools_disambiguates_sanitized_namespace_collisions",
    "test_normalize_tools_disambiguates_sanitized_tool_name_collisions",
    "model_tool_name_len(&model_name), 128",
  ],
  "codex-rs/model-provider-info/src/lib.rs": [
    "ModelProviderInfo",
    "supports_standalone_web_search",
    "pub fn is_openai",
    "is_amazon_bedrock",
    "AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID",
    "openai.gpt-5.6-sol",
    "AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID",
    "openai.gpt-5.6-terra",
    "AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID",
    "openai.gpt-5.6-luna",
    "AMAZON_BEDROCK_GPT_6_1_SOL_MODEL_ID",
    "openai.gpt-6.1-sol",
    "AMAZON_BEDROCK_GPT_6_SOL_MODEL_ID",
    "openai.gpt-6-sol",
    "AMAZON_BEDROCK_GPT_6_LUNA_MODEL_ID",
    "openai.gpt-6-luna",
  ],
  "codex-rs/model-provider-info/src/capabilities.rs": [
    "ModelProviderCapabilities",
    "external_web_access: Option<bool>",
    "remote_compaction: Option<RemoteCompactionSupport>",
    "RemoteCompactionSupport",
    "Unsupported,",
    "V2,",
    "#[serde(rename_all = \"snake_case\")]",
  ],
  "codex-rs/model-provider/src/amazon_bedrock/catalog.rs": [
    "static_model_catalog",
    "normalize_bedrock_catalog",
    "fn bedrock_model(",
    "AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID",
    "AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID",
    "AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID",
    "AMAZON_BEDROCK_GPT_6_1_SOL_MODEL_ID",
    "model.additional_speed_tiers.clear()",
    "model.service_tiers.clear()",
    "model.default_service_tier = None",
    "WebSearchToolType::Text",
    "model.use_responses_lite = false",
    "model.tool_mode = None",
    "AMAZON_BEDROCK_GPT_6_SOL_MODEL_ID",
    "AMAZON_BEDROCK_GPT_6_LUNA_MODEL_ID",
    "configured_bedrock_catalogs_normalize_unsupported_model_capabilities",
    "model.multi_agent_version = version",
    "Some(MultiAgentVersion::V2)",
  ],
  "codex-rs/model-provider/src/provider.rs": [
    "RemoteCompactionSupport",
    "ProviderCapabilities",
    "remote_compaction",
    "RemoteCompactionSupport::V2",
    "amazon_bedrock_provider_creates_static_models_manager",
    "openai.gpt-5.5",
    "openai.gpt-5.6-sol",
    "openai.gpt-5.6-terra",
    "openai.gpt-5.6-luna",
    "openai.gpt-6.1-sol",
    "openai.gpt-6-sol",
    "openai.gpt-6-luna",
    "include_internal_metadata",
    "url.scheme() == \"https\"",
    "host == \"api.openai.com\"",
    "codex_http_client::is_allowed_chatgpt_host(host)",
  ],
  "codex-rs/model-provider/src/capabilities.rs": [
    "ProviderCapabilities",
    "RemoteCompactionSupport",
    "remote_compaction",
    "pub struct ProviderCapabilities",
    "pub external_web_access: bool",
    "pub remote_compaction: RemoteCompactionSupport",
    "is_azure_responses_provider",
    "RemoteCompactionSupport::V2",
    "RemoteCompactionSupport::Unsupported",
    "overrides\n                .external_web_access",
    "overrides\n                .remote_compaction",
    "unwrap_or(defaults.external_web_access)",
    "unwrap_or(defaults.remote_compaction)",
  ],
  "codex-rs/core/src/realtime_conversation.rs": [
    "ConversationStartTransport::Websocket",
    "realtime_request_headers",
    "build_session_headers",
    "RealtimeWsVersion::V1",
    "openai-alpha",
    "quicksilver=v1",
  ],
  "codex-rs/codex-api/src/endpoint/realtime_call.rs": [
    "RealtimeCallClient",
    "realtime/calls",
    "create_with_session_and_headers",
    "configure_realtime_call_request",
    "intent",
    "quicksilver",
    "architecture",
    "avas",
    "validate_avas_session_config",
    "AVAS realtime calls require realtime v1",
  ],
  "codex-rs/codex-api/src/safety_buffering.rs": [
    "SafetyBufferingTreatment",
    "X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER",
    "x-codex-safety-buffering-enabled",
    "X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER",
    "x-codex-safety-buffering-faster-model",
    "treatment_from_headers",
    "faster_model",
  ],
  "codex-rs/codex-api/src/sse/responses.rs": [
    "spawn_response_stream",
    "process_sse",
    "process_responses_event",
    "treatment_from_headers",
    "SafetyBufferingTreatment",
    "with_treatment",
    "x-codex-turn-state",
    "response.completed",
    "response.failed",
    "response.metadata",
    "openai-model",
    "x-reasoning-included",
    "X-Models-Etag",
    "bio_policy",
    "SafetyBuffering",
    "safety_buffering",
    "ResponseEvent::SafetyBuffering",
    "invalid_prompt",
    "ApiError::InvalidPrompt",
    "response.output_item.added",
    "phase",
    "MessagePhase::PartialAnswer",
    "partial_answer",
  ],
  "codex-rs/codex-api/src/sse/responses_error.rs": [
    "parse_failed_response",
    "insufficient_quota",
    "credit_balance_exhausted",
    "organization_spend_limit_exceeded",
    "project_spend_limit_exceeded",
    "rate_limit_exceeded",
    "slow_down",
    "server_is_overloaded",
    "invalid_prompt",
    "ApiError::InvalidPrompt",
    "json_headers_to_http_headers",
    "let retry_after_header = error",
    ".and_then(|error| error.get(\"headers\"))",
    "RetryAfter::from_headers(&json_headers_to_http_headers(headers))",
    "retry_after_header\n                .or_else",
    "Duration::try_from_secs_f64(value).ok()",
  ],
  "codex-rs/codex-api/src/endpoint/responses.rs": [
    "ResponsesClient",
    "ResponsesOptions",
    "turn_state",
    "stream_request",
    "stream_encoded_json_with",
    "\"/responses\"",
    "spawn_response_stream",
  ],
  "codex-rs/codex-api/src/endpoint/responses_websocket.rs": [
    "ResponsesWebsocketConnection",
    "websocket_url_for_path(\"/responses\")",
    "merge_request_headers",
    "add_auth_headers",
    "x-codex-turn-state",
    "response.completed",
    "codex.rate_limits",
    "openai-model",
    "x-reasoning-included",
    "x-models-etag",
    "parse_wrapped_websocket_error_event",
    "websocket_connection_limit_reached",
    "PREVIOUS_RESPONSE_NOT_FOUND_CODE",
    "previous_response_not_found",
    "PREVIOUS_RESPONSE_NOT_FOUND_MESSAGE",
    "serialize_websocket_request",
    "SafetyBuffering",
    "safety_buffering",
    "ResponseEvent::SafetyBuffering",
    "treatment_from_headers",
    "SafetyBufferingTreatment",
    "safety_buffering(treatment)",
    "WsError::Http(response)",
    "let headers = response.headers().clone()",
    "let retry_after = RetryAfter::from_headers(&headers)",
    "headers: Some(headers)",
    "retry_after,",
    "WrappedWebsocketError",
    "headers: Option<Value>,",
    "error.headers.as_ref()",
    "headers.as_ref(),",
    "find_map(|headers| RetryAfter::from_headers(&json_headers_to_http_headers(headers)))",
  ],
  "codex-rs/core/src/context/base_instructions.rs": [
    "pub(crate) const KIND: &str = \"model.base_instructions\";",
    "pub(crate) fn matches_item(item: &ResponseItem) -> bool",
    "internal_chat_message_metadata_passthrough: Some(metadata)",
    "kind.as_str() == Self::KIND",
  ],
  "codex-rs/core/src/context/world_state/base_instructions.rs": [
    "pub(crate) struct BaseInstructionsState(pub(crate) String);",
    "const ID: &'static str = \"base_instructions\";",
    "WorldStateUpdate::optional_prefix_boxed_fragment",
    "PreviousSectionState::Absent",
  ],
  "codex-rs/core/src/context/world_state/top_level_tools.rs": [
    "pub(crate) struct TopLevelToolsState",
    "const NAMESPACE_UPDATE_HINT: &str",
    "WorldStateUpdate::prefix_item",
    "REMOVED_NAMESPACES_HEADER",
    "REMOVED_TOOLS_HEADER",
    "Some(self.hashes.clone())",
  ],
  "codex-rs/core/src/tools/spec_plan.rs": [
    "merge_into_namespaces(specs)",
    "model_info.supports_search_tool",
    "tool_exposure_with_namespace_override",
    "code_mode_only_strict_3p_tools",
    "turn_context.config.multi_agent_v2.tool_namespace.as_deref()",
    "turn_context.provider.capabilities().image_generation",
  ],
  "codex-rs/protocol/src/turn_input.rs": [
    "pub struct TurnAttribution",
    "pub initiating_agent_path: Option<AgentPath>",
    "pub root_turn_id: Option<String>",
    "pub fn start_options(&self) -> TurnStartOptions",
    "Started {",
    "Steered {",
  ],
  "codex-rs/protocol/src/environment.rs": [
    "pub struct TurnEnvironmentRequest",
    "pub struct TurnEnvironmentRequests",
    "pub fn new(",
    "legacy_fallback_cwd",
  ],
  "codex-rs/rollout-trace/src/reducer/conversation/normalize.rs": [
    "fn channel_from_phase(phase: &str)",
    "\"partial_answer\" | \"final_answer\"",
    "ConversationChannel::Final",
  ],
  "codex-rs/config/src/project_trust.rs": [
    "ProjectTrustPath",
    "ProjectTrustLookup",
    "from_native_path",
    "from_paths",
    "normalize_for_path_comparison",
    "canonical != original",
    "keys.push(canonical)",
    "keys.push(original)",
    "get_active_project_for_lookup",
    "normalize_lookup_key",
  ],
  "codex-rs/core/src/config/mod.rs": [
    "respect_system_proxy",
    "Feature::RespectSystemProxy",
    "resolve_bootstrap_respect_system_proxy",
    "AuthRouteConfig::from_http_client_factory",
    "http_client_factory",
    "OutboundProxyPolicy::RespectSystemProxy",
    "features.enabled",
    "feature_requirements",
  ],
  "codex-rs/features/src/lib.rs": [
    "RespectSystemProxy",
    "respect_system_proxy",
    "key: \"respect_system_proxy\"",
    "CompactionImageBudget",
    "compaction_image_budget",
    "key: \"compaction_image_budget\"",
    "default_enabled: true",
    "Feature::InstantInterrupt",
    "key: \"instant_interrupt\"",
    "Stage::UnderDevelopment",
    "default_enabled: false",
    "id: Feature::CliDaybreak,\n        key: \"cli_daybreak\",\n        stage: Stage::UnderDevelopment,\n        default_enabled: false,",
  ],
  "codex-rs/app-server-protocol/src/protocol/v2/mcp.rs": [
    "McpServerEventStreamStartParams",
    "McpServerEventStreamStopParams",
    "McpServerEventNotification",
    "McpServerEventStreamNotification",
    "subscription_id",
  ],
  "codex-rs/app-server/src/request_processors/mcp_event_stream.rs": [
    "MAX_MCP_EVENT_STREAMS_PER_CONNECTION",
    "MCP_EVENT_STREAM_STARTUP_TIMEOUT",
    "McpEventStreams",
    "McpServerEventStreamStartParams",
  ],
  "codex-rs/app-server/src/request_processors/account_processor/workspace_routing.rs": [
    "workspace backend must use an HTTPS origin without credentials",
    "effective_chatgpt_base_url",
    "config.chatgpt_base_url.clone()",
    "get_accounts_check",
    "workspace_backend_origin",
    "resolve_routing",
    "parse_backend_url",
    "url.scheme() != \"https\"",
    "!url.username().is_empty()",
    "url.password().is_some()",
  ],
  "codex-rs/http-client/src/outbound_proxy.rs": [
    "OutboundProxyPolicy",
    "RespectSystemProxy",
    "HttpClientFactory",
    "build_reqwest_client_for_route",
    "ClientRouteClass",
    "RouteFailureClass",
    "SystemProxyDecision",
    "resolve_system_proxy",
    "resolve_platform_system_proxy",
    "Sha256",
    "no_proxy",
    "target_os = \"macos\"",
    "mod macos",
  ],
  "codex-rs/http-client/src/outbound_proxy/macos.rs": [
    "SCDynamicStoreBuilder",
    "CFNetworkCopyProxiesForURL",
    "CFNetworkExecuteProxyAutoConfigurationURL",
    "CFNetworkExecuteProxyAutoConfigurationScript",
    "PAC_EXECUTION_TIMEOUT",
    "proxy_array_decision",
    "proxy_entry_decision",
    "kCFProxyTypeAutoConfigurationURL",
    "kCFProxyTypeAutoConfigurationJavaScript",
    "kCFProxyTypeHTTPS",
    "kCFProxyTypeSOCKS",
    "UnsupportedProxyScheme",
    "RouteFailureClass",
  ],
  "codex-rs/http-client/src/outbound_proxy/windows.rs": [
    "WinHttpGetIEProxyConfigForCurrentUser",
    "WinHttpGetProxyForUrl",
    "WINHTTP_AUTOPROXY_CONFIG_URL",
    "WINHTTP_AUTOPROXY_AUTO_DETECT",
    "WINHTTP_ACCESS_TYPE_NAMED_PROXY",
    "WINHTTP_ACCESS_TYPE_NO_PROXY",
    "proxy_list_decision",
    "proxy_bypass_matches_origin",
    "ParsedProxyListDecision",
    "<local>",
    "WinHttpOpen",
    "GlobalFree",
  ],
  "codex-rs/core-plugins/src/manifest.rs": [
    "RawPluginManifestInterface",
    "logo_dark",
    "logoDark",
    "interface.logoDark",
    "resolve_interface_asset_path",
    "PluginManifestInterface",
    "AGENT_PLUGIN_MANIFEST_RELATIVE_PATH",
    "parse_agent_plugin_manifest_uri",
    "parse_resolved_plugin_manifest_uri",
  ],
  "codex-rs/plugin/src/manifest.rs": [
    "PluginManifestInterface",
    "logo_dark",
    "pub logo_dark: Option<Resource>",
  ],
  "codex-rs/ext/web-search/src/extension.rs": [
    "WebSearchExtensionConfig",
    "supports_standalone_web_search",
    "web_search_mode",
    "WebSearchMode::Disabled",
    "create_model_provider",
    "WebSearchTool",
  ],
  "codex-rs/ext/web-search/src/tool.rs": [
    "ToolExposure::Direct",
    "SearchOutput",
    "response.output",
  ],
  "codex-rs/codex-api/src/endpoint/search.rs": [
    "SearchClient",
    "alpha/search",
    "Method::POST",
    "SearchRequest",
    "SearchResponse",
  ],
  "codex-rs/tools/src/json_schema.rs": [
    "anyOf",
    "oneOf",
    "allOf",
    "compact_large_tool_schema",
  ],
  "codex-rs/tools/src/json_schema/compaction.rs": [
    "compact_large_tool_schema",
    "LARGE_SCHEMA_COMPACTION_PASSES",
    "DEFAULT_COMPACT_TOOL_SCHEMA_BYTES",
    "MAX_COMPACT_TOOL_SCHEMA_DEPTH",
    "prune_schema_compositions",
  ],
  "codex-rs/exec/src/cli.rs": [
    "ThreadSource",
    "long = \"thread-source\"",
    "value_name = \"SOURCE\"",
    "global = true",
    "pub thread_source: Option<ThreadSource>",
    "#[arg(long, value_enum, value_name = \"PROGRAM\", global = true)]",
    "pub cyber_access_program: Option<CyberAccessProgramCliArg>",
    "pub enum CyberAccessProgramCliArg",
    "Standard,",
    "DaybreakBlue,",
    "DaybreakRed,",
    "#[value(rename_all = \"snake_case\")]",
  ],
  "codex-rs/exec/src/lib.rs": [
    "ThreadSource::User",
    "ThreadForkParams",
    "thread_start_params_from_config",
    "thread_source: Some(thread_source.clone())",
    "let cyber_access_program = match cyber_access_program {",
    "Some(program) => Some(program),",
    "None if config.features.enabled(Feature::CliDaybreak) => {",
    "daybreak::program_for_turn(",
    "None => None,",
    "daybreak_override",
    "ClientRequest::TurnStart",
  ],
  "codex-rs/protocol/src/protocol.rs": [
    "pub enum ThreadSource",
    "Feature(String)",
    "\"memory_consolidation\"",
    "other => Ok(ThreadSource::Feature(other.to_string()))",
  ],
  "codex-rs/app-server/README.md": [
    "userVerification/cancel",
    "experimentalApi",
    "thread/attachment/add",
    "thread/attachment/list",
    "thread/attachment/remove",
    "thread/attachment/updated",
    "thread/archive",
    "thread/delete",
  ],
  "codex-rs/app-server-protocol/src/export.rs": [
    "generate_internal_json_schema",
    "JsonSchemaEmitter",
    "schema_for",
    "GeneratedSchema",
  ],
  "codex-rs/app-server-protocol/src/rpc.rs": [
    "JSONRPCMessage",
    "JSONRPCRequest",
    "JSONRPCResponse",
    "JSONRPCNotification",
    "JSONRPCError",
    "JsonSchema",
    "jsonrpc",
  ],
  "codex-rs/app-server-protocol/src/protocol/common.rs": [
    "ThreadStart => \"thread/start\"",
    "ThreadResume => \"thread/resume\"",
    "ThreadFork => \"thread/fork\"",
    "ThreadQueueAdd => \"thread/queue/add\"",
    "TurnStart => \"turn/start\"",
    "ThreadStarted => \"thread/started\"",
    "ThreadQueueChanged => \"thread/queue/changed\"",
    "TurnStarted => \"turn/started\"",
    "JsonSchema",
    "ClientRequest",
    "ServerNotification",
  ],
  "codex-rs/app-server-protocol/src/protocol/v2/thread.rs": [
    "ThreadStartParams",
    "ThreadForkParams",
    "pub thread_source: Option<ThreadSource>",
    "Optional client-supplied analytics source classification",
    "ThreadItemEntry",
    "pub started_at_ms: Option<i64>",
    "pub completed_at_ms: Option<i64>",
    "ThreadItemsListParams",
    "pub cursor: Option<ThreadItemsListCursor>",
    "ThreadItemsListCursor",
    "Anchor(ThreadItemsListAnchor)",
    "ThreadItemsListAnchor",
    "item_id: String",
  ],
  "codex-rs/app-server-protocol/src/protocol/v2/turn.rs": [
    "pub parent_turn_id: Option<String>",
    "pub root_turn_id: Option<String>",
    "pub environments: Option<Vec<TurnEnvironmentParams>>",
  ],
  "codex-rs/app-server-protocol/src/protocol/v2/environment.rs": [
    "EnvironmentAddParams",
    "pub skills: Option<EnvironmentSkillsParams>",
    "pub struct EnvironmentSkillsParams",
    "pub required: Option<Vec<String>>",
    "checked before model inference",
  ],
  "codex-rs/app-server/src/message_processor.rs": [
    "reject_obsolete_request_fields",
    "reject_removed_permission_profile",
    "thread/start\" | \"thread/resume\" | \"thread/fork\" | \"turn/start",
    "permissionProfile",
    "use `permissions` with a named profile id instead",
  ],
  "codex-rs/app-server/src/request_processors/initialize_processor.rs": [
    "InitializeRequestProcessor",
    "initialize",
    "Already initialized",
    "send_initialize_notifications_to_connection",
    "track_initialized_request",
  ],
  "codex-rs/app-server/src/request_processors/thread_processor.rs": [
    "ThreadRequestProcessor",
    "ThreadStartParams",
    "ThreadResumeParams",
    "ThreadForkParams",
    "ThreadCompactStartParams",
    "ConnectionRequestId",
  ],
  "codex-rs/app-server/src/request_processors/turn_processor.rs": [
    "TurnRequestProcessor",
    "TurnStartParams",
    "TurnStartResponse",
    "turn/start",
    "TurnStatus::InProgress",
    "ThreadSettingsBuildParams",
    "TurnEnvironmentRequests",
    "resolve_turn_environment_requests",
    "parent_turn_id: params.parent_turn_id",
    "root_turn_id: params.root_turn_id",
    "root_turn_id: Some(root_turn_id)",
  ],
  "codex-rs/app-server/src/request_processors/environment_processor.rs": [
    "EnvironmentAddParams",
    "ScopedSkillsConfig",
    ".skills",
    "upsert_environment_with_options(params.environment_id, options, skills)",
  ],
  "codex-rs/app-server/src/request_serialization.rs": [
    "ClientRequestSerializationScope",
    "RequestSerializationQueueKey",
    "Thread",
    "ThreadPath",
    "RequestSerializationAccess",
    "QueuedInitializedRequest",
  ],
  "codex-rs/utils/process/src/lib.rs": [
    "pub fn background_command",
    "CREATE_NO_WINDOW",
    "command.creation_flags",
    "Command::new(program)",
  ],
  "codex-rs/core/src/spawn.rs": [
    "StdioPolicy::RedirectForShellTool",
    "codex_utils_process::background_command",
    "StdioPolicy::Inherit",
    "Command::new(&program)",
  ],
  "codex-rs/tui/src/security_setup.rs": [
    "Optional account-security reminders for the local, ChatGPT-authenticated CLI.",
    "config.model_provider_id != \"openai\" || server.uses_remote_workspace()",
    "ClientRequest::GetAuthStatus",
    "GetAuthStatusParams",
    "!matches!(auth, CodexAuth::Chatgpt(_)) || auth.is_fedramp_account()",
    "status.auth_method != Some(AuthMode::Chatgpt)",
    "status.auth_token.as_deref() != Some(saved_token.as_str())",
    "RouteAwareClientPool::new_without_redirects",
    "{}/wham/security-setup",
    "config.chatgpt_base_url.trim_end_matches('/')",
    "auth_provider_from_auth(&auth).to_auth_headers()",
    "url.host_str() == Some(\"chatgpt.com\")",
    "Duration::from_secs(3)",
  ],
  "codex-rs/models-manager/src/manager.rs": [
    "Only explicit catalogs serialize refresh and suppress bundled fallback.",
    "Self::ExplicitProvider(_) => None",
    "pub fn with_provider_catalog(mut self) -> Self",
    "self.remote_models.get_mut().models.clear();",
    "matches!(&self.catalog_source, CatalogSource::ExplicitProvider(_))",
    "current.models.clear();",
    "if !remote_only && let Some(mut models) = self.catalog_source.fallback_models()",
  ],
  "codex-rs/tui/src/app_server_session/provider_selection.rs": [
    "Provider request overrides honor managed requirements over explicit invocation choices.",
    "required_model_provider()",
    ".or_else(|| explicit_provider(config))",
    "pub(crate) async fn history_model_provider",
    "read_effective_config_if_supported",
    ".model_provider",
    "unwrap_or_else(|| \"openai\".to_string())",
  ],
  "codex-rs/tui/src/projectless.rs": [
    "Select desktop-like execution defaults for positively discovered local projectless folders.",
    "config.config_layer_stack.is_projectless()",
    "config.active_project.trust_level.is_some()",
    "config.workspace_roots.len() != 1",
    "config.workspace_roots.first() != Some(&config.cwd)",
    "has_only_local_environments(environments)",
    "PermissionProfile::workspace_write()",
    "set_permission_profile_from_session_snapshot",
  ],
  "codex-rs/tui/src/resume_permissions.rs": [
    "Omitted choices let app-server restore the destination task's saved settings.",
    "ConfigLayerSource::SessionFlags",
    "overrides.approval_policy.is_some() || has(\"approval_policy\")",
    "overrides.approvals_reviewer.is_some() || has(\"approvals_reviewer\")",
    "overrides.permission_profile.is_some()",
    "workspace_roots: overrides.cwd.is_some()",
    "has(\"sandbox_workspace_write.writable_roots\")",
  ],
  "codex-rs/core/src/session/environment.rs": [
    "follow_inherited_environment_configurations",
    "starting.owner_configuration()",
    "ConfigUpdateSource::Inherited",
    "no one will retry this child's one-time update",
    "!inherited || matches!(environment.config, EnvironmentConfigState::Pending)",
    "if inherited && matches!(environments, (None, None))",
  ],
  "codex-rs/core/src/agent/control/spawn.rs": [
    "inherited_environments_for_source",
    "let inherited_environments = self",
    "environment_selections: None",
    "inherited_environments,",
    "resume_thread_with_history_with_source",
  ],
  "codex-rs/rmcp-client/src/stdio_server_launcher.rs": [
    "fn remote_env_policy(remote_env_vars: &[String])",
    "crate::utils::DEFAULT_ENV_VARS",
    "remote_env_vars.iter().cloned()",
    "include_only,",
    "env.get(\"REMOTE_TOKEN\")",
    "assert!(!env.contains_key(\"UNREQUESTED_SECRET\"))",
    ".chain([\"SYSTEMROOT\", \"TEMP\", \"TMP\"].iter())",
    "env.get(\"SystemRoot\")",
    "env.get(name)",
  ],

  "codex-rs/core/src/responses_retry.rs": [
    "ResponsesStreamRetryState",
    "ResponsesStreamRequest::RemoteCompactionV2",
    "let retry_after = err.retry_after()",
    "retry_state.retries >= max_retries",
    "try_switch_fallback_transport",
    "tokio::time::sleep_until(retry_after.deadline()).await",
    "retry_state.retries < max_retries",
    "retry_after.map(RetryAfter::deadline).unwrap_or(now + delay)",
    "ExhaustedResponseRetry",
    "turn_id: turn_context.sub_id.clone()",
  ],
  "codex-rs/codex-api/src/common.rs": [
    "pub struct ResponsesApiRequest",
    "pub struct ResponseCreateWsRequest",
    "pub model: String,\n    pub stream: bool,",
    "pub model: &'a str,\n    pub stream: bool,",
    "pub service_tier: Option<String>",
    "pub service_tier: Option<&'a str>",
    "pub previous_response_id: Option<String>",
  ],
  "codex-rs/core/src/cyber_access_program.rs": [
    "pub(crate) fn for_provider(",
    "program.filter(|_| provider_id == OPENAI_PROVIDER_ID)",
    "if config.model_provider_id != OPENAI_PROVIDER_ID {",
    "Feature::ApiKeyCyberAccessPrograms",
    "if auth.is_chatgpt_auth()",
    "if !auth.is_api_key_auth()",
  ],
};

const REQUIRED_EXPECTED_HEADERS = [
  "session_id",
  "x-openai-subagent",
  "x-openai-memgen-request",
  "x-codex-installation-id",
  "x-codex-turn-state",
  "x-codex-turn-metadata",
  "x-codex-parent-thread-id",
  "x-codex-window-id",
  "x-client-request-id",
  "x-codex-beta-features",
  "x-responsesapi-include-timing-metrics",
  "x-openai-internal-codex-responses-lite",
  "ws_request_header_x_openai_internal_codex_responses_lite",
  "x-codex-ws-stream-request-start-ms",
  "OpenAI-Beta",
  "User-Agent",
];

const REQUIRED_PRESERVED_TRANSPARENCY_HEADERS = [
  "session_id",
  "x-openai-subagent",
  "x-codex-turn-state",
  "x-codex-turn-metadata",
  "x-codex-beta-features",
  "x-openai-internal-codex-responses-lite",
  "ws_request_header_x_openai_internal_codex_responses_lite",
  "x-codex-ws-stream-request-start-ms",
  "User-Agent",
];

const REQUIRED_PROXY_REPLACED_HEADERS = ["Authorization", "ChatGPT-Account-Id"];

const REQUIRED_PROXY_SKIPPED_HEADERS = [
  "Host",
  "Connection",
  "Content-Length",
  "Transfer-Encoding",
  "Upgrade",
  "sec-websocket-*",
];

const REQUIRED_EXPECTED_ROUTES = [
  "/responses",
  "/realtime/calls",
  "alpha/search",
  "/memories/trace_summarize",
  "websocket_url_for_path(\"/responses\")",
];

const REQUIRED_APP_SERVER_METHODS = [
  "initialize",
  "initialized",
  "thread/start",
  "thread/resume",
  "thread/fork",
  "thread/queue/add",
  "thread/queue/changed",
  "turn/start",
  "turn/cancel",
  "mcpServer/event/stream/start",
  "mcpServer/event/stream/stop",
];

const REQUIRED_STREAM_EVENTS = [
  "response.created",
  "response.in_progress",
  "response.queued",
  "response.output_item.added",
  "response.content_part.added",
  "response.reasoning_summary_part.added",
  "response.completed",
  "response.failed",
  "response.metadata",
  "codex.rate_limits",
];

const COMPAT_FORMAT_VERSION_WITH_SEMANTIC_CHECKS = 2;

const REQUIRED_SEMANTIC_CHECKS = [
  {
    id: "responses.http-route",
    kind: "route",
    file: "codex-rs/codex-api/src/endpoint/responses.rs",
    file_contains_all: [
      "ResponsesClient",
      "stream_request",
      "stream_encoded_json_with",
      "\"/responses\"",
    ],
    expected_routes_all: ["/responses"],
  },
  {
    id: "realtime.call-avas-route",
    kind: "route",
    file: "codex-rs/codex-api/src/endpoint/realtime_call.rs",
    file_contains_all: [
      "RealtimeCallClient",
      "realtime/calls",
      "create_with_session_and_headers",
      "configure_realtime_call_request",
      "intent",
      "quicksilver",
      "architecture",
      "avas",
    ],
    expected_routes_all: ["/realtime/calls"],
  },
  {
    id: "sse.responses-http-route-behavior",
    kind: "route_event_group",
    file: "codex-rs/codex-api/src/sse/responses.rs",
    file_contains_all: [
      "spawn_response_stream",
      "process_sse",
      "process_responses_event",
      "treatment_from_headers",
      "SafetyBufferingTreatment",
      "with_treatment",
      "x-codex-turn-state",
      "openai-model",
      "x-reasoning-included",
      "X-Models-Etag",
    ],
    expected_routes_all: ["/responses"],
    expected_stream_events_all: [
      "response.created",
      "response.completed",
      "response.failed",
      "response.metadata",
    ],
  },
  {
    id: "responses.internal-metadata-destination-gate",
    kind: "security_boundary",
    file: "codex-rs/model-provider/src/provider.rs",
    file_contains_all: [
      "include_internal_metadata",
      "url.scheme() == \"https\"",
      "host == \"api.openai.com\"",
      "codex_http_client::is_allowed_chatgpt_host(host)",
    ],
  },
  {
    id: "responses.internal-metadata-stripping",
    kind: "security_boundary",
    file: "codex-rs/core/src/client.rs",
    file_contains_all: [
      "include_internal",
      ".include_internal_metadata(&client_setup.api_provider)",
      "if !include_internal",
      "clear_tool_result_metadata",
      "responses_metadata.client_metadata(include_internal)",
    ],
  },
  {
    id: "compact.remote-responses-v2",
    kind: "route",
    file: "codex-rs/core/src/compact_remote_v2.rs",
    file_contains_all: [
      "run_remote_compact_task",
      "run_inline_remote_auto_compact_task",
      "run_remote_compaction_request_v2",
      "RequestEffortUsage::Compaction",
      "CompactionImplementation::ResponsesCompactionV2",
    ],
    expected_routes_all: ["/responses"],
  },
  {
    id: "compact.trigger-request",
    kind: "metadata_group",
    file: "codex-rs/core/src/compact_remote_v2_attempt.rs",
    file_contains_all: [
      "ResponseItem::CompactionTrigger {}",
      "compaction_responses_metadata",
      "responses_metadata",
      "run_remote_compaction_request_v2",
    ],
    expected_routes_all: ["/responses"],
    expected_headers_all: ["x-codex-turn-metadata"],
  },
  {
    id: "client.conversation-headers",
    kind: "header_group",
    file: "codex-rs/core/src/client.rs",
    file_contains_all: [
      "build_responses_headers",
      "build_responses_compatibility_headers",
      "build_ws_client_metadata",
      "build_session_headers",
      "x-codex-installation-id",
      "x-codex-turn-state",
      "x-codex-turn-metadata",
      "x-codex-parent-thread-id",
      "x-codex-window-id",
      "x-openai-memgen-request",
      "x-openai-subagent",
      "x-responsesapi-include-timing-metrics",
      "x-openai-internal-codex-responses-lite",
      "ws_request_header_x_openai_internal_codex_responses_lite",
      "x-codex-ws-stream-request-start-ms",
      "x-client-request-id",
    ],
    expected_headers_all: [
      "x-codex-installation-id",
      "x-codex-turn-state",
      "x-codex-turn-metadata",
      "x-codex-parent-thread-id",
      "x-codex-window-id",
      "x-openai-memgen-request",
      "x-openai-subagent",
      "x-responsesapi-include-timing-metrics",
      "x-openai-internal-codex-responses-lite",
      "ws_request_header_x_openai_internal_codex_responses_lite",
      "x-codex-ws-stream-request-start-ms",
      "x-client-request-id",
    ],
  },
  {
    id: "proxy.preserved-headers",
    kind: "header_group",
    file: "codex-rs/core/src/client.rs",
    file_contains_all: [
      "build_responses_headers",
      "build_responses_compatibility_headers",
      "build_session_headers",
      "x-openai-subagent",
      "x-codex-turn-state",
      "x-codex-turn-metadata",
      "x-codex-beta-features",
      "x-openai-internal-codex-responses-lite",
      "ws_request_header_x_openai_internal_codex_responses_lite",
      "x-codex-ws-stream-request-start-ms",
      "OPENAI_BETA_HEADER",
    ],
    expected_headers_all: REQUIRED_PRESERVED_TRANSPARENCY_HEADERS,
  },
  {
    id: "client.websocket-beta",
    kind: "co_occurrence",
    file: "codex-rs/core/src/client.rs",
    file_contains_all: ["stream_responses_websocket", "OPENAI_BETA_HEADER", "responses_websockets=2026-02-06"],
    expected_headers_all: ["OpenAI-Beta"],
  },
  {
    id: "realtime.websocket-v1-alpha-header",
    kind: "header_behavior",
    file: "codex-rs/core/src/realtime_conversation.rs",
    file_contains_all: [
      "ConversationStartTransport::Websocket",
      "realtime_request_headers",
      "build_session_headers",
      "RealtimeWsVersion::V1",
      "openai-alpha",
      "quicksilver=v1",
    ],
  },
  {
    id: "compact.image-budget-owned-by-codex",
    kind: "feature_gate",
    file: "codex-rs/core/src/compact_remote_v2.rs",
    file_contains_all: [
      "Feature::CompactionImageBudget",
      "RetainedImageBudget::Enabled",
      "truncate_retained_messages",
      "images::truncate_message_to_token_budget",
      "remaining = 0",
    ],
  },
  {
    id: "models.cache-provider-auth-identity",
    kind: "cache_identity",
    file: "codex-rs/model-provider/src/models_identity.rs",
    file_contains_all: [
      "models-cache-v1",
      "provider.name",
      "provider.base_url",
      "provider.query_params",
      "get_account_id",
      "get_chatgpt_user_id",
      "get_account_email",
      "account_plan_type",
      "resolve_provider_auth",
      "has_stable_account",
    ],
  },
  {
    id: "models.cache-entry-identity",
    kind: "cache_identity",
    file: "codex-rs/models-manager/src/cache.rs",
    file_contains_all: [
      "ModelsCacheEntry",
      "identity: Option<String>",
      "refresh_ttl",
      "entry.identity",
      "client_version",
      "etag",
    ],
  },
  {
    id: "turn-metadata.request-kind-window",
    kind: "metadata_group",
    file: "codex-rs/core/src/turn_metadata.rs",
    file_contains_all: [
      "request_kind",
      "window_id",
      "Turn",
      "Memory",
      "ThreadSource",
      "thread_source",
      "CodexResponsesMetadata",
      "to_responses_metadata",
    ],
    expected_headers_all: ["x-codex-turn-metadata"],
  },
  {
    id: "turn-metadata.memory-consolidation",
    kind: "metadata_group",
    file: "codex-rs/core/src/turn_metadata.rs",
    file_contains_all: [
      "detached_memory_responses_metadata",
      "CodexResponsesRequestKind::Memory",
      "ThreadSource::MemoryConsolidation",
    ],
    expected_headers_all: ["x-codex-turn-metadata"],
  },
  {
    id: "turn-metadata.compaction-dispatch",
    kind: "metadata_group",
    file: "codex-rs/core/src/responses_metadata.rs",
    file_contains_all: [
      "COMPACTION_KEY",
      "compaction",
      "CompactionTurnMetadata",
      "CompactionTrigger",
      "CompactionReason",
      "CompactionImplementation",
      "CompactionPhase",
      "CompactionStrategy",
      "CodexResponsesRequestKind",
      "ThreadSource",
      "THREAD_SOURCE_KEY",
      "thread_source",
      "LEGACY_CODE_MODE_TOOL_NAMES_KEY",
      "code_mode_tool_names",
      "tool_namespaces_info: None",
      "turn_metadata_payload",
      "X_CODEX_TURN_METADATA_HEADER",
    ],
    expected_headers_all: ["x-codex-turn-metadata"],
  },
  {
    id: "model-provider.remote-compaction-capability",
    kind: "capability_gate",
    file: "codex-rs/model-provider/src/capabilities.rs",
    file_contains_all: [
      "RemoteCompactionSupport",
      "ProviderCapabilities",
      "remote_compaction",
      "is_azure_responses_provider",
      "RemoteCompactionSupport::V2",
    ],
  },
  {
    id: "model-provider.bedrock-gpt-6-catalog",
    kind: "provider_catalog",
    file: "codex-rs/model-provider/src/amazon_bedrock/catalog.rs",
    file_contains_all: [
      "static_model_catalog",
      "normalize_bedrock_catalog",
      "fn bedrock_model(",
      "AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID",
      "AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID",
      "AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID",
      "AMAZON_BEDROCK_GPT_6_1_SOL_MODEL_ID",
      "model.additional_speed_tiers.clear()",
      "model.service_tiers.clear()",
      "model.default_service_tier = None",
      "WebSearchToolType::Text",
      "model.use_responses_lite = false",
      "model.tool_mode = None",
      "AMAZON_BEDROCK_GPT_6_SOL_MODEL_ID",
      "AMAZON_BEDROCK_GPT_6_LUNA_MODEL_ID",
      "configured_bedrock_catalogs_normalize_unsupported_model_capabilities",
      "model.multi_agent_version = version",
      "Some(MultiAgentVersion::V2)",
    ],
  },
  {
    id: "tools.large-schema-compaction",
    kind: "tool_schema",
    file: "codex-rs/tools/src/json_schema/compaction.rs",
    file_contains_all: [
      "compact_large_tool_schema",
      "LARGE_SCHEMA_COMPACTION_PASSES",
      "DEFAULT_COMPACT_TOOL_SCHEMA_BYTES",
      "MAX_COMPACT_TOOL_SCHEMA_DEPTH",
      "prune_schema_compositions",
    ],
  },
  {
    id: "model-provider.bedrock-static-manager-models",
    kind: "provider_catalog",
    file: "codex-rs/model-provider/src/provider.rs",
    file_contains_all: [
      "amazon_bedrock_provider_creates_static_models_manager",
      "openai.gpt-5.5",
      "openai.gpt-5.6-sol",
      "openai.gpt-5.6-terra",
      "openai.gpt-5.6-luna",
      "openai.gpt-6.1-sol",
      "openai.gpt-6-sol",
      "openai.gpt-6-luna",
    ],
  },
  {
    id: "sse.responses-events",
    kind: "event_group",
    file: "codex-rs/codex-api/src/sse/responses.rs",
    file_contains_all: [
      "process_responses_event",
      "treatment_from_headers",
      "SafetyBufferingTreatment",
      "with_treatment",
      "response.completed",
      "response.failed",
      "response.metadata",
      "SafetyBuffering",
      "safety_buffering",
      "ResponseEvent::SafetyBuffering",
      "invalid_prompt",
      "ApiError::InvalidPrompt",
    ],
    expected_stream_events_all: [
      "response.created",
      "response.completed",
      "response.failed",
      "response.metadata",
    ],
  },
  {
    id: "sse.quota-codes",
    kind: "co_occurrence",
    file: "codex-rs/codex-api/src/sse/responses_error.rs",
    file_contains_all: [
      "insufficient_quota",
      "credit_balance_exhausted",
      "organization_spend_limit_exceeded",
      "project_spend_limit_exceeded",
      "rate_limit_exceeded",
      "slow_down",
      "server_is_overloaded",
    ],
  },
  {
    id: "web-search.custom-provider-capability",
    kind: "capability_gate",
    file: "codex-rs/ext/web-search/src/extension.rs",
    file_contains_all: [
      "WebSearchExtensionConfig",
      "supports_standalone_web_search",
      "web_search_mode",
      "WebSearchMode::Disabled",
      "create_model_provider",
      "WebSearchTool",
    ],
  },
  {
    id: "web-search.standalone-route",
    kind: "route",
    file: "codex-rs/codex-api/src/endpoint/search.rs",
    file_contains_all: ["SearchClient", "alpha/search", "Method::POST", "SearchRequest"],
    expected_routes_all: ["alpha/search"],
  },
  {
    id: "websocket.responses-route",
    kind: "route",
    file: "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
    file_contains_all: ["ResponsesWebsocketConnection", "websocket_url_for_path(\"/responses\")"],
    expected_routes_all: ["websocket_url_for_path(\"/responses\")"],
  },
  {
    id: "websocket.session-behavior",
    kind: "route_event_group",
    file: "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
    file_contains_all: [
      "ResponsesWebsocketConnection",
      "websocket_url_for_path(\"/responses\")",
      "merge_request_headers",
      "add_auth_headers",
      "treatment_from_headers",
      "SafetyBufferingTreatment",
      "safety_buffering(treatment)",
      "x-codex-turn-state",
      "serialize_websocket_request",
      "SafetyBuffering",
      "safety_buffering",
      "ResponseEvent::SafetyBuffering",
      "parse_wrapped_websocket_error_event",
      "websocket_connection_limit_reached",
      "PREVIOUS_RESPONSE_NOT_FOUND_CODE",
      "previous_response_not_found",
      "PREVIOUS_RESPONSE_NOT_FOUND_MESSAGE",
    ],
    expected_routes_all: ["websocket_url_for_path(\"/responses\")"],
    expected_headers_all: ["x-codex-turn-state"],
    expected_stream_events_all: [
      "response.created",
      "response.in_progress",
      "response.queued",
      "response.output_item.added",
      "response.content_part.added",
      "response.reasoning_summary_part.added",
      "response.completed",
      "response.failed",
      "codex.rate_limits",
    ],
  },
  {
    id: "websocket.responses-events",
    kind: "event_group",
    file: "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
    file_contains_all: [
      "response.completed",
      "codex.rate_limits",
      "treatment_from_headers",
      "SafetyBufferingTreatment",
      "safety_buffering(treatment)",
      "SafetyBuffering",
      "safety_buffering",
      "ResponseEvent::SafetyBuffering",
    ],
    expected_stream_events_all: [
      "response.created",
      "response.in_progress",
      "response.queued",
      "response.output_item.added",
      "response.content_part.added",
      "response.reasoning_summary_part.added",
      "response.completed",
      "response.failed",
      "codex.rate_limits",
    ],
  },
  {
    id: "websocket.header-auth-merge",
    kind: "header_group",
    file: "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
    file_contains_all: ["merge_request_headers", "add_auth_headers", "x-codex-turn-state"],
    expected_headers_all: ["x-codex-turn-state"],
    proxy_replaced_headers_all: ["Authorization", "ChatGPT-Account-Id"],
  },
  {
    id: "proxy.replaced-headers",
    kind: "header_group",
    file: "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
    file_contains_all: ["merge_request_headers", "add_auth_headers"],
    proxy_replaced_headers_all: ["Authorization", "ChatGPT-Account-Id"],
  },
  {
    id: "proxy.skipped-transport-headers",
    kind: "header_group",
    file: "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
    file_contains_all: ["merge_request_headers"],
    proxy_skipped_headers_all: REQUIRED_PROXY_SKIPPED_HEADERS,
  },
  {
    id: "safety-buffering.response-header-treatment",
    kind: "header_group",
    file: "codex-rs/codex-api/src/safety_buffering.rs",
    file_contains_all: [
      "treatment_from_headers",
      "X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER",
      "x-codex-safety-buffering-enabled",
      "X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER",
      "x-codex-safety-buffering-faster-model",
      "faster_model",
    ],
  },
  {
    id: "proxy.system-proxy-macos",
    kind: "capability_gate",
    file: "codex-rs/http-client/src/outbound_proxy/macos.rs",
    file_contains_all: [
      "SCDynamicStoreBuilder",
      "CFNetworkCopyProxiesForURL",
      "CFNetworkExecuteProxyAutoConfigurationURL",
      "CFNetworkExecuteProxyAutoConfigurationScript",
      "PAC_EXECUTION_TIMEOUT",
      "proxy_array_decision",
      "UnsupportedProxyScheme",
    ],
  },
  {
    id: "plugins.dark-mode-logo",
    kind: "metadata_group",
    file: "codex-rs/core-plugins/src/manifest.rs",
    file_contains_all: [
      "RawPluginManifestInterface",
      "logo_dark",
      "logoDark",
      "interface.logoDark",
      "resolve_interface_asset_path",
      "PluginManifestInterface",
      "AGENT_PLUGIN_MANIFEST_RELATIVE_PATH",
      "parse_agent_plugin_manifest_uri",
      "parse_resolved_plugin_manifest_uri",
    ],
  },
  {
    id: "config.project-trust-canonical-before-original",
    kind: "trust_lookup",
    file: "codex-rs/config/src/project_trust.rs",
    file_contains_all: [
      "ProjectTrustPath",
      "ProjectTrustLookup",
      "canonical != original",
      "keys.push(canonical)",
      "keys.push(original)",
      "get_active_project_for_lookup",
    ],
  },
  {
    id: "features.compaction-image-budget",
    kind: "feature_gate",
    file: "codex-rs/features/src/lib.rs",
    file_contains_all: [
      "CompactionImageBudget",
      "compaction_image_budget",
      "key: \"compaction_image_budget\"",
      "default_enabled: true",
    ],
  },
  {
    id: "features.instant-interrupt-opt-in",
    kind: "feature_gate",
    file: "codex-rs/features/src/lib.rs",
    file_contains_all: [
      "Feature::InstantInterrupt",
      "key: \"instant_interrupt\"",
      "Stage::UnderDevelopment",
      "default_enabled: false",
    ],
  },
  {
    id: "exec.thread-source",
    kind: "cli_contract",
    file: "codex-rs/exec/src/cli.rs",
    file_contains_all: [
      "ThreadSource",
      "long = \"thread-source\"",
      "value_name = \"SOURCE\"",
      "global = true",
      "pub thread_source: Option<ThreadSource>",
    ],
  },
  {
    id: "exec.thread-source-propagation",
    kind: "thread_lifecycle",
    file: "codex-rs/exec/src/lib.rs",
    file_contains_all: [
      "ThreadSource::User",
      "ThreadForkParams",
      "thread_start_params_from_config",
      "thread_source: Some(thread_source.clone())",
    ],
  },
  {
    id: "thread-source.forward-compatible-values",
    kind: "serialization_contract",
    file: "codex-rs/protocol/src/protocol.rs",
    file_contains_all: [
      "pub enum ThreadSource",
      "Feature(String)",
      "other => Ok(ThreadSource::Feature(other.to_string()))",
    ],
  },
  {
    id: "app-server.thread-source",
    kind: "jsonrpc_schema",
    file: "codex-rs/app-server-protocol/src/protocol/v2/thread.rs",
    file_contains_all: [
      "ThreadStartParams",
      "ThreadForkParams",
      "pub thread_source: Option<ThreadSource>",
    ],
  },
  {
    id: "app-server.thread-item-lifecycle-timestamps",
    kind: "jsonrpc_schema",
    file: "codex-rs/app-server-protocol/src/protocol/v2/thread.rs",
    file_contains_all: [
      "ThreadItemEntry",
      "pub started_at_ms: Option<i64>",
      "pub completed_at_ms: Option<i64>",
    ],
  },
  {
    id: "app-server.thread-items-anchor-pagination",
    kind: "jsonrpc_schema",
    file: "codex-rs/app-server-protocol/src/protocol/v2/thread.rs",
    file_contains_all: [
      "ThreadItemsListParams",
      "pub cursor: Option<ThreadItemsListCursor>",
      "ThreadItemsListCursor",
      "Anchor(ThreadItemsListAnchor)",
      "ThreadItemsListAnchor",
      "item_id: String",
    ],
  },
  {
    id: "app-server.additive-extensions",
    kind: "jsonrpc_docs",
    file: "codex-rs/app-server/README.md",
    file_contains_all: [
      "userVerification/cancel",
      "experimentalApi",
      "thread/attachment/add",
      "thread/attachment/list",
      "thread/attachment/remove",
      "thread/attachment/updated",
    ],
  },
  {
    id: "app-server.jsonrpc-envelope",
    kind: "jsonrpc_schema",
    file: "codex-rs/app-server-protocol/src/rpc.rs",
    file_contains_all: ["JSONRPCMessage", "JSONRPCRequest", "JSONRPCResponse", "JSONRPCNotification", "JSONRPCError", "jsonrpc"],
  },
  {
    id: "app-server.lifecycle-methods",
    kind: "jsonrpc_methods",
    file: "codex-rs/app-server-protocol/src/protocol/common.rs",
    file_contains_all: [
      "ThreadStart => \"thread/start\"",
      "ThreadResume => \"thread/resume\"",
      "ThreadFork => \"thread/fork\"",
      "ThreadQueueAdd => \"thread/queue/add\"",
      "TurnStart => \"turn/start\"",
      "ThreadStarted => \"thread/started\"",
      "ThreadQueueChanged => \"thread/queue/changed\"",
      "TurnStarted => \"turn/started\"",
    ],
  },
  {
    id: "app-server.workspace-routing-https-bootstrap",
    kind: "security_boundary",
    file: "codex-rs/app-server/src/request_processors/account_processor/workspace_routing.rs",
    file_contains_all: [
      "workspace backend must use an HTTPS origin without credentials",
      "effective_chatgpt_base_url",
      "config.chatgpt_base_url.clone()",
      "get_accounts_check",
      "workspace_backend_origin",
      "resolve_routing",
      "parse_backend_url",
      "url.scheme() != \"https\"",
      "!url.username().is_empty()",
      "url.password().is_some()",
    ],
  },
  {
    id: "app-server.mcp-event-stream",
    kind: "experimental_jsonrpc_methods",
    file: "codex-rs/app-server/src/request_processors/mcp_event_stream.rs",
    file_contains_all: [
      "McpEventStreams",
      "McpServerEventStreamStartParams",
      "MAX_MCP_EVENT_STREAMS_PER_CONNECTION",
    ],
  },
  {
    id: "app-server.permission-profile-removal",
    kind: "jsonrpc_validation",
    file: "codex-rs/app-server/src/message_processor.rs",
    file_contains_all: [
      "reject_removed_permission_profile",
      "thread/start\" | \"thread/resume\" | \"thread/fork\" | \"turn/start",
      "permissionProfile",
      "use `permissions` with a named profile id instead",
    ],
  },
  {
    id: "app-server.initialize-handshake",
    kind: "jsonrpc_handshake",
    file: "codex-rs/app-server/src/request_processors/initialize_processor.rs",
    file_contains_all: ["InitializeRequestProcessor", "Already initialized", "send_initialize_notifications_to_connection"],
  },
  {
    id: "app-server.thread-lifecycle-processors",
    kind: "thread_lifecycle",
    file: "codex-rs/app-server/src/request_processors/thread_processor.rs",
    file_contains_all: ["ThreadStartParams", "ThreadResumeParams", "ThreadForkParams", "ConnectionRequestId"],
  },
  {
    id: "app-server.turn-start-processor",
    kind: "turn_lifecycle",
    file: "codex-rs/app-server/src/request_processors/turn_processor.rs",
    file_contains_all: ["TurnStartParams", "TurnStartResponse", "turn/start", "TurnStatus::InProgress"],
  },
  {
    id: "app-server.thread-serialization-scope",
    kind: "serialization_scope",
    file: "codex-rs/app-server/src/request_serialization.rs",
    file_contains_all: [
      "ClientRequestSerializationScope",
      "RequestSerializationQueueKey",
      "Thread",
      "ThreadPath",
      "RequestSerializationAccess",
    ],
  },
  {
    id: "process.background-command-no-console",
    kind: "process_launch",
    file: "codex-rs/utils/process/src/lib.rs",
    file_contains_all: [
      "pub fn background_command",
      "CREATE_NO_WINDOW",
      "command.creation_flags",
      "Command::new(program)",
    ],
  },
  {
    id: "core.redirect-shell-tool-background-command",
    kind: "process_launch",
    file: "codex-rs/core/src/spawn.rs",
    file_contains_all: [
      "StdioPolicy::RedirectForShellTool",
      "codex_utils_process::background_command",
      "StdioPolicy::Inherit",
      "Command::new(&program)",
    ],
  },
  {
    id: "tui.security-setup-auth-boundary",
    kind: "authenticated_auxiliary_request",
    file: "codex-rs/tui/src/security_setup.rs",
    file_contains_all: [
      "config.model_provider_id != \"openai\" || server.uses_remote_workspace()",
      "ClientRequest::GetAuthStatus",
      "!matches!(auth, CodexAuth::Chatgpt(_)) || auth.is_fedramp_account()",
      "status.auth_method != Some(AuthMode::Chatgpt)",
      "status.auth_token.as_deref() != Some(saved_token.as_str())",
      "RouteAwareClientPool::new_without_redirects",
      "{}/wham/security-setup",
      "config.chatgpt_base_url.trim_end_matches('/')",
      "auth_provider_from_auth(&auth).to_auth_headers()",
      "url.host_str() == Some(\"chatgpt.com\")",
      "Duration::from_secs(3)",
    ],
  },
  {
    id: "models.explicit-provider-catalog-authoritative",
    kind: "provider_catalog",
    file: "codex-rs/models-manager/src/manager.rs",
    file_contains_all: [
      "Only explicit catalogs serialize refresh and suppress bundled fallback.",
      "Self::ExplicitProvider(_) => None",
      "pub fn with_provider_catalog(mut self) -> Self",
      "self.remote_models.get_mut().models.clear();",
      "matches!(&self.catalog_source, CatalogSource::ExplicitProvider(_))",
      "current.models.clear();",
      "if !remote_only && let Some(mut models) = self.catalog_source.fallback_models()",
    ],
  },
  {
    id: "tui.provider-selection-history-defaults",
    kind: "provider_selection",
    file: "codex-rs/tui/src/app_server_session/provider_selection.rs",
    file_contains_all: [
      "Provider request overrides honor managed requirements over explicit invocation choices.",
      "required_model_provider()",
      ".or_else(|| explicit_provider(config))",
      "pub(crate) async fn history_model_provider",
      "read_effective_config_if_supported",
      ".model_provider",
      "unwrap_or_else(|| \"openai\".to_string())",
    ],
  },
  {
    id: "tui.projectless-workspace-defaults",
    kind: "permission_defaults",
    file: "codex-rs/tui/src/projectless.rs",
    file_contains_all: [
      "Select desktop-like execution defaults for positively discovered local projectless folders.",
      "config.config_layer_stack.is_projectless()",
      "config.active_project.trust_level.is_some()",
      "config.workspace_roots.len() != 1",
      "config.workspace_roots.first() != Some(&config.cwd)",
      "has_only_local_environments(environments)",
      "PermissionProfile::workspace_write()",
      "set_permission_profile_from_session_snapshot",
    ],
  },
  {
    id: "tui.resume-saved-permissions",
    kind: "permission_restore",
    file: "codex-rs/tui/src/resume_permissions.rs",
    file_contains_all: [
      "Omitted choices let app-server restore the destination task's saved settings.",
      "ConfigLayerSource::SessionFlags",
      "overrides.approval_policy.is_some() || has(\"approval_policy\")",
      "overrides.approvals_reviewer.is_some() || has(\"approvals_reviewer\")",
      "overrides.permission_profile.is_some()",
      "workspace_roots: overrides.cwd.is_some()",
      "has(\"sandbox_workspace_write.writable_roots\")",
    ],
  },
  {
    id: "core.subagent-pending-environment-inheritance",
    kind: "environment_inheritance",
    file: "codex-rs/core/src/session/environment.rs",
    file_contains_all: [
      "follow_inherited_environment_configurations",
      "starting.owner_configuration()",
      "ConfigUpdateSource::Inherited",
      "no one will retry this child's one-time update",
      "!inherited || matches!(environment.config, EnvironmentConfigState::Pending)",
      "if inherited && matches!(environments, (None, None))",
    ],
  },
  {
    id: "core.subagent-spawn-inherited-environments",
    kind: "environment_inheritance",
    file: "codex-rs/core/src/agent/control/spawn.rs",
    file_contains_all: [
      "inherited_environments_for_source",
      "let inherited_environments = self",
      "environment_selections: None",
      "inherited_environments,",
      "resume_thread_with_history_with_source",
    ],
  },
  {
    id: "rmcp.remote-stdio-env-allowlist",
    kind: "remote_mcp_environment",
    file: "codex-rs/rmcp-client/src/stdio_server_launcher.rs",
    file_contains_all: [
      "fn remote_env_policy(remote_env_vars: &[String])",
      "crate::utils::DEFAULT_ENV_VARS",
      "remote_env_vars.iter().cloned()",
      "include_only,",
      "env.get(\"REMOTE_TOKEN\")",
      "assert!(!env.contains_key(\"UNREQUESTED_SECRET\"))",
    ],
  },

  {
    id: "responses.server-retry-guidance",
    kind: "retry_lifecycle",
    file: "codex-rs/core/src/responses_retry.rs",
    file_contains_all: [
      "ResponsesStreamRetryState",
      "ResponsesStreamRequest::RemoteCompactionV2",
      "let retry_after = err.retry_after()",
      "retry_state.retries >= max_retries",
      "try_switch_fallback_transport",
      "tokio::time::sleep_until(retry_after.deadline()).await",
      "retry_state.retries < max_retries",
      "retry_after.map(RetryAfter::deadline).unwrap_or(now + delay)",
      "ExhaustedResponseRetry",
      "turn_id: turn_context.sub_id.clone()",
    ],
  },
  {
    id: "responses.routing-fields-before-input",
    kind: "serialization",
    file: "codex-rs/codex-api/src/common.rs",
    file_contains_all: [
      "pub struct ResponsesApiRequest",
      "pub struct ResponseCreateWsRequest",
      "pub model: String,\n    pub stream: bool,",
      "pub model: &'a str,\n    pub stream: bool,",
      "pub service_tier: Option<String>",
      "pub service_tier: Option<&'a str>",
      "pub previous_response_id: Option<String>",
    ],
  },
  {
    id: "websocket.handshake-retry-after",
    kind: "retry_metadata",
    file: "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
    file_contains_all: [
      "WsError::Http(response)",
      "let headers = response.headers().clone()",
      "let retry_after = RetryAfter::from_headers(&headers)",
      "headers: Some(headers)",
      "retry_after,",
    ],
  },
  {
    id: "responses.failed-retry-after",
    kind: "retry_metadata",
    file: "codex-rs/codex-api/src/sse/responses_error.rs",
    file_contains_all: [
      "parse_failed_response",
      "let retry_after_header = error",
      ".and_then(|error| error.get(\"headers\"))",
      "RetryAfter::from_headers(&json_headers_to_http_headers(headers))",
      "retry_after_header\n                .or_else",
      "Duration::try_from_secs_f64(value).ok()",
    ],
  },
  {
    id: "websocket.error-retry-after",
    kind: "retry_metadata",
    file: "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
    file_contains_all: [
      "WrappedWebsocketError",
      "headers: Option<Value>,",
      "error.headers.as_ref()",
      "headers.as_ref(),",
      "find_map(|headers| RetryAfter::from_headers(&json_headers_to_http_headers(headers)))",
    ],
  },
  {
    id: "model-provider.custom-capabilities",
    kind: "provider_capabilities",
    file: "codex-rs/model-provider/src/capabilities.rs",
    file_contains_all: [
      "pub struct ProviderCapabilities",
      "pub external_web_access: bool",
      "pub remote_compaction: RemoteCompactionSupport",
      "is_azure_responses_provider",
      "RemoteCompactionSupport::V2",
      "RemoteCompactionSupport::Unsupported",
      "overrides\n                .external_web_access",
      "overrides\n                .remote_compaction",
    ],
  },
  {
    id: "tools.namespace-gate-removal",
    kind: "tool_capability",
    file: "codex-rs/core/src/tools/spec_plan.rs",
    file_contains_all: [
      "merge_into_namespaces(specs)",
      "model_info.supports_search_tool",
      "tool_exposure_with_namespace_override",
      "code_mode_only_strict_3p_tools",
      "turn_context.config.multi_agent_v2.tool_namespace.as_deref()",
      "turn_context.provider.capabilities().image_generation",
    ],
  },
  {
    id: "context.base-instructions-identity",
    kind: "context_boundary",
    file: "codex-rs/core/src/context/base_instructions.rs",
    file_contains_all: [
      "pub(crate) const KIND: &str = \"model.base_instructions\";",
      "pub(crate) fn matches_item(item: &ResponseItem) -> bool",
      "internal_chat_message_metadata_passthrough: Some(metadata)",
      "kind.as_str() == Self::KIND",
    ],
  },
  {
    id: "context.incremental-tool-prefix",
    kind: "request_context",
    file: "codex-rs/core/src/client.rs",
    file_contains_all: [
      "let mut prefix = Vec::new();",
      "create_tools_json_for_responses_lite(&prompt.tools)?",
      "create_tools_raw_json_for_responses_api(&prompt.tools)?.into()",
      "BaseInstructionsFragment(",
      "input.splice(0..0, prefix);",
    ],
  },
  {
    id: "responses.partial-answer-phase",
    kind: "stream_phase",
    file: "codex-rs/codex-api/src/sse/responses.rs",
    file_contains_all: [
      "response.output_item.added",
      "phase",
      "MessagePhase::PartialAnswer",
      "partial_answer",
    ],
    expected_stream_events_all: ["response.output_item.added", "response.completed"],
  },
  {
    id: "rollout.partial-answer-channel",
    kind: "stream_phase",
    file: "codex-rs/rollout-trace/src/reducer/conversation/normalize.rs",
    file_contains_all: [
      "fn channel_from_phase(phase: &str)",
      "\"partial_answer\" | \"final_answer\"",
      "ConversationChannel::Final",
    ],
  },
  {
    id: "rmcp.remote-windows-env",
    kind: "remote_mcp_environment",
    file: "codex-rs/rmcp-client/src/stdio_server_launcher.rs",
    file_contains_all: [
      "crate::utils::DEFAULT_ENV_VARS",
      ".chain([\"SYSTEMROOT\", \"TEMP\", \"TMP\"].iter())",
      "include_only,",
      "env.get(\"SystemRoot\")",
      "env.get(name)",
      "assert!(!env.contains_key(\"UNREQUESTED_SECRET\"))",
    ],
  },
  {
    id: "app-server.turn-lineage",
    kind: "app_server_lineage",
    file: "codex-rs/app-server/src/request_processors/turn_processor.rs",
    file_contains_all: [
      "TurnEnvironmentRequests",
      "resolve_turn_environment_requests",
      "parent_turn_id: params.parent_turn_id",
      "root_turn_id: params.root_turn_id",
      "root_turn_id: Some(root_turn_id)",
    ],
  },
  {
    id: "app-server.turn-lineage-wire",
    kind: "app_server_lineage",
    file: "codex-rs/app-server-protocol/src/protocol/v2/turn.rs",
    file_contains_all: [
      "pub parent_turn_id: Option<String>",
      "pub root_turn_id: Option<String>",
      "pub environments: Option<Vec<TurnEnvironmentParams>>",
    ],
  },
  {
    id: "app-server.environment-skills",
    kind: "app_server_environment",
    file: "codex-rs/app-server/src/request_processors/environment_processor.rs",
    file_contains_all: [
      "ScopedSkillsConfig",
      ".skills",
      "upsert_environment_with_options(params.environment_id, options, skills)",
    ],
  },
  {
    id: "app-server.environment-skills-wire",
    kind: "app_server_environment",
    file: "codex-rs/app-server-protocol/src/protocol/v2/environment.rs",
    file_contains_all: [
      "pub skills: Option<EnvironmentSkillsParams>",
      "pub struct EnvironmentSkillsParams",
      "pub required: Option<Vec<String>>",
      "checked before model inference",
    ],
  },
  {
    id: "turn.input-attribution",
    kind: "turn_lineage",
    file: "codex-rs/protocol/src/turn_input.rs",
    file_contains_all: [
      "pub struct TurnAttribution",
      "pub initiating_agent_path: Option<AgentPath>",
      "pub root_turn_id: Option<String>",
      "pub fn start_options(&self) -> TurnStartOptions",
      "Started {",
      "Steered {",
    ],
  },
  {
    "id": "exec.cyber-access-program",
    "kind": "cli_contract",
    "file": "codex-rs/exec/src/cli.rs",
    "file_contains_all": [
      "#[arg(long, value_enum, value_name = \"PROGRAM\", global = true)]",
      "pub cyber_access_program: Option<CyberAccessProgramCliArg>",
      "pub enum CyberAccessProgramCliArg",
      "Standard,",
      "DaybreakBlue,",
      "DaybreakRed,",
      "#[value(rename_all = \"snake_case\")]"
    ]
  },
  {
    "id": "exec.daybreak-explicit-opt-in",
    "kind": "feature_gate",
    "file": "codex-rs/exec/src/lib.rs",
    "file_contains_all": [
      "let cyber_access_program = match cyber_access_program {",
      "Some(program) => Some(program),",
      "None if config.features.enabled(Feature::CliDaybreak) => {",
      "daybreak::program_for_turn(",
      "None => None,",
      "daybreak_override",
      "ClientRequest::TurnStart"
    ]
  },
  {
    "id": "features.cli-daybreak-opt-in",
    "kind": "feature_gate",
    "file": "codex-rs/features/src/lib.rs",
    "file_contains_all": [
      "id: Feature::CliDaybreak,\n        key: \"cli_daybreak\",\n        stage: Stage::UnderDevelopment,\n        default_enabled: false,"
    ]
  },
  {
    "id": "cyber-access-program.provider-eligibility",
    "kind": "provider_boundary",
    "file": "codex-rs/core/src/cyber_access_program.rs",
    "file_contains_all": [
      "pub(crate) fn for_provider(",
      "program.filter(|_| provider_id == OPENAI_PROVIDER_ID)",
      "if config.model_provider_id != OPENAI_PROVIDER_ID {",
      "Feature::ApiKeyCyberAccessPrograms",
      "if auth.is_chatgpt_auth()",
      "if !auth.is_api_key_auth()"
    ]
  },
];

const SEMANTIC_LIST_FIELDS = [
  "file_contains_all",
  "expected_headers_all",
  "proxy_replaced_headers_all",
  "proxy_skipped_headers_all",
  "expected_routes_all",
  "expected_stream_events_all",
];

function parseArgs(argv) {
  const args = {
    baseline: DEFAULT_BASELINE_PATH,
    report: null,
    source: null,
    json: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--baseline") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--baseline requires a value");
      }
      args.baseline = argv[index];
      continue;
    }
    if (value === "--report") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--report requires a value");
      }
      args.report = argv[index];
      continue;
    }
    if (value === "--source") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--source requires a directory");
      }
      args.source = argv[index];
      continue;
    }
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--self-test") {
      args.selfTest = true;
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

function stringArray(value) {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item) => typeof item === "string");
}

function missingValues(required, actual) {
  const actualSet = new Set(actual);
  return required.filter((item) => !actualSet.has(item));
}

function duplicateValues(values) {
  const seen = new Set();
  const duplicates = new Set();
  for (const value of values) {
    if (seen.has(value)) {
      duplicates.add(value);
    }
    seen.add(value);
  }
  return [...duplicates];
}

function criticalFileMap(compat) {
  const files = Array.isArray(compat?.critical_files) ? compat.critical_files : [];
  const mapped = new Map();
  for (const file of files) {
    if (file && typeof file.path === "string") {
      mapped.set(file.path, file);
    }
  }
  return mapped;
}

function semanticCheckMap(compat) {
  const checks = Array.isArray(compat?.semantic_checks) ? compat.semantic_checks : [];
  const mapped = new Map();
  for (const check of checks) {
    if (check && typeof check.id === "string") {
      mapped.set(check.id, check);
    }
  }
  return mapped;
}

function validateSemanticListField({ check, field, label, allowedValues, errors, warnings }) {
  if (!(field in check)) {
    return [];
  }
  if (!Array.isArray(check[field])) {
    errors.push(`codex.compatibility.semantic_checks.${check.id}.${field} must be an array`);
    return [];
  }

  const values = check[field];
  for (const [index, value] of values.entries()) {
    if (typeof value !== "string") {
      errors.push(`codex.compatibility.semantic_checks.${check.id}.${field}[${index}] must be a string`);
    }
  }
  for (const duplicate of duplicateValues(stringArray(values))) {
    warnings.push(`codex.compatibility.semantic_checks.${check.id}.${field} contains duplicate ${JSON.stringify(duplicate)}`);
  }

  if (allowedValues) {
    for (const value of missingValues(stringArray(values), allowedValues)) {
      errors.push(`codex.compatibility.semantic_checks.${check.id}.${field} references ${label} missing ${JSON.stringify(value)}`);
    }
  }

  return stringArray(values);
}

function validateRequiredSemanticCheck({ required, check, errors }) {
  if (check.file !== required.file) {
    errors.push(`codex.compatibility.semantic_checks.${required.id}.file must be ${required.file}`);
  }
  if (check.kind !== required.kind) {
    errors.push(`codex.compatibility.semantic_checks.${required.id}.kind must be ${required.kind}`);
  }
  for (const field of SEMANTIC_LIST_FIELDS) {
    const requiredValues = stringArray(required[field]);
    if (requiredValues.length === 0) {
      continue;
    }
    const actualValues = stringArray(check[field]);
    for (const value of missingValues(requiredValues, actualValues)) {
      errors.push(`codex.compatibility.semantic_checks.${required.id}.${field} missing ${JSON.stringify(value)}`);
    }
  }
}

function validateSemanticChecks({ compat, files, errors, warnings }) {
  const formatVersion = compat.format_version;
  let semanticChecksRequired = false;
  if (formatVersion !== undefined) {
    if (!Number.isInteger(formatVersion)) {
      errors.push("codex.compatibility.format_version must be an integer when set");
    } else {
      semanticChecksRequired = formatVersion >= COMPAT_FORMAT_VERSION_WITH_SEMANTIC_CHECKS;
    }
  }

  if (!Array.isArray(compat.semantic_checks)) {
    const message = `codex.compatibility.semantic_checks must be an array for format_version ${COMPAT_FORMAT_VERSION_WITH_SEMANTIC_CHECKS}`;
    if (semanticChecksRequired) {
      errors.push(message);
    } else {
      warnings.push("codex.compatibility.semantic_checks should be an array for grouped compatibility guards");
    }
    return;
  }

  const checks = semanticCheckMap(compat);
  const duplicatedIds = duplicateValues(
    compat.semantic_checks
      .filter((check) => check && typeof check.id === "string")
      .map((check) => check.id),
  );
  for (const duplicate of duplicatedIds) {
    warnings.push(`codex.compatibility.semantic_checks contains duplicate id ${JSON.stringify(duplicate)}`);
  }

  if (semanticChecksRequired) {
    for (const required of REQUIRED_SEMANTIC_CHECKS) {
      const check = checks.get(required.id);
      if (!check) {
        errors.push(`codex.compatibility.semantic_checks missing ${required.id}`);
        continue;
      }
      validateRequiredSemanticCheck({ required, check, errors });
    }
  }

  const expectedHeaders = stringArray(compat.expected_headers);
  const proxyReplacedHeaders = stringArray(compat.proxy_replaced_headers);
  const proxySkippedHeaders = stringArray(compat.proxy_skipped_headers);
  const expectedRoutes = stringArray(compat.expected_routes);
  const expectedStreamEvents = stringArray(compat.expected_stream_events);

  for (const [index, check] of compat.semantic_checks.entries()) {
    if (!check || typeof check !== "object" || Array.isArray(check)) {
      errors.push(`codex.compatibility.semantic_checks[${index}] must be an object`);
      continue;
    }
    if (typeof check.id !== "string" || check.id.length === 0) {
      errors.push(`codex.compatibility.semantic_checks[${index}].id must be a non-empty string`);
      continue;
    }
    if (typeof check.kind !== "string" || check.kind.length === 0) {
      warnings.push(`codex.compatibility.semantic_checks.${check.id}.kind should describe the grouped assumption`);
    }
    if (typeof check.file !== "string" || check.file.length === 0) {
      errors.push(`codex.compatibility.semantic_checks.${check.id}.file must be a non-empty string`);
      continue;
    }
    if (typeof check.reason !== "string" || check.reason.length === 0) {
      warnings.push(`codex.compatibility.semantic_checks.${check.id}.reason should explain why the grouped assumption matters`);
    }

    const file = files.get(check.file);
    if (!file) {
      errors.push(`codex.compatibility.semantic_checks.${check.id}.file is not listed in critical_files`);
      continue;
    }
    const fileContains = stringArray(file.required_contains);
    let checkedFieldCount = 0;

    checkedFieldCount += validateSemanticListField({
      check,
      field: "file_contains_all",
      label: `${check.file}.required_contains`,
      allowedValues: fileContains,
      errors,
      warnings,
    }).length;
    checkedFieldCount += validateSemanticListField({
      check,
      field: "expected_headers_all",
      label: "codex.compatibility.expected_headers",
      allowedValues: expectedHeaders,
      errors,
      warnings,
    }).length;
    checkedFieldCount += validateSemanticListField({
      check,
      field: "proxy_replaced_headers_all",
      label: "codex.compatibility.proxy_replaced_headers",
      allowedValues: proxyReplacedHeaders,
      errors,
      warnings,
    }).length;
    checkedFieldCount += validateSemanticListField({
      check,
      field: "proxy_skipped_headers_all",
      label: "codex.compatibility.proxy_skipped_headers",
      allowedValues: proxySkippedHeaders,
      errors,
      warnings,
    }).length;
    checkedFieldCount += validateSemanticListField({
      check,
      field: "expected_routes_all",
      label: "codex.compatibility.expected_routes",
      allowedValues: expectedRoutes,
      errors,
      warnings,
    }).length;
    checkedFieldCount += validateSemanticListField({
      check,
      field: "expected_stream_events_all",
      label: "codex.compatibility.expected_stream_events",
      allowedValues: expectedStreamEvents,
      errors,
      warnings,
    }).length;

    if (checkedFieldCount === 0) {
      warnings.push(`codex.compatibility.semantic_checks.${check.id} should include at least one grouped expectation`);
    }
  }
}

function validateBaseline(baseline) {
  const errors = [];
  const warnings = [];
  const compat = baseline?.codex?.compatibility;

  if (!compat || typeof compat !== "object") {
    errors.push("codex.compatibility is missing");
    return { errors, warnings };
  }

  const files = criticalFileMap(compat);
  const missingFiles = missingValues(REQUIRED_CRITICAL_FILES, [...files.keys()]);
  for (const filePath of missingFiles) {
    errors.push(`codex.compatibility.critical_files missing ${filePath}`);
  }

  for (const filePath of REQUIRED_CRITICAL_FILES) {
    const file = files.get(filePath);
    if (!file) {
      continue;
    }
    const requiredContains = stringArray(file.required_contains);
    if (!Array.isArray(file.required_contains)) {
      errors.push(`${filePath}.required_contains must be an array`);
      continue;
    }
    const missingContains = missingValues(REQUIRED_FILE_CONTAINS[filePath], requiredContains);
    for (const token of missingContains) {
      errors.push(`${filePath}.required_contains missing ${JSON.stringify(token)}`);
    }
    for (const token of duplicateValues(requiredContains)) {
      warnings.push(`${filePath}.required_contains contains duplicate ${JSON.stringify(token)}`);
    }
  }

  if (!Array.isArray(compat.expected_headers)) {
    errors.push("codex.compatibility.expected_headers must be an array");
  } else {
    for (const header of missingValues(REQUIRED_EXPECTED_HEADERS, stringArray(compat.expected_headers))) {
      errors.push(`codex.compatibility.expected_headers missing ${header}`);
    }
  }

  if (!Array.isArray(compat.proxy_replaced_headers)) {
    errors.push("codex.compatibility.proxy_replaced_headers must be an array");
  } else {
    for (const header of missingValues(
      REQUIRED_PROXY_REPLACED_HEADERS,
      stringArray(compat.proxy_replaced_headers),
    )) {
      errors.push(`codex.compatibility.proxy_replaced_headers missing ${header}`);
    }
  }

  if (!Array.isArray(compat.proxy_skipped_headers)) {
    errors.push("codex.compatibility.proxy_skipped_headers must be an array");
  } else {
    for (const header of missingValues(REQUIRED_PROXY_SKIPPED_HEADERS, stringArray(compat.proxy_skipped_headers))) {
      errors.push(`codex.compatibility.proxy_skipped_headers missing ${header}`);
    }
  }

  if (!Array.isArray(compat.expected_routes)) {
    errors.push("codex.compatibility.expected_routes must be an array");
  } else {
    for (const route of missingValues(REQUIRED_EXPECTED_ROUTES, stringArray(compat.expected_routes))) {
      errors.push(`codex.compatibility.expected_routes missing ${route}`);
    }
  }

  if (!Array.isArray(compat.expected_stream_events)) {
    warnings.push("codex.compatibility.expected_stream_events should be an array");
  } else {
    for (const event of missingValues(REQUIRED_STREAM_EVENTS, stringArray(compat.expected_stream_events))) {
      errors.push(`codex.compatibility.expected_stream_events missing ${event}`);
    }
  }

  if (typeof compat.upstream_repository !== "string" || compat.upstream_repository.length === 0) {
    warnings.push("codex.compatibility.upstream_repository should identify the upstream repository");
  }

  if (typeof compat.guard_command !== "string" || compat.guard_command.length === 0) {
    warnings.push("codex.compatibility.guard_command should document the offline guard command");
  }

  const testedCodexRelease = compat.tested_codex_release;
  const latestCodexRelease = baseline?.codex?.latestRelease?.tag_name;
  if (
    typeof testedCodexRelease !== "string" ||
    !/^rust-v\d+\.\d+\.\d+$/u.test(testedCodexRelease)
  ) {
    errors.push("codex.compatibility.tested_codex_release must be a stable rust-vX.Y.Z release tag");
  }
  if (
    typeof latestCodexRelease !== "string" ||
    !/^rust-v\d+\.\d+\.\d+$/u.test(latestCodexRelease)
  ) {
    errors.push("codex.latestRelease.tag_name must be a stable rust-vX.Y.Z release tag");
  }
  if (
    typeof testedCodexRelease === "string" &&
    typeof latestCodexRelease === "string" &&
    testedCodexRelease !== latestCodexRelease
  ) {
    errors.push(
      "codex.compatibility.tested_codex_release must match codex.latestRelease.tag_name",
    );
  }

  const appServerProtocol = compat.app_server_protocol;
  if (!appServerProtocol || typeof appServerProtocol !== "object" || Array.isArray(appServerProtocol)) {
    errors.push("codex.compatibility.app_server_protocol is missing");
  } else {
    if (
      typeof appServerProtocol.schema_command !== "string" ||
      !appServerProtocol.schema_command.includes("generate-json-schema")
    ) {
      errors.push("codex.compatibility.app_server_protocol.schema_command must document generate-json-schema");
    }
    if (appServerProtocol.schema_hash !== null && typeof appServerProtocol.schema_hash !== "string") {
      errors.push("codex.compatibility.app_server_protocol.schema_hash must be a string or null");
    }
    if (!Array.isArray(appServerProtocol.required_methods)) {
      errors.push("codex.compatibility.app_server_protocol.required_methods must be an array");
    } else {
      for (const method of missingValues(
        REQUIRED_APP_SERVER_METHODS,
        stringArray(appServerProtocol.required_methods),
      )) {
        errors.push(`codex.compatibility.app_server_protocol.required_methods missing ${method}`);
      }
    }
  }

  validateSemanticChecks({ compat, files, errors, warnings });

  return { errors, warnings };
}

async function validateSourceMarkers({ compat, sourceRoot }) {
  const errors = [];
  const contents = new Map();
  const files = Array.isArray(compat?.critical_files) ? compat.critical_files : [];

  for (const file of files) {
    if (!file || typeof file.path !== "string") {
      continue;
    }
    const sourcePath = path.resolve(sourceRoot, file.path);
    try {
      contents.set(file.path, await fs.readFile(sourcePath, "utf8"));
    } catch (error) {
      errors.push(
        `upstream source missing critical file ${file.path}: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }

  for (const file of files) {
    const text = contents.get(file?.path);
    if (text === undefined) {
      continue;
    }
    for (const marker of stringArray(file.required_contains)) {
      if (!text.includes(marker)) {
        errors.push(`upstream source ${file.path} missing critical marker ${JSON.stringify(marker)}`);
      }
    }
  }

  for (const check of Array.isArray(compat?.semantic_checks) ? compat.semantic_checks : []) {
    const text = contents.get(check?.file);
    if (text === undefined) {
      continue;
    }
    for (const marker of stringArray(check.file_contains_all)) {
      if (!text.includes(marker)) {
        errors.push(
          `upstream source ${check.file} missing semantic marker ${check.id}.${JSON.stringify(marker)}`,
        );
      }
    }
  }

  return errors;
}

function renderReport(report) {
  const lines = [];
  lines.push("Upstream Codex baseline guard");
  lines.push(`Baseline: ${report.baselinePath}`);
  lines.push(`Generated at: ${report.generated_at}`);
  lines.push(`Status: ${report.ok ? "ok" : "failed"}`);
  lines.push("");

  if (report.errors.length > 0) {
    lines.push("Errors:");
    for (const error of report.errors) {
      lines.push(`- ${error}`);
    }
    lines.push("");
  }

  if (report.warnings.length > 0) {
    lines.push("Warnings:");
    for (const warning of report.warnings) {
      lines.push(`- ${warning}`);
    }
    lines.push("");
  }

  if (report.errors.length === 0 && report.warnings.length === 0) {
    lines.push("Baseline contains all required Codex runtime compatibility assumptions.");
  }

  return `${lines.join("\n").trimEnd()}\n`;
}

function buildSelfTestBaseline() {
  return {
    codex: {
      latestRelease: { tag_name: "rust-v9.8.7" },
      compatibility: {
        upstream_repository: "self-test",
        guard_command: "node scripts/compat/check-upstream-baseline.mjs --self-test",
        format_version: COMPAT_FORMAT_VERSION_WITH_SEMANTIC_CHECKS,
        tested_codex_release: "rust-v9.8.7",
        app_server_protocol: {
          schema_command: "codex app-server generate-json-schema --out DIR",
          schema_hash: null,
          required_methods: REQUIRED_APP_SERVER_METHODS,
        },
        critical_files: REQUIRED_CRITICAL_FILES.map((filePath) => ({
          path: filePath,
          reason: "self-test critical file",
          required_contains: REQUIRED_FILE_CONTAINS[filePath],
        })),
        expected_headers: REQUIRED_EXPECTED_HEADERS,
        proxy_replaced_headers: REQUIRED_PROXY_REPLACED_HEADERS,
        proxy_skipped_headers: REQUIRED_PROXY_SKIPPED_HEADERS,
        expected_routes: REQUIRED_EXPECTED_ROUTES,
        expected_stream_events: REQUIRED_STREAM_EVENTS,
        semantic_checks: REQUIRED_SEMANTIC_CHECKS.map((check) => ({
          reason: "self-test semantic group",
          ...check,
        })),
      },
    },
  };
}

function assertSelfTestError({ name, mutate, expectedMessage }) {
  const baseline = buildSelfTestBaseline();
  mutate(baseline.codex.compatibility, baseline);
  const { errors } = validateBaseline(baseline);
  if (!errors.includes(expectedMessage)) {
    throw new Error(
      [
        `self-test ${name} failed`,
        `expected error: ${expectedMessage}`,
        `actual errors: ${errors.length === 0 ? "(none)" : errors.join("; ")}`,
      ].join("\n"),
    );
  }
}

function semanticCheck(compat, id) {
  const check = compat.semantic_checks.find((candidate) => candidate.id === id);
  if (!check) {
    throw new Error(`self-test fixture is missing semantic check ${id}`);
  }
  return check;
}

function runSelfTest() {
  const valid = validateBaseline(buildSelfTestBaseline());
  if (valid.errors.length > 0) {
    throw new Error(`self-test valid baseline failed: ${valid.errors.join("; ")}`);
  }

  assertSelfTestError({
    name: "mismatched bundled release",
    mutate: (compat) => {
      compat.tested_codex_release = "rust-v9.8.6";
    },
    expectedMessage:
      "codex.compatibility.tested_codex_release must match codex.latestRelease.tag_name",
  });

  assertSelfTestError({
    name: "invalid latest release",
    mutate: (_compat, baseline) => {
      baseline.codex.latestRelease.tag_name = "not-a-release";
    },
    expectedMessage: "codex.latestRelease.tag_name must be a stable rust-vX.Y.Z release tag",
  });

  assertSelfTestError({
    name: "missing semantic group",
    mutate: (compat) => {
      compat.semantic_checks = compat.semantic_checks.filter((check) => check.id !== "proxy.preserved-headers");
    },
    expectedMessage: "codex.compatibility.semantic_checks missing proxy.preserved-headers",
  });

  assertSelfTestError({
    name: "missing semantic header token",
    mutate: (compat) => {
      const check = semanticCheck(compat, "proxy.preserved-headers");
      check.expected_headers_all = check.expected_headers_all.filter((header) => header !== "session_id");
    },
    expectedMessage: 'codex.compatibility.semantic_checks.proxy.preserved-headers.expected_headers_all missing "session_id"',
  });

  assertSelfTestError({
    name: "missing semantic file token",
    mutate: (compat) => {
      const check = semanticCheck(compat, "sse.responses-http-route-behavior");
      check.file_contains_all = check.file_contains_all.filter((token) => token !== "process_sse");
    },
    expectedMessage: 'codex.compatibility.semantic_checks.sse.responses-http-route-behavior.file_contains_all missing "process_sse"',
  });

  assertSelfTestError({
    name: "missing realtime v1 websocket alpha header token",
    mutate: (compat) => {
      const check = semanticCheck(compat, "realtime.websocket-v1-alpha-header");
      check.file_contains_all = check.file_contains_all.filter((token) => token !== "quicksilver=v1");
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.realtime.websocket-v1-alpha-header.file_contains_all missing "quicksilver=v1"',
  });

  assertSelfTestError({
    name: "missing Bedrock GPT-6.1 catalog token",
    mutate: (compat) => {
      const check = semanticCheck(compat, "model-provider.bedrock-gpt-6-catalog");
      check.file_contains_all = check.file_contains_all.filter(
        (token) => token !== "AMAZON_BEDROCK_GPT_6_1_SOL_MODEL_ID",
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.model-provider.bedrock-gpt-6-catalog.file_contains_all missing "AMAZON_BEDROCK_GPT_6_1_SOL_MODEL_ID"',
  });

  assertSelfTestError({
    name: "missing server-directed transport fallback deadline",
    mutate: (compat) => {
      const check = semanticCheck(compat, "responses.server-retry-guidance");
      check.file_contains_all = check.file_contains_all.filter(
        (value) => value !== "tokio::time::sleep_until(retry_after.deadline()).await",
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.responses.server-retry-guidance.file_contains_all missing "tokio::time::sleep_until(retry_after.deadline()).await"',
  });

  assertSelfTestError({
    name: "missing explicit per-turn program before Daybreak fallback",
    mutate: (compat) => {
      const check = semanticCheck(compat, "exec.daybreak-explicit-opt-in");
      check.file_contains_all = check.file_contains_all.filter(
        (value) => value !== "Some(program) => Some(program),",
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.exec.daybreak-explicit-opt-in.file_contains_all missing "Some(program) => Some(program),"',
  });

  assertSelfTestError({
    name: "missing failed Responses retry header handling",
    mutate: (compat) => {
      const check = semanticCheck(compat, "responses.failed-retry-after");
      check.file_contains_all = check.file_contains_all.filter(
        (value) => !value.includes("json_headers_to_http_headers(headers)"),
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.responses.failed-retry-after.file_contains_all missing "RetryAfter::from_headers(&json_headers_to_http_headers(headers))"',
  });

  assertSelfTestError({
    name: "missing WebSocket nested retry header handling",
    mutate: (compat) => {
      const check = semanticCheck(compat, "websocket.error-retry-after");
      check.file_contains_all = check.file_contains_all.filter(
        (value) => value !== "error.headers.as_ref()",
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.websocket.error-retry-after.file_contains_all missing "error.headers.as_ref()"',
  });

  assertSelfTestError({
    name: "missing custom provider remote compaction default",
    mutate: (compat) => {
      const check = semanticCheck(compat, "model-provider.custom-capabilities");
      check.file_contains_all = check.file_contains_all.filter(
        (value) => value !== "RemoteCompactionSupport::Unsupported",
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.model-provider.custom-capabilities.file_contains_all missing "RemoteCompactionSupport::Unsupported"',
  });

  assertSelfTestError({
    name: "missing namespace exposure gate removal",
    mutate: (compat) => {
      const check = semanticCheck(compat, "tools.namespace-gate-removal");
      check.file_contains_all = check.file_contains_all.filter(
        (value) => value !== "merge_into_namespaces(specs)",
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.tools.namespace-gate-removal.file_contains_all missing "merge_into_namespaces(specs)"',
  });

  assertSelfTestError({
    name: "missing partial answer phase",
    mutate: (compat) => {
      const check = semanticCheck(compat, "responses.partial-answer-phase");
      check.file_contains_all = check.file_contains_all.filter(
        (value) => value !== "MessagePhase::PartialAnswer",
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.responses.partial-answer-phase.file_contains_all missing "MessagePhase::PartialAnswer"',
  });

  assertSelfTestError({
    name: "missing remote MCP Windows bootstrap variables",
    mutate: (compat) => {
      const check = semanticCheck(compat, "rmcp.remote-windows-env");
      check.file_contains_all = check.file_contains_all.filter(
        (value) => value !== '.chain(["SYSTEMROOT", "TEMP", "TMP"].iter())',
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.rmcp.remote-windows-env.file_contains_all missing ".chain([\\"SYSTEMROOT\\", \\"TEMP\\", \\"TMP\\"].iter())"',
  });

  assertSelfTestError({
    name: "missing app-server root lineage",
    mutate: (compat) => {
      const check = semanticCheck(compat, "app-server.turn-lineage");
      check.file_contains_all = check.file_contains_all.filter(
        (value) => value !== "root_turn_id: Some(root_turn_id)",
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.app-server.turn-lineage.file_contains_all missing "root_turn_id: Some(root_turn_id)"',
  });

  assertSelfTestError({
    name: "missing app-server environment skills wire field",
    mutate: (compat) => {
      const check = semanticCheck(compat, "app-server.environment-skills-wire");
      check.file_contains_all = check.file_contains_all.filter(
        (value) => value !== "pub required: Option<Vec<String>>",
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.app-server.environment-skills-wire.file_contains_all missing "pub required: Option<Vec<String>>"',
  });

  assertSelfTestError({
    name: "missing skipped transport header",
    mutate: (compat) => {
      compat.proxy_skipped_headers = compat.proxy_skipped_headers.filter((header) => header !== "sec-websocket-*");
    },
    expectedMessage: "codex.compatibility.proxy_skipped_headers missing sec-websocket-*",
  });

  assertSelfTestError({
    name: "missing app-server lifecycle method",
    mutate: (compat) => {
      compat.app_server_protocol.required_methods = compat.app_server_protocol.required_methods.filter(
        (method) => method !== "turn/start",
      );
    },
    expectedMessage: "codex.compatibility.app_server_protocol.required_methods missing turn/start",
  });
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/compat/check-upstream-baseline.mjs [--baseline <path>] [--source <dir>] [--report <path>] [--json] [--self-test]",
        "",
        "Offline guard for critical upstream Codex runtime assumptions recorded in scripts/compat/upstream-baseline.json.",
      ].join("\n") + "\n",
    );
    return;
  }

  if (args.selfTest) {
    runSelfTest();
    process.stdout.write("upstream baseline guard self-test passed\n");
    return;
  }

  const baselineText = await fs.readFile(args.baseline, "utf8");
  const baseline = JSON.parse(baselineText);
  const validation = validateBaseline(baseline);
  const errors = [...validation.errors];
  const warnings = [...validation.warnings];
  if (args.source) {
    errors.push(
      ...(await validateSourceMarkers({
        compat: baseline?.codex?.compatibility,
        sourceRoot: args.source,
      })),
    );
  }
  const report = {
    baselinePath: args.baseline,
    sourcePath: args.source,
    generated_at: new Date().toISOString(),
    ok: errors.length === 0,
    errors,
    warnings,
    required: {
      critical_files: REQUIRED_CRITICAL_FILES,
      expected_headers: REQUIRED_EXPECTED_HEADERS,
      proxy_replaced_headers: REQUIRED_PROXY_REPLACED_HEADERS,
      proxy_skipped_headers: REQUIRED_PROXY_SKIPPED_HEADERS,
      expected_routes: REQUIRED_EXPECTED_ROUTES,
      expected_stream_events: REQUIRED_STREAM_EVENTS,
      semantic_checks: REQUIRED_SEMANTIC_CHECKS.map((check) => check.id),
    },
  };

  if (args.report) {
    await fs.writeFile(args.report, `${JSON.stringify(report, null, 2)}\n`);
  }

  if (args.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    process.stdout.write(renderReport(report));
  }

  if (!report.ok) {
    process.exitCode = 1;
  }
}

await main();
