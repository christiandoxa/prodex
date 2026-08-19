use runtime_anthropic_crate::runtime_proxy_anthropic_unversioned_tool_type;
use runtime_proxy_crate::{
    PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES, RuntimeProxyRequest,
    is_runtime_anthropic_messages_path, is_runtime_chat_completions_path, is_runtime_compact_path,
    is_runtime_responses_path, runtime_proxy_request_header_value, runtime_proxy_request_origin,
    runtime_request_previous_response_id, runtime_request_session_id, runtime_request_turn_state,
};
use std::collections::BTreeSet;

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

fn runtime_capability_request_json(request: &RuntimeProxyRequest) -> Option<serde_json::Value> {
    serde_json::from_slice::<serde_json::Value>(&request.body).ok()
}

fn runtime_capability_labels_from_flags(
    flags: &BTreeSet<&'static str>,
    none_label: &'static str,
) -> String {
    if flags.is_empty() {
        none_label.to_string()
    } else {
        flags.iter().copied().collect::<Vec<_>>().join("+")
    }
}

fn runtime_capability_request_origin_label(request: &RuntimeProxyRequest) -> &'static str {
    match runtime_proxy_request_origin(&request.headers) {
        Some(PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES) => "anthropic_messages",
        Some(_) => "internal",
        None => "external",
    }
}

fn runtime_capability_client_labels(
    request: &RuntimeProxyRequest,
    route: &'static str,
) -> (&'static str, &'static str) {
    let user_agent = runtime_proxy_request_header_value(&request.headers, "user-agent")
        .map(|value| value.to_ascii_lowercase());
    let has_claude_session =
        runtime_proxy_request_header_value(&request.headers, "x-claude-code-session-id").is_some();
    let has_codex_turn_state =
        runtime_proxy_request_header_value(&request.headers, "x-codex-turn-state").is_some();
    let has_codex_subagent =
        runtime_proxy_request_header_value(&request.headers, "x-openai-subagent").is_some();

    if has_claude_session
        || runtime_capability_agent_is(&user_agent, &["claude-code", "claude-cli"])
    {
        return ("claude_code", "claude_code");
    }

    if is_runtime_anthropic_messages_path(&request.path_and_query) {
        return runtime_capability_anthropic_client_labels(request, &user_agent);
    }

    if has_codex_turn_state
        || has_codex_subagent
        || route == "websocket"
        || runtime_capability_agent_is(&user_agent, &["codex"])
    {
        return if has_codex_subagent {
            ("codex", "codex_subagent")
        } else {
            ("codex", "codex_cli")
        };
    }

    if is_runtime_chat_completions_path(&request.path_and_query) {
        ("openai_compatible", "chat_completions_client")
    } else if is_runtime_responses_path(&request.path_and_query)
        || is_runtime_compact_path(&request.path_and_query)
    {
        ("openai_compatible", "responses_client")
    } else {
        ("unknown", "unknown")
    }
}

fn runtime_capability_agent_is(user_agent: &Option<String>, markers: &[&str]) -> bool {
    user_agent
        .as_deref()
        .is_some_and(|agent| markers.iter().any(|marker| agent.contains(marker)))
}

fn runtime_capability_anthropic_client_labels(
    request: &RuntimeProxyRequest,
    user_agent: &Option<String>,
) -> (&'static str, &'static str) {
    if runtime_capability_request_origin_label(request) == "anthropic_messages" {
        ("claude_code", "claude_code")
    } else if runtime_capability_agent_is(user_agent, &["anthropic"]) {
        ("anthropic", "anthropic_sdk")
    } else {
        ("anthropic", "anthropic_compatible")
    }
}

fn runtime_capability_collect_tool_surface_from_anthropic(
    value: &serde_json::Value,
    flags: &mut BTreeSet<&'static str>,
    warnings: &mut Vec<&'static str>,
) {
    let mut has_mcp_servers = value
        .get("mcp_servers")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| !items.is_empty());
    let mut has_mcp_toolset = false;
    let mut saw_tools = false;

    if let Some(tools) = value.get("tools").and_then(serde_json::Value::as_array) {
        saw_tools = !tools.is_empty();
        for tool in tools {
            has_mcp_toolset |= runtime_capability_collect_anthropic_tool(tool, flags);
        }
    }

    if runtime_capability_has_anthropic_approval(value) {
        flags.insert("approval");
    }

    if has_mcp_servers && !has_mcp_toolset {
        warnings.push("anthropic_mcp_servers_without_toolset");
    }
    if saw_tools && flags.is_empty() {
        warnings.push("anthropic_unknown_tool_surface");
    }
    if has_mcp_toolset {
        has_mcp_servers = true;
    }
    if has_mcp_servers {
        flags.insert("mcp");
    }
}

fn runtime_capability_collect_anthropic_tool(
    tool: &serde_json::Value,
    flags: &mut BTreeSet<&'static str>,
) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let tool_name = tool
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    match runtime_proxy_anthropic_unversioned_tool_type(tool_type).as_str() {
        "mcp_toolset" => {
            flags.insert("mcp");
            true
        }
        "bash" => {
            flags.insert("shell");
            false
        }
        "computer" => {
            flags.insert("computer");
            false
        }
        "text_editor" => {
            flags.insert("editor");
            false
        }
        "web_search" | "web_fetch" => {
            flags.insert("web");
            false
        }
        _ if tool_name.contains("websearch") || tool_name.contains("web_fetch") => {
            flags.insert("web");
            false
        }
        _ if !tool_name.is_empty() => {
            flags.insert("generic_tool");
            false
        }
        _ => false,
    }
}

fn runtime_capability_has_anthropic_approval(value: &serde_json::Value) -> bool {
    value
        .get("messages")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|message| message.get("content").and_then(serde_json::Value::as_array))
        .flatten()
        .filter_map(|block| block.get("type").and_then(serde_json::Value::as_str))
        .any(|block_type| {
            matches!(
                block_type,
                "mcp_approval_request" | "mcp_approval_response" | "mcp_list_tools"
            )
        })
}

fn runtime_capability_collect_tool_surface_from_responses(
    value: &serde_json::Value,
    flags: &mut BTreeSet<&'static str>,
) {
    if let Some(tools) = value.get("tools").and_then(serde_json::Value::as_array) {
        for tool in tools {
            runtime_capability_collect_responses_tool(tool, flags);
        }
    }
    if let Some(items) = value.get("input").and_then(serde_json::Value::as_array) {
        for item in items {
            runtime_capability_collect_responses_item(item, flags);
        }
    }
}

fn runtime_capability_collect_responses_tool(
    tool: &serde_json::Value,
    flags: &mut BTreeSet<&'static str>,
) {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = tool
        .get("name")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            tool.get("function")
                .and_then(|function| function.get("name"))
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or_default()
        .to_ascii_lowercase();
    match tool_type.as_str() {
        "function" => runtime_capability_collect_responses_named_tool(&name, flags),
        "web_search" | "web_search_preview" | "web_fetch" => {
            flags.insert("web");
        }
        "computer" | "computer_use_preview" => {
            flags.insert("computer");
        }
        "shell" | "code_interpreter" => {
            flags.insert("shell");
        }
        _ if tool_type.starts_with("mcp") => {
            flags.insert("mcp");
        }
        _ if !tool_type.is_empty() || !name.is_empty() => {
            flags.insert("generic_tool");
        }
        _ => {}
    }
}

fn runtime_capability_collect_responses_named_tool(name: &str, flags: &mut BTreeSet<&'static str>) {
    flags.insert(match name {
        "websearch" | "webfetch" => "web",
        "bash" => "shell",
        "computer" => "computer",
        _ => "generic_tool",
    });
}

fn runtime_capability_collect_responses_item(
    item: &serde_json::Value,
    flags: &mut BTreeSet<&'static str>,
) {
    let item_type = item
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let name = item
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if item_type.starts_with("mcp_") {
        flags.insert("mcp");
    }
    if item_type.contains("approval") {
        flags.insert("approval");
    }
    if item_type.ends_with("_call_output") {
        flags.insert("tool_result");
    }
    if item_type == "function_call" {
        runtime_capability_collect_responses_named_tool(&name, flags);
    }
}

fn runtime_capability_continuation_label(request: &RuntimeProxyRequest) -> String {
    let mut flags = BTreeSet::new();
    if runtime_request_previous_response_id(request).is_some() {
        flags.insert("previous_response");
    }
    if runtime_request_turn_state(request).is_some() {
        flags.insert("turn_state");
    }
    if runtime_request_session_id(request).is_some() {
        flags.insert("session");
    }
    runtime_capability_labels_from_flags(&flags, "none")
}

fn runtime_codex_client_version(request: &RuntimeProxyRequest) -> Option<&str> {
    runtime_proxy_request_header_value(&request.headers, "user-agent").and_then(|user_agent| {
        user_agent.split_ascii_whitespace().find_map(|component| {
            match component.split_once('/')? {
                (
                    "codex-cli" | "codex_cli_rs" | "codex-tui" | "codex_exec" | "codex_app_server"
                    | "codex-app-server",
                    version,
                ) => Some(version),
                _ => None,
            }
        })
    })
}

/// Returns whether this request needs the Codex 0.148 previous-response compatibility shim.
pub fn runtime_codex_previous_response_id_regression(request: &RuntimeProxyRequest) -> bool {
    runtime_codex_client_version(request) == Some("0.148.0")
}

/// Returns whether an exact invalid-id failure after a WebSocket reconnect needs replay signaling.
pub fn runtime_codex_previous_response_id_websocket_reconnect_regression(
    request: &RuntimeProxyRequest,
) -> bool {
    matches!(
        runtime_codex_client_version(request),
        Some("0.147.0" | "0.148.0")
    )
}

pub fn runtime_detect_request_compatibility_surface(
    request: &RuntimeProxyRequest,
    stage: &'static str,
    transport: &'static str,
) -> RuntimeRequestCompatibilitySurface {
    let route = runtime_capability_route_label(request);
    let value = runtime_capability_request_json(request);
    let mut surface = RuntimeRequestCompatibilitySurface::new(stage, route, transport);
    let (family, client) = runtime_capability_client_labels(request, route);
    surface.family = family;
    surface.client = client;
    surface.stream = runtime_capability_stream_label(request, value.as_ref(), transport);
    surface.continuation = runtime_capability_continuation_label(request);
    surface.request_origin = runtime_capability_request_origin_label(request);
    surface.user_agent = runtime_proxy_request_header_value(&request.headers, "user-agent")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "-".to_string());

    let tool_flags = runtime_capability_tool_flags(request, value.as_ref(), &mut surface.warnings);

    if surface.family == "unknown" {
        surface.warnings.push("unknown_client_family");
    }
    if transport == "websocket"
        && runtime_request_previous_response_id(request).is_some()
        && runtime_request_turn_state(request).is_none()
    {
        surface
            .warnings
            .push("websocket_previous_response_without_turn_state");
    }

    surface.approval = tool_flags.contains("approval");
    surface.tool_surface = runtime_capability_labels_from_flags(&tool_flags, "none");
    surface
}

fn runtime_capability_route_label(request: &RuntimeProxyRequest) -> &'static str {
    if is_runtime_anthropic_messages_path(&request.path_and_query) {
        "anthropic_messages"
    } else if is_runtime_compact_path(&request.path_and_query) {
        "compact"
    } else if is_runtime_responses_path(&request.path_and_query) {
        "responses"
    } else if is_runtime_chat_completions_path(&request.path_and_query) {
        "chat_completions"
    } else {
        "standard"
    }
}

fn runtime_capability_stream_label(
    request: &RuntimeProxyRequest,
    value: Option<&serde_json::Value>,
    transport: &str,
) -> &'static str {
    if is_runtime_compact_path(&request.path_and_query) {
        return "unary";
    }
    let stream_requested = value
        .and_then(|value| value.get("stream"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if is_runtime_chat_completions_path(&request.path_and_query) {
        return if transport == "websocket" || stream_requested {
            "streaming"
        } else {
            "unary"
        };
    }
    if is_runtime_responses_path(&request.path_and_query)
        && value
            .and_then(|value| value.get("stream"))
            .and_then(serde_json::Value::as_bool)
            == Some(false)
    {
        return "unary";
    }
    let anthropic_streaming =
        is_runtime_anthropic_messages_path(&request.path_and_query) && stream_requested;
    if transport == "websocket"
        || anthropic_streaming
        || is_runtime_responses_path(&request.path_and_query)
    {
        "streaming"
    } else {
        "unary"
    }
}

fn runtime_capability_tool_flags(
    request: &RuntimeProxyRequest,
    value: Option<&serde_json::Value>,
    warnings: &mut Vec<&'static str>,
) -> BTreeSet<&'static str> {
    let mut flags = BTreeSet::new();
    let Some(value) = value else {
        return flags;
    };
    if is_runtime_anthropic_messages_path(&request.path_and_query) {
        runtime_capability_collect_tool_surface_from_anthropic(value, &mut flags, warnings);
    }
    if is_runtime_responses_path(&request.path_and_query)
        || is_runtime_compact_path(&request.path_and_query)
        || is_runtime_chat_completions_path(&request.path_and_query)
    {
        runtime_capability_collect_tool_surface_from_responses(value, &mut flags);
    }
    flags
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
    fn compatibility_surface_detects_codex_session_id_header() {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses/compact".to_string(),
            headers: vec![("session-id".to_string(), " session-123 ".to_string())],
            body: br#"{"input":[]}"#.to_vec(),
        };

        let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");

        assert_eq!(surface.route, "compact");
        assert_eq!(surface.continuation, "session");
    }

    #[test]
    fn previous_response_regression_is_gated_to_codex_0_148() {
        for (user_agent, affected) in [
            ("codex-cli/0.147.0", false),
            ("codex-cli/0.148.0", true),
            ("codex_cli_rs/0.148.0", true),
            ("codex-tui/0.148.0 (Linux; x86_64)", true),
            ("codex_exec/0.148.0 (Linux; x86_64)", true),
            ("codex-cli/0.149.0", false),
        ] {
            let request = RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/backend-api/codex/responses".to_string(),
                headers: vec![("User-Agent".to_string(), user_agent.to_string())],
                body: Vec::new(),
            };

            assert_eq!(
                runtime_codex_previous_response_id_regression(&request),
                affected,
                "user_agent={user_agent}"
            );
        }
    }

    #[test]
    fn previous_response_websocket_reconnect_regression_is_version_bounded() {
        for (user_agent, affected) in [
            ("codex-cli/0.146.0", false),
            ("codex-tui/0.147.0 (Linux; x86_64)", true),
            ("codex_exec/0.148.0 (Linux; x86_64)", true),
            ("codex-cli/0.149.0", false),
        ] {
            let request = RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/backend-api/codex/responses".to_string(),
                headers: vec![("User-Agent".to_string(), user_agent.to_string())],
                body: Vec::new(),
            };

            assert_eq!(
                runtime_codex_previous_response_id_websocket_reconnect_regression(&request),
                affected,
                "user_agent={user_agent}"
            );
        }
    }

    #[test]
    fn explicit_session_headers_keep_legacy_precedence() {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: vec![
                ("x-session-id".to_string(), "x-session".to_string()),
                ("session-id".to_string(), "codex-session".to_string()),
                ("session_id".to_string(), "legacy-session".to_string()),
            ],
            body: Vec::new(),
        };

        assert_eq!(
            runtime_proxy_crate::runtime_request_explicit_session_id(&request).as_deref(),
            Some("legacy-session")
        );
    }

    #[test]
    fn responses_stream_false_and_top_level_tools_are_detected() {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: Vec::new(),
            body: br#"{
                "stream": false,
                "tools": [
                    {"type": "function", "name": "bash"},
                    {"type": "web_search_preview"}
                ]
            }"#
            .to_vec(),
        };

        let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");

        assert_eq!(surface.stream, "unary");
        assert_eq!(surface.tool_surface, "shell+web");
    }

    #[test]
    fn compact_requests_are_always_unary() {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses/compact?reason=remote".to_string(),
            headers: Vec::new(),
            body: br#"{"stream":true}"#.to_vec(),
        };

        for transport in ["http", "websocket"] {
            let surface =
                runtime_detect_request_compatibility_surface(&request, "request", transport);

            assert_eq!(surface.route, "compact");
            assert_eq!(surface.stream, "unary", "transport={transport}");
        }
    }

    #[test]
    fn chat_completions_stream_and_nested_tools_are_detected() {
        let mut request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/chat/completions".to_string(),
            headers: Vec::new(),
            body: br#"{
                "stream": true,
                "tools": [
                    {"type": "function", "function": {"name": "bash"}},
                    {"type": "function", "function": {"name": "websearch"}}
                ]
            }"#
            .to_vec(),
        };

        let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");

        assert_eq!(surface.family, "openai_compatible");
        assert_eq!(surface.client, "chat_completions_client");
        assert_eq!(surface.route, "chat_completions");
        assert_eq!(surface.stream, "streaming");
        assert_eq!(surface.tool_surface, "shell+web");

        request.body = br#"{"stream":false}"#.to_vec();
        let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");
        assert_eq!(surface.stream, "unary");
    }

    #[test]
    fn responses_without_stream_remains_streaming() {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: Vec::new(),
            body: br#"{}"#.to_vec(),
        };

        let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");

        assert_eq!(surface.stream, "streaming");
    }
}
