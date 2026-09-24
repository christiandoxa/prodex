use super::*;
use std::collections::BTreeSet;

pub(super) fn detect(
    request: &RuntimeProxyRequest,
    stage: &'static str,
    transport: &'static str,
) -> RuntimeRequestCompatibilitySurface {
    let route = route_label(request);
    let mut surface = RuntimeRequestCompatibilitySurface::new(stage, route, transport);
    classify_client_surface(&mut surface, request, route, transport);
    populate_request_metadata(&mut surface, request);

    let value = serde_json::from_slice::<serde_json::Value>(&request.body).ok();
    surface.stream = stream_label(request, value.as_ref(), transport);
    surface.continuation = continuation_label(request);
    populate_tool_surface(&mut surface, value.as_ref());
    append_request_compatibility_warnings(&mut surface, request, transport);
    surface
}

fn classify_client_surface(
    surface: &mut RuntimeRequestCompatibilitySurface,
    request: &RuntimeProxyRequest,
    route: &str,
    transport: &str,
) {
    let user_agent = runtime_proxy_request_header_value(&request.headers, "user-agent")
        .map(str::to_ascii_lowercase);
    let codex_headers = runtime_proxy_request_header_value(&request.headers, "x-codex-turn-state")
        .is_some()
        || runtime_proxy_request_header_value(&request.headers, "x-openai-subagent").is_some()
        || runtime_proxy_request_header_value(&request.headers, "session-id").is_some();
    let codex_client = codex_headers
        || transport == "websocket"
        || user_agent
            .as_deref()
            .is_some_and(|agent| agent.contains("codex"));

    if codex_client {
        surface.family = "codex";
        surface.client = if runtime_proxy_request_header_value(
            &request.headers,
            "x-openai-subagent",
        )
        .is_some()
        {
            "codex_subagent"
        } else {
            "codex_cli"
        };
        return;
    }
    if matches!(route, "responses" | "compact" | "chat_completions") {
        surface.family = "openai_compatible";
        surface.client = if route == "chat_completions" {
            "chat_completions_client"
        } else {
            "responses_client"
        };
        return;
    }
    surface.warnings.push("unknown_client_family");
}

fn populate_request_metadata(
    surface: &mut RuntimeRequestCompatibilitySurface,
    request: &RuntimeProxyRequest,
) {
    surface.user_agent = runtime_proxy_request_header_value(&request.headers, "user-agent")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-")
        .to_string();
    surface.request_origin = if runtime_proxy_request_origin(&request.headers).is_some() {
        "internal"
    } else {
        "external"
    };
}

fn populate_tool_surface(
    surface: &mut RuntimeRequestCompatibilitySurface,
    value: Option<&serde_json::Value>,
) {
    let tools = tool_flags(value);
    surface.approval = tools.contains("approval");
    surface.tool_surface = if tools.is_empty() {
        "none".to_string()
    } else {
        tools.iter().copied().collect::<Vec<_>>().join("+")
    };
}

fn append_request_compatibility_warnings(
    surface: &mut RuntimeRequestCompatibilitySurface,
    request: &RuntimeProxyRequest,
    transport: &str,
) {
    if transport == "websocket"
        && runtime_request_previous_response_id(request).is_some()
        && runtime_request_turn_state(request).is_none()
    {
        surface
            .warnings
            .push("websocket_previous_response_without_turn_state");
    }
}

fn stream_label(
    request: &RuntimeProxyRequest,
    value: Option<&serde_json::Value>,
    transport: &str,
) -> &'static str {
    if is_runtime_compact_path(&request.path_and_query) {
        return "unary";
    }
    let explicit = value
        .and_then(|value| value.get("stream"))
        .and_then(serde_json::Value::as_bool);
    if transport == "websocket"
        || explicit == Some(true)
        || (is_runtime_responses_path(&request.path_and_query) && explicit != Some(false))
    {
        "streaming"
    } else {
        "unary"
    }
}

fn continuation_label(request: &RuntimeProxyRequest) -> String {
    let mut labels = BTreeSet::new();
    if runtime_request_previous_response_id(request).is_some() {
        labels.insert("previous_response");
    }
    if runtime_request_turn_state(request).is_some() {
        labels.insert("turn_state");
    }
    if runtime_request_session_id(request).is_some() {
        labels.insert("session");
    }
    if labels.is_empty() {
        "none".to_string()
    } else {
        labels.into_iter().collect::<Vec<_>>().join("+")
    }
}

fn tool_flags(value: Option<&serde_json::Value>) -> BTreeSet<&'static str> {
    let mut flags = BTreeSet::new();
    let Some(tools) = value
        .and_then(|value| value.get("tools"))
        .and_then(serde_json::Value::as_array)
    else {
        return flags;
    };
    if !tools.is_empty() {
        flags.insert("tools");
    }
    for tool in tools {
        let label = tool
            .get("type")
            .or_else(|| tool.get("name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if label.contains("web") {
            flags.insert("web");
        }
        if label.contains("mcp") {
            flags.insert("mcp");
        }
        if label.contains("computer") {
            flags.insert("computer");
        }
        if label.contains("shell") || label.contains("bash") {
            flags.insert("shell");
        }
        if label.contains("approval") {
            flags.insert("approval");
        }
    }
    flags
}
