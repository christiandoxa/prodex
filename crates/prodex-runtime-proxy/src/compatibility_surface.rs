use crate::{
    RuntimeProxyRequest, is_runtime_chat_completions_path, is_runtime_compact_path,
    is_runtime_responses_path, runtime_proxy_request_header_value, runtime_proxy_request_origin,
    runtime_request_previous_response_id, runtime_request_session_id, runtime_request_turn_state,
};

#[cfg(any(not(feature = "mojo"), test))]
#[path = "compatibility_surface/rust_oracle.rs"]
mod rust_oracle;

#[cfg(feature = "mojo")]
const COMPAT_TOOL_TOOLS: i64 = 1;
#[cfg(feature = "mojo")]
const COMPAT_TOOL_WEB: i64 = 2;
#[cfg(feature = "mojo")]
const COMPAT_TOOL_MCP: i64 = 4;
#[cfg(feature = "mojo")]
const COMPAT_TOOL_COMPUTER: i64 = 8;
#[cfg(feature = "mojo")]
const COMPAT_TOOL_SHELL: i64 = 16;
#[cfg(feature = "mojo")]
const COMPAT_TOOL_APPROVAL: i64 = 32;
#[cfg(feature = "mojo")]
const COMPAT_CONTINUATION_PREVIOUS_RESPONSE: i64 = 1;
#[cfg(feature = "mojo")]
const COMPAT_CONTINUATION_TURN_STATE: i64 = 2;
#[cfg(feature = "mojo")]
const COMPAT_CONTINUATION_SESSION: i64 = 4;
#[cfg(feature = "mojo")]
const COMPAT_WARNING_UNKNOWN_CLIENT: i64 = 1;
#[cfg(feature = "mojo")]
const COMPAT_WARNING_WEBSOCKET_PREVIOUS_RESPONSE_WITHOUT_TURN_STATE: i64 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRequestCompatibilitySurface {
    pub stage: &'static str,
    pub family: &'static str,
    pub client: &'static str,
    pub route: &'static str,
    pub transport: &'static str,
    pub stream: &'static str,
    pub tool_surface: String,
    pub continuation: String,
    pub request_origin: &'static str,
    pub approval: bool,
    pub user_agent: String,
    pub warnings: Vec<&'static str>,
}

impl RuntimeRequestCompatibilitySurface {
    #[cfg(any(not(feature = "mojo"), test))]
    fn new(stage: &'static str, route: &'static str, transport: &'static str) -> Self {
        Self {
            stage,
            family: "unknown",
            client: "unknown",
            route,
            transport,
            stream: "unary",
            tool_surface: "none".to_string(),
            continuation: "none".to_string(),
            request_origin: "external",
            approval: false,
            user_agent: "-".to_string(),
            warnings: Vec::new(),
        }
    }
}

pub fn runtime_detect_request_compatibility_surface(
    request: &RuntimeProxyRequest,
    stage: &'static str,
    transport: &'static str,
) -> RuntimeRequestCompatibilitySurface {
    #[cfg(feature = "mojo")]
    {
        runtime_detect_request_compatibility_surface_mojo(request, stage, transport)
    }

    #[cfg(not(feature = "mojo"))]
    {
        rust_oracle::detect(request, stage, transport)
    }
}

#[cfg(feature = "mojo")]
fn runtime_detect_request_compatibility_surface_mojo(
    request: &RuntimeProxyRequest,
    stage: &'static str,
    transport: &'static str,
) -> RuntimeRequestCompatibilitySurface {
    let route = route_label(request);
    let value = serde_json::from_slice::<serde_json::Value>(&request.body).ok();
    let tools = value
        .as_ref()
        .and_then(|value| value.get("tools"))
        .and_then(serde_json::Value::as_array);
    let tool_labels = tools
        .into_iter()
        .flatten()
        .filter_map(|tool| {
            tool.get("type")
                .or_else(|| tool.get("name"))
                .and_then(serde_json::Value::as_str)
        })
        .collect::<Vec<_>>();
    let user_agent = runtime_proxy_request_header_value(&request.headers, "user-agent");
    let subagent_header =
        runtime_proxy_request_header_value(&request.headers, "x-openai-subagent").is_some();
    let codex_headers = runtime_proxy_request_header_value(&request.headers, "x-codex-turn-state")
        .is_some()
        || subagent_header
        || runtime_proxy_request_header_value(&request.headers, "session-id").is_some();
    let explicit_stream = value
        .as_ref()
        .and_then(|value| value.get("stream"))
        .and_then(serde_json::Value::as_bool);
    let plan = prodex_mojo_core::runtime::compatibility_surface_plan(
        [
            route_kind_tag(route),
            i64::from(transport == "websocket"),
            i64::from(codex_headers),
            i64::from(subagent_header),
            i64::from(runtime_proxy_request_origin(&request.headers).is_some()),
            explicit_stream.map_or(-1, i64::from),
            i64::from(runtime_request_previous_response_id(request).is_some()),
            i64::from(runtime_request_turn_state(request).is_some()),
            i64::from(runtime_request_session_id(request).is_some()),
            i64::from(tools.is_some_and(|tools| !tools.is_empty())),
        ],
        user_agent.unwrap_or_default(),
        &tool_labels,
    )
    .expect("Mojo compatibility-surface planning returned an invalid result");

    RuntimeRequestCompatibilitySurface {
        stage,
        family: family_label(plan[0]),
        client: client_label(plan[1]),
        route,
        transport,
        stream: stream_label(plan[2]),
        tool_surface: tool_surface_label(plan[3]),
        continuation: continuation_label(plan[4]),
        request_origin: request_origin_label(plan[6]),
        approval: plan[3] & COMPAT_TOOL_APPROVAL != 0,
        user_agent: user_agent
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("-")
            .to_string(),
        warnings: warning_labels(plan[5]),
    }
}

#[cfg(feature = "mojo")]
fn route_kind_tag(route: &str) -> i64 {
    match route {
        "responses" => 0,
        "compact" => 1,
        "chat_completions" => 2,
        _ => 3,
    }
}

#[cfg(feature = "mojo")]
fn family_label(value: i64) -> &'static str {
    match value {
        1 => "codex",
        2 => "openai_compatible",
        _ => "unknown",
    }
}

#[cfg(feature = "mojo")]
fn client_label(value: i64) -> &'static str {
    match value {
        1 => "codex_subagent",
        2 => "codex_cli",
        3 => "chat_completions_client",
        4 => "responses_client",
        _ => "unknown",
    }
}

#[cfg(feature = "mojo")]
fn stream_label(value: i64) -> &'static str {
    if value == 1 { "streaming" } else { "unary" }
}

#[cfg(feature = "mojo")]
fn tool_surface_label(flags: i64) -> String {
    let mut labels = Vec::new();
    for (flag, label) in [
        (COMPAT_TOOL_APPROVAL, "approval"),
        (COMPAT_TOOL_COMPUTER, "computer"),
        (COMPAT_TOOL_MCP, "mcp"),
        (COMPAT_TOOL_SHELL, "shell"),
        (COMPAT_TOOL_TOOLS, "tools"),
        (COMPAT_TOOL_WEB, "web"),
    ] {
        if flags & flag != 0 {
            labels.push(label);
        }
    }
    if labels.is_empty() {
        "none".to_string()
    } else {
        labels.join("+")
    }
}

#[cfg(feature = "mojo")]
fn continuation_label(flags: i64) -> String {
    let mut labels = Vec::new();
    for (flag, label) in [
        (COMPAT_CONTINUATION_PREVIOUS_RESPONSE, "previous_response"),
        (COMPAT_CONTINUATION_SESSION, "session"),
        (COMPAT_CONTINUATION_TURN_STATE, "turn_state"),
    ] {
        if flags & flag != 0 {
            labels.push(label);
        }
    }
    if labels.is_empty() {
        "none".to_string()
    } else {
        labels.join("+")
    }
}

#[cfg(feature = "mojo")]
fn request_origin_label(value: i64) -> &'static str {
    if value == 1 { "internal" } else { "external" }
}

#[cfg(feature = "mojo")]
fn warning_labels(flags: i64) -> Vec<&'static str> {
    let mut warnings = Vec::new();
    if flags & COMPAT_WARNING_UNKNOWN_CLIENT != 0 {
        warnings.push("unknown_client_family");
    }
    if flags & COMPAT_WARNING_WEBSOCKET_PREVIOUS_RESPONSE_WITHOUT_TURN_STATE != 0 {
        warnings.push("websocket_previous_response_without_turn_state");
    }
    warnings
}

fn route_label(request: &RuntimeProxyRequest) -> &'static str {
    if is_runtime_compact_path(&request.path_and_query) {
        "compact"
    } else if is_runtime_responses_path(&request.path_and_query) {
        "responses"
    } else if is_runtime_chat_completions_path(&request.path_and_query) {
        "chat_completions"
    } else {
        "standard"
    }
}

pub fn runtime_detect_websocket_message_compatibility_surface(
    handshake_request: &RuntimeProxyRequest,
    request_text: &str,
) -> RuntimeRequestCompatibilitySurface {
    let request = RuntimeProxyRequest {
        method: handshake_request.method.clone(),
        path_and_query: handshake_request.path_and_query.clone(),
        headers: handshake_request.headers.clone(),
        body: request_text.as_bytes().to_vec(),
    };
    runtime_detect_request_compatibility_surface(&request, "message", "websocket")
}

pub fn runtime_capability_log_safe_value(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/' | ':') {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }
    if sanitized.is_empty() {
        "-".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_request_is_unary_and_tracks_session() {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses/compact".to_string(),
            headers: vec![("session-id".to_string(), "session-123".to_string())],
            body: br#"{"input":[]}"#.to_vec(),
        };
        let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");
        assert_eq!(surface.route, "compact");
        assert_eq!(surface.stream, "unary");
        assert!(surface.continuation.contains("session"));
    }

    #[test]
    fn responses_tools_are_classified_without_provider_specific_logic() {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: vec![("user-agent".to_string(), "codex-cli".to_string())],
            body: br#"{"tools":[{"type":"web_search"}]}"#.to_vec(),
        };
        let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");
        assert_eq!(surface.family, "codex");
        assert!(surface.tool_surface.contains("web"));
    }

    #[cfg(feature = "mojo")]
    #[test]
    fn mojo_compatibility_surface_matches_rust_oracle() {
        let fixtures = [
            RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/backend-api/codex/responses".to_string(),
                headers: vec![
                    ("user-agent".to_string(), "Codex CLI".to_string()),
                    ("session-id".to_string(), "s1".to_string()),
                ],
                body: br#"{"tools":[{"type":"web_search"},{"type":"mcp_call"},{"name":"approval_tool"}]}"#.to_vec(),
            },
            RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/v1/chat/completions".to_string(),
                headers: vec![],
                body: br#"{"stream":false,"tools":[{"type":"computer_use"}]}"#.to_vec(),
            },
            RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/unknown".to_string(),
                headers: vec![],
                body: br#"{"input":[]}"#.to_vec(),
            },
            RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/backend-api/codex/responses".to_string(),
                headers: vec![("x-openai-subagent".to_string(), "1".to_string())],
                body: br#"{"stream":true,"previous_response_id":"resp_1"}"#.to_vec(),
            },
        ];
        for request in &fixtures {
            for transport in ["http", "websocket"] {
                assert_eq!(
                    runtime_detect_request_compatibility_surface(request, "request", transport),
                    rust_oracle::detect(request, "request", transport),
                    "path={} transport={transport}",
                    request.path_and_query
                );
            }
        }
    }
}
