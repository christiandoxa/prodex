//! Runtime proxy boundary primitives.
//!
//! This crate intentionally starts with side-effect-free types and helpers only.
//! The live proxy keeps owning transport, affinity, rotation, and persistence
//! until those dependencies can be split without changing hot-path behavior.

use std::borrow::Cow;
use std::collections::BTreeMap;

mod attempt_outcome;
mod buffered_response;
mod failure_response;
mod health;
mod payload_detection;
mod transport_failure;
mod websocket_message;

pub use self::attempt_outcome::*;
pub use self::buffered_response::*;
pub use self::failure_response::*;
pub use self::health::*;
pub use self::payload_detection::*;
pub use self::transport_failure::*;
pub use self::websocket_message::*;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeRouteKind {
    Responses,
    Compact,
    Websocket,
    Standard,
}

pub fn runtime_route_kind_label(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses",
        RuntimeRouteKind::Compact => "compact",
        RuntimeRouteKind::Websocket => "websocket",
        RuntimeRouteKind::Standard => "standard",
    }
}

pub fn runtime_route_kind_from_label(label: &str) -> Option<RuntimeRouteKind> {
    match label {
        "responses" => Some(RuntimeRouteKind::Responses),
        "compact" => Some(RuntimeRouteKind::Compact),
        "websocket" => Some(RuntimeRouteKind::Websocket),
        "standard" => Some(RuntimeRouteKind::Standard),
        _ => None,
    }
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

pub fn is_runtime_compact_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/responses/compact")
}

pub fn runtime_proxy_request_lane(path: &str, websocket: bool) -> RuntimeRouteKind {
    if websocket {
        RuntimeRouteKind::Websocket
    } else if is_runtime_compact_path(path) {
        RuntimeRouteKind::Compact
    } else if is_runtime_responses_path(path) || is_runtime_anthropic_messages_path(path) {
        RuntimeRouteKind::Responses
    } else {
        RuntimeRouteKind::Standard
    }
}

pub fn runtime_proxy_request_is_long_lived(path: &str, websocket: bool) -> bool {
    websocket || is_runtime_responses_path(path) || is_runtime_anthropic_messages_path(path)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProxyLogField<'a> {
    key: &'a str,
    value: Cow<'a, str>,
}

pub fn runtime_proxy_log_field<'a>(
    key: &'a str,
    value: impl Into<Cow<'a, str>>,
) -> RuntimeProxyLogField<'a> {
    RuntimeProxyLogField {
        key,
        value: value.into(),
    }
}

pub fn runtime_proxy_structured_log_message<'a>(
    event: &str,
    fields: impl IntoIterator<Item = RuntimeProxyLogField<'a>>,
) -> String {
    let mut message = runtime_proxy_sanitize_log_fragment(event).into_owned();
    for field in fields {
        if field.key.is_empty() || runtime_proxy_log_key_needs_skip(field.key) {
            continue;
        }
        if !message.is_empty() {
            message.push(' ');
        }
        message.push_str(field.key);
        message.push('=');
        message.push_str(&runtime_proxy_format_log_field_value(&field.value));
    }
    message
}

fn runtime_proxy_log_key_needs_skip(key: &str) -> bool {
    key.bytes()
        .any(|byte| byte == b'=' || byte.is_ascii_whitespace())
}

fn runtime_proxy_format_log_field_value(value: &str) -> String {
    let sanitized = runtime_proxy_sanitize_log_fragment(value);
    if runtime_proxy_log_field_value_needs_quotes(&sanitized) {
        serde_json::to_string(sanitized.as_ref()).unwrap_or_else(|_| "\"\"".to_string())
    } else {
        sanitized.into_owned()
    }
}

fn runtime_proxy_sanitize_log_fragment(value: &str) -> Cow<'_, str> {
    if value.bytes().any(|byte| matches!(byte, b'\r' | b'\n')) {
        Cow::Owned(value.replace(['\r', '\n'], " "))
    } else {
        Cow::Borrowed(value)
    }
}

fn runtime_proxy_log_field_value_needs_quotes(value: &str) -> bool {
    value.is_empty()
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
        || value.contains('"')
        || value.contains('\\')
}

fn runtime_proxy_skip_log_whitespace(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_proxy_skip_log_field_value(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    if index >= bytes.len() {
        return index;
    }
    if bytes[index] == b'"' {
        index += 1;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if escaped {
                escaped = false;
                index += 1;
                continue;
            }
            match byte {
                b'\\' => {
                    escaped = true;
                    index += 1;
                }
                b'"' => {
                    index += 1;
                    break;
                }
                _ => index += 1,
            }
        }
        return index;
    }
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_proxy_parse_log_field_value(raw_value: &str) -> String {
    if raw_value.starts_with('"') {
        serde_json::from_str::<String>(raw_value)
            .unwrap_or_else(|_| raw_value.trim_matches('"').to_string())
    } else {
        raw_value.trim_matches('"').to_string()
    }
}

pub fn runtime_proxy_log_fields(message: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let bytes = message.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        index = runtime_proxy_skip_log_whitespace(message, index);
        if index >= bytes.len() {
            break;
        }

        let key_start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            continue;
        }
        let key = &message[key_start..index];
        index += 1;
        let value_start = index;
        index = runtime_proxy_skip_log_field_value(message, index);
        let raw_value = &message[value_start..index];
        fields.insert(
            key.to_string(),
            runtime_proxy_parse_log_field_value(raw_value),
        );
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_prodex_openai_mount_paths() {
        assert_eq!(
            runtime_proxy_normalize_openai_path("/backend-api/prodex/responses?x=1").as_ref(),
            "/backend-api/codex/responses?x=1"
        );
        assert_eq!(
            runtime_proxy_normalize_openai_path("/backend-api/prodex/v1/responses").as_ref(),
            "/backend-api/codex/responses"
        );
        assert_eq!(
            runtime_proxy_normalize_openai_path("/backend-api/codex/responses").as_ref(),
            "/backend-api/codex/responses"
        );
    }

    #[test]
    fn classifies_runtime_proxy_lanes_without_transport_side_effects() {
        assert_eq!(
            runtime_proxy_request_lane("/backend-api/codex/responses", false),
            RuntimeRouteKind::Responses
        );
        assert_eq!(
            runtime_proxy_request_lane("/backend-api/codex/responses/compact", false),
            RuntimeRouteKind::Compact
        );
        assert_eq!(
            runtime_proxy_request_lane("/backend-api/codex/realtime", true),
            RuntimeRouteKind::Websocket
        );
        assert_eq!(
            runtime_proxy_request_lane("/dashboard", false),
            RuntimeRouteKind::Standard
        );
    }

    #[test]
    fn extracts_affinity_request_markers_from_json() {
        let body = br#"{
            "previous_response_id": " resp_123 ",
            "prompt_cache_key": " cache-a "
        }"#;
        let value = serde_json::from_slice::<serde_json::Value>(body).unwrap();
        assert_eq!(
            runtime_request_previous_response_id_from_bytes(body).as_deref(),
            Some("resp_123")
        );
        assert_eq!(
            runtime_request_prompt_cache_key_from_value(&value).as_deref(),
            Some("cache-a")
        );
    }

    #[test]
    fn detects_internal_interactive_origin_case_insensitively() {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/messages".to_string(),
            headers: vec![(
                "x-prodex-internal-request-origin".to_string(),
                " Anthropic_Messages ".to_string(),
            )],
            body: Vec::new(),
        };

        assert!(runtime_proxy_request_prefers_interactive_inflight_wait(
            &request
        ));
        assert!(runtime_proxy_request_prefers_inflight_wait(&request));
    }

    #[test]
    fn structured_log_round_trips_quoted_values() {
        let message = runtime_proxy_structured_log_message(
            "event\nname",
            [
                runtime_proxy_log_field("request", "7"),
                runtime_proxy_log_field("bad key", "skip"),
                runtime_proxy_log_field("profile", "alpha beta"),
                runtime_proxy_log_field("error", "line\rbreak"),
            ],
        );

        assert_eq!(
            message,
            "event name request=7 profile=\"alpha beta\" error=\"line break\""
        );

        let fields = runtime_proxy_log_fields(&message);
        assert_eq!(fields.get("request").map(String::as_str), Some("7"));
        assert_eq!(
            fields.get("profile").map(String::as_str),
            Some("alpha beta")
        );
        assert_eq!(fields.get("error").map(String::as_str), Some("line break"));
        assert!(!fields.contains_key("bad key"));
    }
}
