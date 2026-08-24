#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { openaiCodexVersion, repoRoot } from "../npm/common.mjs";

const DEFAULT_BASELINE_PATH = path.join(repoRoot, "scripts/compat/upstream-baseline.json");
const EXPECTED_CODEX_RELEASE = `rust-v${openaiCodexVersion}`;

const REQUIRED_CRITICAL_FILES = [
  "codex-rs/core/src/client.rs",
  "codex-rs/core/src/compact_remote.rs",
  "codex-rs/core/src/compact_remote_request.rs",
  "codex-rs/core/src/compact_remote_v2.rs",
  "codex-rs/core/src/turn_metadata.rs",
  "codex-rs/core/src/responses_metadata.rs",
  "codex-rs/models-manager/models.json",
  "codex-rs/model-provider/src/auth.rs",
  "codex-rs/codex-mcp/src/tools.rs",
  "codex-rs/codex-mcp/src/connection_manager_tests.rs",
  "codex-rs/model-provider-info/src/lib.rs",
  "codex-rs/model-provider/src/amazon_bedrock/catalog.rs",
  "codex-rs/model-provider/src/provider.rs",
  "codex-rs/core/src/realtime_conversation.rs",
  "codex-rs/codex-api/src/endpoint/realtime_call.rs",
  "codex-rs/codex-api/src/safety_buffering.rs",
  "codex-rs/codex-api/src/sse/responses.rs",
  "codex-rs/codex-api/src/endpoint/compact.rs",
  "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
  "codex-rs/core/src/config/mod.rs",
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
  "codex-rs/exec/src/cli.rs",
  "codex-rs/exec/src/lib.rs",
  "codex-rs/protocol/src/protocol.rs",
  "codex-rs/app-server/README.md",
  "codex-rs/app-server-protocol/src/export.rs",
  "codex-rs/app-server-protocol/src/rpc.rs",
  "codex-rs/app-server-protocol/src/protocol/common.rs",
  "codex-rs/app-server-protocol/src/protocol/v2/thread.rs",
  "codex-rs/app-server/src/message_processor.rs",
  "codex-rs/app-server/src/request_processors/initialize_processor.rs",
  "codex-rs/app-server/src/request_processors/thread_processor.rs",
  "codex-rs/app-server/src/request_processors/turn_processor.rs",
  "codex-rs/app-server/src/request_serialization.rs",
];

const REQUIRED_FILE_CONTAINS = {
  "codex-rs/core/src/client.rs": [
    "RESPONSES_ENDPOINT",
    "/responses",
    "RESPONSES_COMPACT_ENDPOINT",
    "/responses/compact",
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
    "compact_conversation_history",
    "compact_input",
    "client_metadata",
    "prepare_response_items_for_request",
    "responses_request_properties_match",
  ],
  "codex-rs/core/src/compact_remote.rs": [
    "run_remote_compact_task",
    "run_inline_remote_auto_compact_task",
    "run_remote_compact_attempt",
    "CompactionImplementation::ResponsesCompact",
    "ContextCompactionItem",
    "CompactionTurnMetadata",
    "turn_state",
  ],
  "codex-rs/core/src/compact_remote_request.rs": [
    "run_remote_compact_attempt",
    "compact_conversation_history",
    "CompactConversationRequestSettings",
    "CompactionTurnMetadata",
    "CodexResponsesRequestKind::Compaction",
    "to_responses_metadata",
    "turn_state",
    "responses_metadata",
  ],
  "codex-rs/core/src/compact_remote_v2.rs": [
    "Feature::CompactionImageBudget",
    "RetainedImageBudget::Enabled",
    "truncate_retained_messages",
    "images::truncate_message_to_token_budget",
    "remaining = 0",
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
  "codex-rs/models-manager/models.json": [
    "\"slug\": \"gpt-5.6-sol\"",
    "\"slug\": \"gpt-5.6-terra\"",
    "\"slug\": \"gpt-5.6-luna\"",
    "\"context_window\": 272000",
    "\"max_context_window\": 872000",
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
  ],
  "codex-rs/model-provider/src/amazon_bedrock/catalog.rs": [
    "static_model_catalog",
    "normalize_bedrock_catalog",
    "gpt_5_6_bedrock_model",
    "AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID",
    "AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID",
    "AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID",
    "ReasoningEffort::Ultra",
    ".retain(|level| level.effort != ReasoningEffort::Ultra)",
    "model.additional_speed_tiers.clear()",
    "model.service_tiers.clear()",
    "model.default_service_tier = None",
    "WebSearchToolType::Text",
    "model.use_responses_lite = false",
    "model.tool_mode = None",
  ],
  "codex-rs/model-provider/src/provider.rs": [
    "RemoteCompactionSupport",
    "ProviderCapabilities",
    "remote_compaction",
    "is_azure_responses_provider",
    "RemoteCompactionSupport::V2",
    "amazon_bedrock_provider_creates_static_models_manager",
    "openai.gpt-5.5",
    "openai.gpt-5.4",
    "openai.gpt-5.6-sol",
    "openai.gpt-5.6-terra",
    "openai.gpt-5.6-luna",
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
    "insufficient_quota",
    "rate_limit_exceeded",
  ],
  "codex-rs/codex-api/src/endpoint/compact.rs": [
    "CompactClient",
    "responses/compact",
    "compact_input",
    "X_CODEX_TURN_STATE_HEADER",
    "x-codex-turn-state",
    "turn_state",
    "headers",
  ],
  "codex-rs/codex-api/src/endpoint/responses_websocket.rs": [
    "ResponsesWebsocketConnection",
    "websocket_url_for_path(\"responses\")",
    "merge_request_headers",
    "add_auth_headers",
    "treatment_from_headers",
    "SafetyBufferingTreatment",
    "safety_buffering(treatment)",
    "x-codex-turn-state",
    "response.completed",
    "codex.rate_limits",
    "openai-model",
    "x-reasoning-included",
    "x-models-etag",
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
    "default_enabled: false",
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
    "MAX_COMPACT_TOOL_SCHEMA_DEPTH",
    "prune_schema_compositions",
  ],
  "codex-rs/exec/src/cli.rs": [
    "ThreadSource",
    "long = \"thread-source\"",
    "value_name = \"SOURCE\"",
    "global = true",
    "pub thread_source: Option<ThreadSource>",
  ],
  "codex-rs/exec/src/lib.rs": [
    "ThreadSource::User",
    "ThreadForkParams",
    "thread_start_params_from_config",
    "thread_source: Some(thread_source.clone())",
  ],
  "codex-rs/protocol/src/protocol.rs": [
    "pub enum ThreadSource",
    "Feature(String)",
    "\"memory_consolidation\"",
    "other => Ok(ThreadSource::Feature(other.to_string()))",
  ],
  "codex-rs/app-server/README.md": [
    "codex app-server generate-json-schema",
    "--experimental",
    "thread/start",
    "thread/resume",
    "thread/fork",
    "turn/start",
    "thread/started",
    "turn/started",
    "turn/completed",
    "JSON-RPC",
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
  ],
  "codex-rs/app-server/src/request_serialization.rs": [
    "ClientRequestSerializationScope",
    "RequestSerializationQueueKey",
    "Thread",
    "ThreadPath",
    "RequestSerializationAccess",
    "QueuedInitializedRequest",
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
  "/responses/compact",
  "/realtime/calls",
  "alpha/search",
  "/memories/trace_summarize",
  "websocket_url_for_path(\"responses\")",
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
    id: "client.responses-route",
    kind: "route",
    file: "codex-rs/core/src/client.rs",
    file_contains_all: ["RESPONSES_ENDPOINT", "/responses"],
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
    id: "client.responses-compact-route",
    kind: "route",
    file: "codex-rs/core/src/client.rs",
    file_contains_all: [
      "RESPONSES_COMPACT_ENDPOINT",
      "/responses/compact",
      "compact_conversation_history",
      "compact_input",
      "CodexResponsesMetadata",
    ],
    expected_routes_all: ["/responses/compact"],
  },
  {
    id: "client.responses-compact-metadata-header",
    kind: "header_group",
    file: "codex-rs/core/src/client.rs",
    file_contains_all: [
      "compact_conversation_history",
      "build_responses_headers",
      "build_responses_compatibility_headers",
      "CodexResponsesMetadata",
      "compact_input",
    ],
    expected_routes_all: ["/responses/compact"],
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
    id: "compact.remote-responses-compact",
    kind: "route",
    file: "codex-rs/core/src/compact_remote_request.rs",
    file_contains_all: [
      "run_remote_compact_attempt",
      "compact_conversation_history",
      "CompactConversationRequestSettings",
      "CodexResponsesRequestKind::Compaction",
      "to_responses_metadata",
    ],
    expected_routes_all: ["/responses/compact"],
  },
  {
    id: "compact.remote-turn-metadata-header",
    kind: "header_group",
    file: "codex-rs/core/src/compact_remote_request.rs",
    file_contains_all: [
      "CompactionTurnMetadata",
      "compact_conversation_history",
      "CodexResponsesRequestKind::Compaction",
      "responses_metadata",
    ],
    expected_routes_all: ["/responses/compact"],
    expected_headers_all: ["x-codex-turn-metadata"],
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
    id: "compact.response-turn-state",
    kind: "header_group",
    file: "codex-rs/codex-api/src/endpoint/compact.rs",
    file_contains_all: [
      "compact_input",
      "X_CODEX_TURN_STATE_HEADER",
      "x-codex-turn-state",
      "turn_state",
      "headers",
    ],
    expected_routes_all: ["/responses/compact"],
    expected_headers_all: ["x-codex-turn-state"],
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
    file: "codex-rs/model-provider/src/provider.rs",
    file_contains_all: [
      "RemoteCompactionSupport",
      "ProviderCapabilities",
      "remote_compaction",
      "is_azure_responses_provider",
      "RemoteCompactionSupport::V2",
    ],
  },
  {
    id: "model-provider.bedrock-gpt-5-6-catalog",
    kind: "provider_catalog",
    file: "codex-rs/model-provider/src/amazon_bedrock/catalog.rs",
    file_contains_all: [
      "static_model_catalog",
      "normalize_bedrock_catalog",
      "gpt_5_6_bedrock_model",
      "AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID",
      "AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID",
      "AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID",
      "ReasoningEffort::Ultra",
      ".retain(|level| level.effort != ReasoningEffort::Ultra)",
      "model.additional_speed_tiers.clear()",
      "model.service_tiers.clear()",
      "model.default_service_tier = None",
      "WebSearchToolType::Text",
      "model.use_responses_lite = false",
      "model.tool_mode = None",
    ],
  },
  {
    id: "model-provider.bedrock-static-manager-models",
    kind: "provider_catalog",
    file: "codex-rs/model-provider/src/provider.rs",
    file_contains_all: [
      "amazon_bedrock_provider_creates_static_models_manager",
      "openai.gpt-5.5",
      "openai.gpt-5.4",
      "openai.gpt-5.6-sol",
      "openai.gpt-5.6-terra",
      "openai.gpt-5.6-luna",
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
    file: "codex-rs/codex-api/src/sse/responses.rs",
    file_contains_all: ["insufficient_quota", "rate_limit_exceeded"],
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
    file_contains_all: ["ResponsesWebsocketConnection", "websocket_url_for_path(\"responses\")"],
    expected_routes_all: ["websocket_url_for_path(\"responses\")"],
  },
  {
    id: "websocket.session-behavior",
    kind: "route_event_group",
    file: "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
    file_contains_all: [
      "ResponsesWebsocketConnection",
      "websocket_url_for_path(\"responses\")",
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
    expected_routes_all: ["websocket_url_for_path(\"responses\")"],
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
    id: "features.compaction-image-budget",
    kind: "feature_gate",
    file: "codex-rs/features/src/lib.rs",
    file_contains_all: [
      "CompactionImageBudget",
      "compaction_image_budget",
      "key: \"compaction_image_budget\"",
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
    id: "app-server.schema-generation",
    kind: "schema_generation",
    file: "codex-rs/app-server/README.md",
    file_contains_all: ["codex app-server generate-json-schema", "--experimental"],
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

  if (compat.tested_codex_release !== EXPECTED_CODEX_RELEASE) {
    errors.push(
      `codex.compatibility.tested_codex_release must match bundled Codex ${EXPECTED_CODEX_RELEASE}`,
    );
  }

  if (baseline?.codex?.latestRelease?.tag_name !== EXPECTED_CODEX_RELEASE) {
    errors.push(`codex.latestRelease.tag_name must match bundled Codex ${EXPECTED_CODEX_RELEASE}`);
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
      latestRelease: { tag_name: EXPECTED_CODEX_RELEASE },
      compatibility: {
        upstream_repository: "self-test",
        guard_command: "node scripts/compat/check-upstream-baseline.mjs --self-test",
        format_version: COMPAT_FORMAT_VERSION_WITH_SEMANTIC_CHECKS,
        tested_codex_release: EXPECTED_CODEX_RELEASE,
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
      compat.tested_codex_release = "rust-v0.0.0";
    },
    expectedMessage: `codex.compatibility.tested_codex_release must match bundled Codex ${EXPECTED_CODEX_RELEASE}`,
  });

  assertSelfTestError({
    name: "mismatched latest release",
    mutate: (_compat, baseline) => {
      baseline.codex.latestRelease.tag_name = "rust-v0.0.0";
    },
    expectedMessage: `codex.latestRelease.tag_name must match bundled Codex ${EXPECTED_CODEX_RELEASE}`,
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
    name: "missing Bedrock GPT-5.6 catalog token",
    mutate: (compat) => {
      const check = semanticCheck(compat, "model-provider.bedrock-gpt-5-6-catalog");
      check.file_contains_all = check.file_contains_all.filter(
        (token) => token !== "AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID",
      );
    },
    expectedMessage:
      'codex.compatibility.semantic_checks.model-provider.bedrock-gpt-5-6-catalog.file_contains_all missing "AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID"',
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
        "Usage: node scripts/compat/check-upstream-baseline.mjs [--baseline <path>] [--report <path>] [--json] [--self-test]",
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
  const { errors, warnings } = validateBaseline(baseline);
  const report = {
    baselinePath: args.baseline,
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
