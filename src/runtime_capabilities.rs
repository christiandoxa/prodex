use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeRequestCompatibilitySurface {
    pub(super) stage: &'static str,
    pub(super) family: &'static str,
    pub(super) client: &'static str,
    pub(super) route: &'static str,
    pub(super) transport: &'static str,
    pub(super) stream: &'static str,
    pub(super) tool_surface: String,
    pub(super) continuation: String,
    pub(super) request_origin: &'static str,
    pub(super) approval: bool,
    pub(super) user_agent: String,
    pub(super) warnings: Vec<&'static str>,
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
        || user_agent
            .as_deref()
            .is_some_and(|agent| agent.contains("claude-code") || agent.contains("claude-cli"))
    {
        return ("claude_code", "claude_code");
    }

    if is_runtime_anthropic_messages_path(&request.path_and_query) {
        if runtime_capability_request_origin_label(request) == "anthropic_messages" {
            return ("claude_code", "claude_code");
        }
        if user_agent
            .as_deref()
            .is_some_and(|agent| agent.contains("anthropic"))
        {
            return ("anthropic", "anthropic_sdk");
        }
        return ("anthropic", "anthropic_compatible");
    }

    if has_codex_turn_state
        || has_codex_subagent
        || route == "websocket"
        || user_agent
            .as_deref()
            .is_some_and(|agent| agent.contains("codex"))
    {
        return if has_codex_subagent {
            ("codex", "codex_subagent")
        } else {
            ("codex", "codex_cli")
        };
    }

    if is_runtime_responses_path(&request.path_and_query)
        || is_runtime_compact_path(&request.path_and_query)
    {
        ("openai_compatible", "responses_client")
    } else {
        ("unknown", "unknown")
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
        for tool in tools {
            saw_tools = true;
            let tool_type = tool
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let tool_name = tool
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let unversioned = runtime_proxy_anthropic_unversioned_tool_type(tool_type);
            match unversioned.as_str() {
                "mcp_toolset" => {
                    has_mcp_toolset = true;
                    flags.insert("mcp");
                }
                "bash" => {
                    flags.insert("shell");
                }
                "computer" => {
                    flags.insert("computer");
                }
                "text_editor" => {
                    flags.insert("editor");
                }
                "web_search" | "web_fetch" => {
                    flags.insert("web");
                }
                _ => {
                    if tool_name.contains("websearch") || tool_name.contains("web_fetch") {
                        flags.insert("web");
                    } else if !tool_name.is_empty() {
                        flags.insert("generic_tool");
                    }
                }
            }
        }
    }

    if let Some(messages) = value.get("messages").and_then(serde_json::Value::as_array) {
        for message in messages {
            if let Some(content) = message.get("content").and_then(serde_json::Value::as_array) {
                for block in content {
                    let block_type = block
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    if matches!(
                        block_type,
                        "mcp_approval_request" | "mcp_approval_response" | "mcp_list_tools"
                    ) {
                        flags.insert("approval");
                    }
                }
            }
        }
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

fn runtime_capability_collect_tool_surface_from_responses(
    value: &serde_json::Value,
    flags: &mut BTreeSet<&'static str>,
) {
    if let Some(items) = value.get("input").and_then(serde_json::Value::as_array) {
        for item in items {
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
                if matches!(name.as_str(), "websearch" | "webfetch") {
                    flags.insert("web");
                } else if name == "bash" {
                    flags.insert("shell");
                } else if name == "computer" {
                    flags.insert("computer");
                } else {
                    flags.insert("generic_tool");
                }
            }
        }
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

pub(super) fn runtime_detect_request_compatibility_surface(
    request: &RuntimeProxyRequest,
    stage: &'static str,
    transport: &'static str,
) -> RuntimeRequestCompatibilitySurface {
    let route = if is_runtime_anthropic_messages_path(&request.path_and_query) {
        "anthropic_messages"
    } else if is_runtime_compact_path(&request.path_and_query) {
        "compact"
    } else if is_runtime_responses_path(&request.path_and_query) {
        "responses"
    } else {
        "standard"
    };
    let value = runtime_capability_request_json(request);
    let mut surface = RuntimeRequestCompatibilitySurface::new(stage, route, transport);
    let (family, client) = runtime_capability_client_labels(request, route);
    surface.family = family;
    surface.client = client;
    let anthropic_streaming = is_runtime_anthropic_messages_path(&request.path_and_query)
        && value
            .as_ref()
            .and_then(|value| value.get("stream"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
    let streaming = transport == "websocket"
        || anthropic_streaming
        || is_runtime_responses_path(&request.path_and_query);
    surface.stream = if streaming { "streaming" } else { "unary" };
    surface.continuation = runtime_capability_continuation_label(request);
    surface.request_origin = runtime_capability_request_origin_label(request);
    surface.user_agent = runtime_proxy_request_header_value(&request.headers, "user-agent")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "-".to_string());

    let mut tool_flags = BTreeSet::new();
    if let Some(value) = value.as_ref() {
        if is_runtime_anthropic_messages_path(&request.path_and_query) {
            runtime_capability_collect_tool_surface_from_anthropic(
                value,
                &mut tool_flags,
                &mut surface.warnings,
            );
        }
        if is_runtime_responses_path(&request.path_and_query)
            || is_runtime_compact_path(&request.path_and_query)
        {
            runtime_capability_collect_tool_surface_from_responses(value, &mut tool_flags);
        }
    }

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

pub(super) fn runtime_detect_websocket_message_compatibility_surface(
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

pub(super) fn runtime_proxy_log_request_compatibility(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    surface: &RuntimeRequestCompatibilitySurface,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} compat_request_surface stage={} family={} client={} route={} transport={} stream={} tool_surface={} continuation={} approval={} origin={} user_agent={}",
            surface.stage,
            surface.family,
            surface.client,
            surface.route,
            surface.transport,
            surface.stream,
            surface.tool_surface,
            surface.continuation,
            surface.approval,
            surface.request_origin,
            runtime_capability_log_safe_value(&surface.user_agent),
        ),
    );
    for warning in &surface.warnings {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} compat_warning stage={} family={} client={} route={} transport={} warning={warning}",
                surface.stage, surface.family, surface.client, surface.route, surface.transport
            ),
        );
    }
}

fn runtime_capability_log_safe_value(value: &str) -> String {
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
