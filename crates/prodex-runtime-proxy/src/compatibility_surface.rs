use crate::{
    RuntimeProxyRequest, is_runtime_chat_completions_path, is_runtime_compact_path,
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

pub fn runtime_detect_request_compatibility_surface(
    request: &RuntimeProxyRequest,
    stage: &'static str,
    transport: &'static str,
) -> RuntimeRequestCompatibilitySurface {
    let route = route_label(request);
    let mut surface = RuntimeRequestCompatibilitySurface::new(stage, route, transport);
    let user_agent = runtime_proxy_request_header_value(&request.headers, "user-agent")
        .map(str::to_ascii_lowercase);
    let codex_headers = runtime_proxy_request_header_value(&request.headers, "x-codex-turn-state")
        .is_some()
        || runtime_proxy_request_header_value(&request.headers, "x-openai-subagent").is_some()
        || runtime_proxy_request_header_value(&request.headers, "session-id").is_some();

    if codex_headers
        || transport == "websocket"
        || user_agent
            .as_deref()
            .is_some_and(|agent| agent.contains("codex"))
    {
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
    } else if matches!(route, "responses" | "compact" | "chat_completions") {
        surface.family = "openai_compatible";
        surface.client = if route == "chat_completions" {
            "chat_completions_client"
        } else {
            "responses_client"
        };
    } else {
        surface.warnings.push("unknown_client_family");
    }

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

    let value = serde_json::from_slice::<serde_json::Value>(&request.body).ok();
    surface.stream = stream_label(request, value.as_ref(), transport);
    surface.continuation = continuation_label(request);
    let tools = tool_flags(value.as_ref());
    surface.approval = tools.contains("approval");
    surface.tool_surface = if tools.is_empty() {
        "none".to_string()
    } else {
        tools.iter().copied().collect::<Vec<_>>().join("+")
    };

    if transport == "websocket"
        && runtime_request_previous_response_id(request).is_some()
        && runtime_request_turn_state(request).is_none()
    {
        surface
            .warnings
            .push("websocket_previous_response_without_turn_state");
    }
    surface
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
}
