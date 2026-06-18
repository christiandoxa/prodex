//! Runtime proxy boundary primitives.
//!
//! Most modules are side-effect-free boundary types and helpers.
//! The bounded WebSocket TCP/DNS executor also lives here so the binary can keep
//! transport policy wiring thin while still owning runtime state and persistence.

use std::borrow::Cow;

pub use prodex_runtime_state::{
    RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX, RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX,
    RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX, RuntimeRouteKind,
    runtime_compact_session_lineage_key, runtime_compact_turn_state_lineage_key,
    runtime_is_compact_session_lineage_key, runtime_is_response_turn_state_lineage_key,
    runtime_profile_route_bad_pairing_key, runtime_profile_route_circuit_health_key,
    runtime_profile_route_circuit_key, runtime_profile_route_circuit_profile_name,
    runtime_profile_route_circuit_reopen_key, runtime_profile_route_health_key,
    runtime_profile_route_key_parts, runtime_profile_route_performance_key,
    runtime_profile_route_success_streak_key, runtime_profile_transport_backoff_key,
    runtime_profile_transport_backoff_key_parts, runtime_profile_transport_backoff_key_valid,
    runtime_profile_transport_backoff_profile_name, runtime_response_turn_state_lineage_key,
    runtime_response_turn_state_lineage_parts, runtime_route_coupled_kinds,
    runtime_route_kind_from_label, runtime_route_kind_label,
};

mod admission;
mod attempt_outcome;
mod buffered_response;
mod chain_log;
mod continuation;
mod error_policy;
mod failure_response;
mod gateway_guardrails;
mod gateway_policy;
mod health;
mod lineage;
mod local_bridge;
mod log_event;
mod payload_detection;
mod previous_response_log;
mod previous_response_orchestration;
mod quota;
mod response_forwarding;
mod route_affinity_log;
mod selection_plan;
mod selection_policy;
mod smart_context;
mod transport_failure;
mod upstream;
mod websocket_message;
mod websocket_proxy;
mod websocket_response_tracking;
mod websocket_tcp_connect_executor;

pub use self::admission::*;
pub use self::attempt_outcome::*;
pub use self::buffered_response::*;
pub use self::chain_log::*;
pub use self::continuation::*;
pub use self::error_policy::*;
pub use self::failure_response::*;
pub use self::gateway_guardrails::*;
pub use self::gateway_policy::*;
pub use self::health::*;
pub use self::lineage::*;
pub use self::local_bridge::*;
pub use self::log_event::*;
pub use self::payload_detection::*;
pub use self::previous_response_log::*;
pub use self::previous_response_orchestration::*;
pub use self::quota::*;
pub use self::response_forwarding::*;
pub use self::route_affinity_log::*;
pub use self::selection_plan::*;
pub use self::selection_policy::*;
pub use self::smart_context::*;
pub use self::transport_failure::*;
pub use self::upstream::*;
pub use self::websocket_message::*;
pub use self::websocket_proxy::*;
pub use self::websocket_response_tracking::*;
pub use self::websocket_tcp_connect_executor::*;

pub const RUNTIME_PROXY_OPENAI_UPSTREAM_PATH: &str = "/backend-api/codex";
pub const RUNTIME_PROXY_OPENAI_MOUNT_PATH: &str = "/backend-api/prodex";
pub const RUNTIME_PROXY_ANTHROPIC_MESSAGES_PATH: &str = "/v1/messages";
pub const LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX: &str = "/backend-api/prodex/v";
pub const PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER: &str = "X-Prodex-Internal-Request-Origin";
pub const PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES: &str = "anthropic_messages";
pub const RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS: u64 = if cfg!(test) { 80 } else { 750 };
pub const RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS: u64 = if cfg!(test) { 80 } else { 750 };
pub const RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS: u64 = if cfg!(test) { 25 } else { 200 };
pub const RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS: u64 =
    if cfg!(test) { 25 } else { 200 };
pub const RUNTIME_PROXY_INTERACTIVE_WAIT_MULTIPLIER: u64 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProxyRequest {
    pub method: String,
    pub path_and_query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub fn runtime_route_kind_inflight_context(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses_http",
        RuntimeRouteKind::Compact => "compact_http",
        RuntimeRouteKind::Websocket => "websocket_session",
        RuntimeRouteKind::Standard => "standard_http",
    }
}

pub fn path_without_query(path_and_query: &str) -> &str {
    path_and_query
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(path_and_query)
}

pub fn runtime_proxy_openai_suffix(path: &str) -> Option<&str> {
    if let Some(suffix) = path.strip_prefix(LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX)
        && let Some(version_suffix_index) = suffix.find('/')
    {
        return Some(&suffix[version_suffix_index..]);
    }

    if let Some(suffix) = path.strip_prefix(RUNTIME_PROXY_OPENAI_MOUNT_PATH)
        && (suffix.is_empty() || suffix.starts_with('/'))
    {
        return Some(suffix);
    }

    None
}

pub fn runtime_proxy_normalize_openai_path(path_and_query: &str) -> Cow<'_, str> {
    let (path, query) = match path_and_query.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (path_and_query, None),
    };
    let Some(suffix) = runtime_proxy_openai_suffix(path) else {
        return Cow::Borrowed(path_and_query);
    };

    let mut normalized =
        String::with_capacity(path_and_query.len() + RUNTIME_PROXY_OPENAI_UPSTREAM_PATH.len());
    normalized.push_str(RUNTIME_PROXY_OPENAI_UPSTREAM_PATH);
    normalized.push_str(suffix);
    if let Some(query) = query {
        normalized.push('?');
        normalized.push_str(query);
    }
    Cow::Owned(normalized)
}

pub fn is_runtime_responses_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/codex/responses")
}

pub fn is_runtime_anthropic_messages_path(path_and_query: &str) -> bool {
    path_without_query(path_and_query).ends_with(RUNTIME_PROXY_ANTHROPIC_MESSAGES_PATH)
}

pub fn is_runtime_chat_completions_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/chat/completions")
}

pub fn is_runtime_compact_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/responses/compact")
}

pub fn runtime_proxy_request_lane(path: &str, websocket: bool) -> RuntimeRouteKind {
    if websocket {
        RuntimeRouteKind::Websocket
    } else if is_runtime_compact_path(path) {
        RuntimeRouteKind::Compact
    } else if is_runtime_responses_path(path)
        || is_runtime_chat_completions_path(path)
        || is_runtime_anthropic_messages_path(path)
    {
        RuntimeRouteKind::Responses
    } else {
        RuntimeRouteKind::Standard
    }
}

pub fn runtime_proxy_request_is_long_lived(path: &str, websocket: bool) -> bool {
    websocket
        || is_runtime_responses_path(path)
        || is_runtime_chat_completions_path(path)
        || is_runtime_anthropic_messages_path(path)
}

pub fn runtime_proxy_request_header_value<'a>(
    headers: &'a [(String, String)],
    name: &str,
) -> Option<&'a str> {
    headers
        .iter()
        .find_map(|(header_name, value)| {
            header_name
                .eq_ignore_ascii_case(name)
                .then_some(value.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub fn runtime_proxy_request_origin(headers: &[(String, String)]) -> Option<&str> {
    runtime_proxy_request_header_value(headers, PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER)
}

pub fn runtime_proxy_request_prefers_interactive_inflight_wait(
    request: &RuntimeProxyRequest,
) -> bool {
    runtime_proxy_request_origin(&request.headers).is_some_and(|origin| {
        origin.eq_ignore_ascii_case(PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES)
    })
}

pub fn runtime_proxy_request_prefers_inflight_wait(request: &RuntimeProxyRequest) -> bool {
    is_runtime_responses_path(&request.path_and_query)
        || is_runtime_chat_completions_path(&request.path_and_query)
        || runtime_proxy_request_prefers_interactive_inflight_wait(request)
}

pub fn runtime_proxy_interactive_wait_budget_ms(path: &str, base_budget_ms: u64) -> u64 {
    if is_runtime_anthropic_messages_path(path) {
        base_budget_ms.saturating_mul(RUNTIME_PROXY_INTERACTIVE_WAIT_MULTIPLIER)
    } else {
        base_budget_ms
    }
}

pub fn runtime_proxy_admission_wait_budget(path: &str, pressure_mode: bool) -> std::time::Duration {
    let base_budget_ms = if pressure_mode {
        RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS
    } else {
        RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS
    };
    std::time::Duration::from_millis(runtime_proxy_interactive_wait_budget_ms(
        path,
        base_budget_ms,
    ))
}

pub fn runtime_proxy_request_inflight_wait_budget(
    request: &RuntimeProxyRequest,
    pressure_mode: bool,
) -> std::time::Duration {
    if runtime_proxy_request_prefers_interactive_inflight_wait(request) {
        runtime_proxy_admission_wait_budget(RUNTIME_PROXY_ANTHROPIC_MESSAGES_PATH, pressure_mode)
    } else if runtime_proxy_request_prefers_inflight_wait(request) {
        runtime_proxy_admission_wait_budget(&request.path_and_query, pressure_mode)
    } else {
        std::time::Duration::ZERO
    }
}

pub fn runtime_proxy_long_lived_queue_wait_budget(
    path: &str,
    pressure_mode: bool,
) -> std::time::Duration {
    let base_budget_ms = if pressure_mode {
        RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS
    } else {
        RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS
    };
    std::time::Duration::from_millis(runtime_proxy_interactive_wait_budget_ms(
        path,
        base_budget_ms,
    ))
}

pub fn is_runtime_realtime_call_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/realtime/calls")
}

pub fn is_runtime_realtime_websocket_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/realtime")
}

pub fn runtime_request_previous_response_id(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_request_previous_response_id_from_bytes(&request.body)
}

pub fn runtime_request_prompt_cache_key(request: &RuntimeProxyRequest) -> Option<String> {
    if request.body.is_empty() {
        return None;
    }

    let value = serde_json::from_slice::<serde_json::Value>(&request.body).ok()?;
    runtime_request_prompt_cache_key_from_value(&value)
}

pub fn runtime_request_previous_response_id_from_bytes(body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }

    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    runtime_request_previous_response_id_from_value(&value)
}

pub fn runtime_request_previous_response_id_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    value
        .get("previous_response_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn runtime_request_prompt_cache_key_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("prompt_cache_key")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct RuntimeWebsocketRequestMetadata {
    pub previous_response_id: Option<String>,
    pub session_id: Option<String>,
    pub prompt_cache_key: Option<String>,
    pub turn_state: Option<String>,
    pub requires_previous_response_affinity: bool,
    pub previous_response_fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

pub fn parse_runtime_websocket_request_metadata(
    request_text: &str,
) -> RuntimeWebsocketRequestMetadata {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(request_text) else {
        return RuntimeWebsocketRequestMetadata::default();
    };
    RuntimeWebsocketRequestMetadata {
        previous_response_id: runtime_request_previous_response_id_from_value(&value),
        session_id: runtime_request_session_id_from_value(&value),
        prompt_cache_key: runtime_request_prompt_cache_key_from_value(&value),
        turn_state: runtime_request_turn_state_from_value(&value),
        requires_previous_response_affinity:
            runtime_request_value_requires_previous_response_affinity(&value),
        previous_response_fresh_fallback_shape:
            runtime_request_value_previous_response_fresh_fallback_shape(&value),
    }
}

pub fn runtime_request_previous_response_id_from_text(request_text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(request_text).ok()?;
    runtime_request_previous_response_id_from_value(&value)
}

pub fn runtime_request_value_requires_previous_response_affinity(
    value: &serde_json::Value,
) -> bool {
    if runtime_request_previous_response_id_from_value(value).is_none() {
        return false;
    }

    value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(runtime_request_value_previous_response_input_item_is_tool_output)
        })
}

fn runtime_request_value_previous_response_input_item_is_tool_output(
    item: &serde_json::Value,
) -> bool {
    let Some(object) = item.as_object() else {
        return false;
    };
    let item_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let has_call_id = object
        .get("call_id")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|call_id| !call_id.trim().is_empty());
    has_call_id && item_type.ends_with("_call_output")
}

pub fn runtime_request_value_previous_response_fresh_fallback_shape(
    value: &serde_json::Value,
) -> Option<RuntimePreviousResponseFreshFallbackShape> {
    runtime_request_previous_response_id_from_value(value)?;

    let has_session_affinity = runtime_request_session_id_from_value(value).is_some();
    let input = value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has_context_dependent_input = input
        .iter()
        .any(|item| !runtime_request_value_previous_response_input_item_is_tool_output(item));
    let tool_output_only = !input.is_empty()
        && input
            .iter()
            .all(runtime_request_value_previous_response_input_item_is_tool_output);

    Some(if tool_output_only {
        RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly
    } else if has_context_dependent_input {
        RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation
    } else if has_session_affinity {
        RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay
    } else {
        RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly
    })
}

pub fn runtime_request_previous_response_fresh_fallback_shape(
    request: &RuntimeProxyRequest,
) -> Option<RuntimePreviousResponseFreshFallbackShape> {
    let body_shape = serde_json::from_slice::<serde_json::Value>(&request.body)
        .ok()
        .and_then(|value| runtime_request_value_previous_response_fresh_fallback_shape(&value));
    runtime_previous_response_fresh_fallback_shape_with_session(
        body_shape,
        runtime_request_explicit_session_id(request).is_some()
            || runtime_request_session_id_from_turn_metadata(request).is_some(),
    )
}

pub fn runtime_request_requires_previous_response_affinity(request: &RuntimeProxyRequest) -> bool {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .map(|value| runtime_request_value_requires_previous_response_affinity(&value))
        .unwrap_or(false)
}

pub fn runtime_websocket_previous_response_requires_previous_response_affinity(
    trusted_previous_response_affinity: bool,
    previous_response_id: Option<&str>,
    request_turn_state: Option<&str>,
) -> bool {
    trusted_previous_response_affinity
        && previous_response_id.is_some()
        && request_turn_state.is_none()
}

pub fn runtime_websocket_request_requires_locked_previous_response_affinity(
    request_requires_previous_response_affinity: bool,
    trusted_previous_response_affinity: bool,
    previous_response_id: Option<&str>,
    request_turn_state: Option<&str>,
) -> bool {
    request_requires_previous_response_affinity
        || runtime_websocket_previous_response_requires_previous_response_affinity(
            trusted_previous_response_affinity,
            previous_response_id,
            request_turn_state,
        )
}

pub fn runtime_request_turn_state(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_proxy_request_header_value(&request.headers, "x-codex-turn-state").map(str::to_string)
}

pub fn runtime_request_turn_state_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("x-codex-turn-state")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("client_metadata")
                .and_then(|metadata| metadata.get("x-codex-turn-state"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn runtime_request_session_id_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("client_metadata")
                .and_then(|metadata| metadata.get("session_id"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn runtime_request_session_id_from_turn_metadata(
    request: &RuntimeProxyRequest,
) -> Option<String> {
    request
        .headers
        .iter()
        .find_map(|(name, value)| {
            name.eq_ignore_ascii_case("x-codex-turn-metadata")
                .then_some(value.as_str())
        })
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| runtime_request_session_id_from_value(&value))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExplicitSessionId(String);

impl RuntimeExplicitSessionId {
    pub fn from_header_value(value: &str) -> Option<Self> {
        let value = value.trim();
        (!value.is_empty()).then(|| Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::ops::Deref for RuntimeExplicitSessionId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

pub fn runtime_request_explicit_session_id(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeExplicitSessionId> {
    runtime_proxy_request_header_value(&request.headers, "session_id")
        .or_else(|| runtime_proxy_request_header_value(&request.headers, "session-id"))
        .or_else(|| runtime_proxy_request_header_value(&request.headers, "x-session-id"))
        .and_then(RuntimeExplicitSessionId::from_header_value)
}

pub fn runtime_request_session_id(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_request_explicit_session_id(request)
        .map(RuntimeExplicitSessionId::into_string)
        .or_else(|| runtime_request_session_id_from_turn_metadata(request))
        .or_else(|| {
            serde_json::from_slice::<serde_json::Value>(&request.body)
                .ok()
                .and_then(|value| runtime_request_session_id_from_value(&value))
        })
}

pub fn runtime_request_without_previous_response_id(
    _request: &RuntimeProxyRequest,
) -> Option<RuntimeProxyRequest> {
    None
}

pub fn runtime_request_text_without_previous_response_id(_request_text: &str) -> Option<String> {
    None
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
