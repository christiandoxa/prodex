//! Runtime proxy boundary primitives.
//! Most modules are side-effect-free; the bounded WebSocket TCP/DNS executor
//! keeps binary wiring thin while owning runtime state and persistence.

use prodex_mojo_core::rich::ascii_casefold_equal_exact;
use std::{borrow::Cow, fmt};

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
mod compatibility_surface;
mod error_policy;
mod failure_response;
mod health;
mod lineage;
mod log_event;
mod payload_detection;
mod previous_response_log;
mod previous_response_orchestration;
mod quota;
mod response_forwarding;
mod route_affinity_log;
mod route_decision_trace;
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
pub use self::compatibility_surface::*;
pub use self::error_policy::*;
pub use self::failure_response::*;
pub use self::health::*;
pub use self::lineage::*;
pub use self::log_event::*;
pub use self::payload_detection::*;
pub use self::previous_response_log::*;
pub use self::previous_response_orchestration::*;
pub use self::quota::*;
pub use self::response_forwarding::*;
pub use self::route_affinity_log::*;
pub use self::route_decision_trace::*;
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
pub const LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX: &str = "/backend-api/prodex/v";
pub const PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER: &str = "X-Prodex-Internal-Request-Origin";
pub const RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS: u64 = if cfg!(test) { 80 } else { 750 };
pub const RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS: u64 = if cfg!(test) { 80 } else { 750 };
pub const RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS: u64 = if cfg!(test) { 25 } else { 200 };
pub const RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS: u64 =
    if cfg!(test) { 25 } else { 200 };
pub const RUNTIME_PROXY_INTERACTIVE_WAIT_MULTIPLIER: u64 = 2;

#[derive(Clone, PartialEq, Eq)]
pub struct RuntimeProxyRequest {
    pub method: String,
    pub path_and_query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl fmt::Debug for RuntimeProxyRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeProxyRequest")
            .field("method", &self.method)
            .field("path_and_query", &"<redacted>")
            .field("header_count", &self.headers.len())
            .field("headers", &"<redacted>")
            .field("body_len", &self.body.len())
            .field("body", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
struct RuntimeRequestSemanticPlan {
    previous_response_id: Option<String>,
    session_id: Option<String>,
    prompt_cache_key: Option<String>,
    turn_state: Option<String>,
    turn_id: Option<String>,
    thread_id: Option<String>,
    window_id: Option<String>,
    requires_previous_response_affinity: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    reconstructable_full_history: bool,
}

fn runtime_request_json_node_kind_and_text(
    value: &serde_json::Value,
) -> (prodex_mojo_core::json::JsonKind, &str) {
    use prodex_mojo_core::json::JsonKind;
    match value {
        serde_json::Value::Null => (JsonKind::Null, ""),
        serde_json::Value::Bool(false) => (JsonKind::False, ""),
        serde_json::Value::Bool(true) => (JsonKind::True, ""),
        serde_json::Value::Number(_) => (JsonKind::Number, ""),
        serde_json::Value::String(value) => (JsonKind::String, value.as_str()),
        serde_json::Value::Array(_) => (JsonKind::Array, ""),
        serde_json::Value::Object(_) => (JsonKind::Object, ""),
    }
}

fn runtime_request_link_json_child(
    nodes: &mut [prodex_mojo_core::json::JsonNode<'_>],
    parent: usize,
    previous: Option<usize>,
    child: usize,
) {
    if let Some(previous) = previous {
        nodes[previous].next_sibling = Some(child);
    } else {
        nodes[parent].first_child = Some(child);
    }
}

fn runtime_request_push_json_node<'a>(
    nodes: &mut Vec<prodex_mojo_core::json::JsonNode<'a>>,
    value: &'a serde_json::Value,
    key: &'a str,
    parent: Option<usize>,
) -> usize {
    use prodex_mojo_core::json::JsonNode;

    let (kind, text) = runtime_request_json_node_kind_and_text(value);
    let index = nodes.len();
    nodes.push(JsonNode {
        kind,
        first_child: None,
        next_sibling: None,
        parent,
        key,
        text,
        raw_start: 0,
        raw_length: 0,
    });

    let mut previous = None;
    match value {
        serde_json::Value::Array(values) => {
            for child in values {
                let child_index = runtime_request_push_json_node(nodes, child, "", Some(index));
                runtime_request_link_json_child(nodes, index, previous, child_index);
                previous = Some(child_index);
            }
        }
        serde_json::Value::Object(map) => {
            for (child_key, child) in map {
                let child_index =
                    runtime_request_push_json_node(nodes, child, child_key.as_str(), Some(index));
                runtime_request_link_json_child(nodes, index, previous, child_index);
                previous = Some(child_index);
            }
        }
        _ => {}
    }
    index
}

fn runtime_request_json_nodes<'a>(
    value: &'a serde_json::Value,
) -> Vec<prodex_mojo_core::json::JsonNode<'a>> {
    let mut nodes = Vec::new();
    runtime_request_push_json_node(&mut nodes, value, "", None);
    nodes
}

fn runtime_request_semantic_plan(value: &serde_json::Value) -> RuntimeRequestSemanticPlan {
    let nodes = runtime_request_json_nodes(value);
    let plan = prodex_mojo_core::json::runtime_proxy_request_metadata(&nodes, "")
        .expect("Mojo runtime request metadata returned invalid output");
    let string_at = |index: Option<usize>| index.map(|index| nodes[index].text.trim().to_string());
    RuntimeRequestSemanticPlan {
        previous_response_id: string_at(plan.previous_response_id),
        session_id: string_at(plan.session_id),
        prompt_cache_key: string_at(plan.prompt_cache_key),
        turn_state: string_at(plan.turn_state),
        turn_id: string_at(plan.turn_id),
        thread_id: string_at(plan.thread_id),
        window_id: string_at(plan.window_id),
        requires_previous_response_affinity: plan.requires_previous_response_affinity,
        fresh_fallback_shape: plan.fresh_fallback_shape.map(|shape| match shape {
            0 => RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly,
            1 => RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
            2 => RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay,
            3 => RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly,
            _ => unreachable!("validated Mojo previous-response fallback shape"),
        }),
        reconstructable_full_history: plan.reconstructable_full_history,
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
    let plan = prodex_mojo_core::rich::runtime_proxy_path_plan(path, false)
        .expect("Mojo runtime proxy path planning returned invalid output");
    let start = plan.mount_suffix_start?;
    path.get(start..plan.path_end)
}

pub fn runtime_proxy_normalize_openai_path(path_and_query: &str) -> Cow<'_, str> {
    let plan = prodex_mojo_core::rich::runtime_proxy_path_plan(path_and_query, false)
        .expect("Mojo runtime proxy path planning returned invalid output");
    let Some(suffix_start) = plan.mount_suffix_start else {
        return Cow::Borrowed(path_and_query);
    };

    let mut normalized =
        String::with_capacity(path_and_query.len() + RUNTIME_PROXY_OPENAI_UPSTREAM_PATH.len());
    normalized.push_str(RUNTIME_PROXY_OPENAI_UPSTREAM_PATH);
    normalized.push_str(
        path_and_query
            .get(suffix_start..plan.path_end)
            .expect("validated Mojo runtime path suffix"),
    );
    if let Some(query_mark) = plan.query_mark {
        normalized.push_str(
            path_and_query
                .get(query_mark..)
                .expect("validated Mojo runtime query boundary"),
        );
    }
    Cow::Owned(normalized)
}

pub fn is_runtime_responses_path(path_and_query: &str) -> bool {
    prodex_mojo_core::rich::runtime_proxy_path_plan(path_and_query, false)
        .expect("Mojo runtime proxy path planning returned invalid output")
        .responses
}

pub fn is_runtime_chat_completions_path(path_and_query: &str) -> bool {
    prodex_mojo_core::rich::runtime_proxy_path_plan(path_and_query, false)
        .expect("Mojo runtime proxy path planning returned invalid output")
        .chat_completions
}

pub fn is_runtime_compact_path(path_and_query: &str) -> bool {
    prodex_mojo_core::rich::runtime_proxy_path_plan(path_and_query, false)
        .expect("Mojo runtime proxy path planning returned invalid output")
        .compact
}

pub fn runtime_proxy_request_lane(path: &str, websocket: bool) -> RuntimeRouteKind {
    match prodex_mojo_core::rich::runtime_proxy_path_plan(path, websocket)
        .expect("Mojo runtime proxy path planning returned invalid output")
        .route_kind
    {
        0 => RuntimeRouteKind::Responses,
        1 => RuntimeRouteKind::Compact,
        2 => RuntimeRouteKind::Websocket,
        3 => RuntimeRouteKind::Standard,
        _ => unreachable!("validated Mojo runtime route kind"),
    }
}

pub fn runtime_proxy_request_is_long_lived(path: &str, websocket: bool) -> bool {
    prodex_mojo_core::rich::runtime_proxy_path_plan(path, websocket)
        .expect("Mojo runtime proxy path planning returned invalid output")
        .long_lived
}

pub fn runtime_proxy_request_prefers_inflight_wait(request: &RuntimeProxyRequest) -> bool {
    ascii_casefold_equal_exact(&request.method, "GET")
        .expect("Mojo request-method comparison failed")
        || prodex_mojo_core::rich::runtime_proxy_path_plan(&request.path_and_query, false)
            .expect("Mojo runtime proxy path planning returned invalid output")
            .long_lived
}

pub fn runtime_proxy_interactive_wait_budget_ms(_path: &str, base_budget_ms: u64) -> u64 {
    base_budget_ms
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

pub fn is_runtime_realtime_call_path(path_and_query: &str) -> bool {
    prodex_mojo_core::rich::runtime_proxy_path_plan(path_and_query, false)
        .expect("Mojo runtime proxy path planning returned invalid output")
        .realtime_call
}

pub fn is_runtime_realtime_websocket_path(path_and_query: &str) -> bool {
    prodex_mojo_core::rich::runtime_proxy_path_plan(path_and_query, false)
        .expect("Mojo runtime proxy path planning returned invalid output")
        .realtime_websocket
}

pub fn runtime_proxy_request_header_value<'a>(
    headers: &'a [(String, String)],
    name: &str,
) -> Option<&'a str> {
    headers
        .iter()
        .find_map(|(header_name, value)| {
            ascii_casefold_equal_exact(header_name, name)
                .expect("Mojo request header-name comparison failed")
                .then_some(value.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub fn runtime_proxy_request_origin(headers: &[(String, String)]) -> Option<&str> {
    runtime_proxy_request_header_value(headers, PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER)
}

pub fn runtime_request_previous_response_id(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_request_previous_response_id_from_bytes(&request.body)
}

pub fn runtime_request_prompt_cache_key(request: &RuntimeProxyRequest) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(&request.body).ok()?;
    runtime_request_semantic_plan(&value).prompt_cache_key
}

pub fn runtime_request_previous_response_id_from_bytes(body: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    runtime_request_semantic_plan(&value).previous_response_id
}

pub fn runtime_request_previous_response_id_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    runtime_request_semantic_plan(value).previous_response_id
}

pub fn runtime_request_prompt_cache_key_from_value(value: &serde_json::Value) -> Option<String> {
    runtime_request_semantic_plan(value).prompt_cache_key
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
    let plan = runtime_request_semantic_plan(&value);
    RuntimeWebsocketRequestMetadata {
        previous_response_id: plan.previous_response_id,
        session_id: plan.session_id,
        prompt_cache_key: plan.prompt_cache_key,
        turn_state: plan.turn_state,
        requires_previous_response_affinity: plan.requires_previous_response_affinity,
        previous_response_fresh_fallback_shape: plan.fresh_fallback_shape,
    }
}

pub fn runtime_request_previous_response_id_from_text(request_text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(request_text).ok()?;
    runtime_request_semantic_plan(&value).previous_response_id
}

pub fn runtime_request_value_requires_previous_response_affinity(
    value: &serde_json::Value,
) -> bool {
    runtime_request_semantic_plan(value).requires_previous_response_affinity
}

pub fn runtime_request_value_previous_response_fresh_fallback_shape(
    value: &serde_json::Value,
) -> Option<RuntimePreviousResponseFreshFallbackShape> {
    runtime_request_semantic_plan(value).fresh_fallback_shape
}

pub fn runtime_request_previous_response_fresh_fallback_shape(
    request: &RuntimeProxyRequest,
) -> Option<RuntimePreviousResponseFreshFallbackShape> {
    let body_shape = serde_json::from_slice::<serde_json::Value>(&request.body)
        .ok()
        .and_then(|value| runtime_request_semantic_plan(&value).fresh_fallback_shape);
    runtime_previous_response_fresh_fallback_shape_with_session(
        body_shape,
        runtime_request_explicit_session_id(request).is_some()
            || runtime_request_session_id_from_turn_metadata(request).is_some(),
    )
}

pub fn runtime_request_requires_previous_response_affinity(request: &RuntimeProxyRequest) -> bool {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .map(|value| runtime_request_semantic_plan(&value).requires_previous_response_affinity)
        .unwrap_or(false)
}

pub fn runtime_request_turn_state(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_proxy_request_header_value(&request.headers, "x-codex-turn-state").map(str::to_string)
}

pub fn runtime_request_turn_state_from_value(value: &serde_json::Value) -> Option<String> {
    runtime_request_semantic_plan(value).turn_state
}

pub fn runtime_request_session_id_from_value(value: &serde_json::Value) -> Option<String> {
    runtime_request_semantic_plan(value).session_id
}

pub fn runtime_request_session_id_from_turn_metadata(
    request: &RuntimeProxyRequest,
) -> Option<String> {
    request
        .headers
        .iter()
        .find_map(|(name, value)| {
            ascii_casefold_equal_exact(name, "x-codex-turn-metadata")
                .expect("Mojo turn-metadata header comparison failed")
                .then_some(value.as_str())
        })
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| runtime_request_semantic_plan(&value).session_id)
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

pub fn runtime_request_turn_id_from_value(value: &serde_json::Value) -> Option<String> {
    runtime_request_semantic_plan(value).turn_id
}

pub fn runtime_request_turn_id(request: &RuntimeProxyRequest) -> Option<String> {
    request
        .headers
        .iter()
        .find_map(|(name, value)| {
            ascii_casefold_equal_exact(name, "x-codex-turn-metadata")
                .expect("Mojo turn-metadata header comparison failed")
                .then_some(value.as_str())
        })
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| runtime_request_semantic_plan(&value).turn_id)
        .or_else(|| {
            serde_json::from_slice::<serde_json::Value>(&request.body)
                .ok()
                .and_then(|value| runtime_request_semantic_plan(&value).turn_id)
        })
}

pub fn runtime_request_thread_id(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_proxy_request_header_value(&request.headers, "thread-id")
        .map(str::to_string)
        .or_else(|| {
            request
                .headers
                .iter()
                .find_map(|(name, value)| {
                    ascii_casefold_equal_exact(name, "x-codex-turn-metadata")
                        .expect("Mojo turn-metadata header comparison failed")
                        .then_some(value.as_str())
                })
                .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
                .and_then(|value| runtime_request_semantic_plan(&value).thread_id)
        })
}

pub fn runtime_request_compaction_generation(request: &RuntimeProxyRequest) -> Option<u64> {
    let window_id = request
        .headers
        .iter()
        .find_map(|(name, value)| {
            ascii_casefold_equal_exact(name, "x-codex-turn-metadata")
                .expect("Mojo turn-metadata header comparison failed")
                .then_some(value.as_str())
        })
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| runtime_request_semantic_plan(&value).window_id)
        .or_else(|| {
            serde_json::from_slice::<serde_json::Value>(&request.body)
                .ok()
                .and_then(|value| runtime_request_semantic_plan(&value).window_id)
        })?;
    window_id.rsplit_once(':')?.1.parse().ok()
}

pub fn runtime_request_without_previous_response_id(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeProxyRequest> {
    let mut value = serde_json::from_slice::<serde_json::Value>(&request.body).ok()?;
    if runtime_request_value_requires_previous_response_affinity(&value)
        || runtime_request_session_id(request).is_some()
    {
        return None;
    }
    remove_previous_response_id(&mut value)?;
    let mut request = request.clone();
    request.body = serde_json::to_vec(&value).ok()?;
    Some(request)
}

pub fn runtime_request_full_history_without_previous_response_id(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeProxyRequest> {
    runtime_request_session_id(request)?;
    let mut value = serde_json::from_slice::<serde_json::Value>(&request.body).ok()?;
    if !runtime_request_semantic_plan(&value).reconstructable_full_history {
        return None;
    }
    remove_previous_response_id(&mut value)?;
    let mut request = request.clone();
    request.body = serde_json::to_vec(&value).ok()?;
    Some(request)
}

pub fn runtime_request_text_without_previous_response_id(request_text: &str) -> Option<String> {
    let mut value = serde_json::from_str::<serde_json::Value>(request_text).ok()?;
    if runtime_request_value_requires_previous_response_affinity(&value)
        || runtime_request_session_id_from_value(&value).is_some()
    {
        return None;
    }
    remove_previous_response_id(&mut value)?;
    Some(value.to_string())
}

fn remove_previous_response_id(value: &mut serde_json::Value) -> Option<()> {
    let object = value.as_object_mut()?;
    object.remove("previous_response_id")?;
    Some(())
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
