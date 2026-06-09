#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const DEFAULT_BASELINE_PATH = path.join(repoRoot, "scripts/compat/upstream-baseline.json");

const REQUIRED_CRITICAL_FILES = [
  "codex-rs/core/src/client.rs",
  "codex-rs/core/src/compact_remote.rs",
  "codex-rs/core/src/turn_metadata.rs",
  "codex-rs/model-provider-info/src/lib.rs",
  "codex-rs/core/src/realtime_conversation.rs",
  "codex-rs/codex-api/src/sse/responses.rs",
  "codex-rs/codex-api/src/endpoint/responses_websocket.rs",
];

const REQUIRED_FILE_CONTAINS = {
  "codex-rs/core/src/client.rs": [
    "RESPONSES_ENDPOINT",
    "/responses",
    "RESPONSES_COMPACT_ENDPOINT",
    "/responses/compact",
    "build_responses_headers",
    "build_responses_identity_headers",
    "build_session_headers",
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
    "x-codex-beta-features",
    "OPENAI_BETA_HEADER",
    "responses_websockets=2026-02-06",
    "x-client-request-id",
    "stream_responses_websocket",
    "compact_conversation_history",
    "turn_metadata_header",
    "parse_turn_metadata_header",
  ],
  "codex-rs/core/src/compact_remote.rs": [
    "run_remote_compact_task",
    "compact_conversation_history",
    "CompactionImplementation::ResponsesCompact",
    "ContextCompactionItem",
    "CompactionTurnMetadata",
    "current_header_value_for_compaction",
    "turn_metadata_header",
  ],
  "codex-rs/core/src/turn_metadata.rs": [
    "REQUEST_KIND_KEY",
    "request_kind",
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
    "TurnMetadataRequestKind",
    "Turn",
    "Prewarm",
    "Compaction",
    "Memory",
    "current_header_value_for_model_request_kind",
    "current_header_value_for_compaction",
    "to_ascii_json_string",
  ],
  "codex-rs/model-provider-info/src/lib.rs": [
    "ModelProviderInfo",
    "supports_remote_compaction",
    "is_openai()",
    "is_azure_responses_provider",
  ],
  "codex-rs/core/src/realtime_conversation.rs": [
    "ConversationStartTransport::Websocket",
    "realtime_request_headers",
    "RealtimeWsVersion::V1",
    "openai-alpha",
    "quicksilver=v1",
  ],
  "codex-rs/codex-api/src/sse/responses.rs": [
    "spawn_response_stream",
    "process_sse",
    "process_responses_event",
    "x-codex-turn-state",
    "response.completed",
    "response.failed",
    "response.metadata",
    "openai-model",
    "x-reasoning-included",
    "X-Models-Etag",
    "insufficient_quota",
    "rate_limit_exceeded",
  ],
  "codex-rs/codex-api/src/endpoint/responses_websocket.rs": [
    "ResponsesWebsocketConnection",
    "websocket_url_for_path(\"responses\")",
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
  "/memories/trace_summarize",
  "websocket_url_for_path(\"responses\")",
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
    id: "sse.responses-http-route-behavior",
    kind: "route_event_group",
    file: "codex-rs/codex-api/src/sse/responses.rs",
    file_contains_all: [
      "spawn_response_stream",
      "process_sse",
      "process_responses_event",
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
      "turn_metadata_header",
      "parse_turn_metadata_header",
    ],
    expected_routes_all: ["/responses/compact"],
  },
  {
    id: "client.responses-compact-metadata-header",
    kind: "header_group",
    file: "codex-rs/core/src/client.rs",
    file_contains_all: [
      "compact_conversation_history",
      "turn_metadata_header",
      "build_responses_headers",
      "parse_turn_metadata_header",
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
      "build_responses_identity_headers",
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
      "x-client-request-id",
    ],
  },
  {
    id: "proxy.preserved-headers",
    kind: "header_group",
    file: "codex-rs/core/src/client.rs",
    file_contains_all: [
      "build_responses_headers",
      "build_responses_identity_headers",
      "build_session_headers",
      "x-openai-subagent",
      "x-codex-turn-state",
      "x-codex-turn-metadata",
      "x-codex-beta-features",
      "x-openai-internal-codex-responses-lite",
      "ws_request_header_x_openai_internal_codex_responses_lite",
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
      "RealtimeWsVersion::V1",
      "openai-alpha",
      "quicksilver=v1",
    ],
  },
  {
    id: "compact.remote-responses-compact",
    kind: "route",
    file: "codex-rs/core/src/compact_remote.rs",
    file_contains_all: [
      "run_remote_compact_task",
      "compact_conversation_history",
      "CompactionImplementation::ResponsesCompact",
      "ContextCompactionItem",
    ],
    expected_routes_all: ["/responses/compact"],
  },
  {
    id: "compact.remote-turn-metadata-header",
    kind: "header_group",
    file: "codex-rs/core/src/compact_remote.rs",
    file_contains_all: [
      "CompactionTurnMetadata",
      "current_header_value_for_compaction",
      "turn_metadata_header",
      "compact_conversation_history",
    ],
    expected_routes_all: ["/responses/compact"],
    expected_headers_all: ["x-codex-turn-metadata"],
  },
  {
    id: "turn-metadata.request-kind-window",
    kind: "metadata_group",
    file: "codex-rs/core/src/turn_metadata.rs",
    file_contains_all: [
      "REQUEST_KIND_KEY",
      "request_kind",
      "WINDOW_ID_KEY",
      "window_id",
      "TurnMetadataRequestKind",
      "Turn",
      "Prewarm",
      "Memory",
      "current_header_value_for_model_request_kind",
    ],
    expected_headers_all: ["x-codex-turn-metadata"],
  },
  {
    id: "turn-metadata.compaction-dispatch",
    kind: "metadata_group",
    file: "codex-rs/core/src/turn_metadata.rs",
    file_contains_all: [
      "COMPACTION_KEY",
      "compaction",
      "CompactionTurnMetadata",
      "CompactionTrigger",
      "CompactionReason",
      "CompactionImplementation",
      "CompactionPhase",
      "CompactionStrategy",
      "current_header_value_for_compaction",
    ],
    expected_headers_all: ["x-codex-turn-metadata"],
  },
  {
    id: "model-provider.remote-compaction-capability",
    kind: "capability_gate",
    file: "codex-rs/model-provider-info/src/lib.rs",
    file_contains_all: [
      "ModelProviderInfo",
      "supports_remote_compaction",
      "is_openai()",
      "is_azure_responses_provider",
    ],
  },
  {
    id: "sse.responses-events",
    kind: "event_group",
    file: "codex-rs/codex-api/src/sse/responses.rs",
    file_contains_all: ["process_responses_event", "response.completed", "response.failed", "response.metadata"],
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
      "x-codex-turn-state",
      "parse_wrapped_websocket_error_event",
      "websocket_connection_limit_reached",
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
    file_contains_all: ["response.completed", "codex.rate_limits"],
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
      compatibility: {
        upstream_repository: "self-test",
        guard_command: "node scripts/compat/check-upstream-baseline.mjs --self-test",
        format_version: COMPAT_FORMAT_VERSION_WITH_SEMANTIC_CHECKS,
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
  mutate(baseline.codex.compatibility);
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
    name: "missing skipped transport header",
    mutate: (compat) => {
      compat.proxy_skipped_headers = compat.proxy_skipped_headers.filter((header) => header !== "sec-websocket-*");
    },
    expectedMessage: "codex.compatibility.proxy_skipped_headers missing sec-websocket-*",
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
