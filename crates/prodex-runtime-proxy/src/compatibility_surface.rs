use crate::{
    RuntimeProxyRequest, is_runtime_chat_completions_path, is_runtime_compact_path,
    is_runtime_responses_path, runtime_proxy_request_header_value, runtime_proxy_request_origin,
    runtime_request_previous_response_id, runtime_request_session_id, runtime_request_turn_state,
};

#[cfg(any(not(feature = "mojo"), test))]
#[path = "compatibility_surface/rust_oracle.rs"]
mod rust_oracle;

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
            match route {
                "responses" => 0,
                "compact" => 1,
                "chat_completions" => 2,
                _ => 3,
            },
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
        family: compatibility_tag_label(&["unknown", "codex", "openai_compatible"], plan[0]),
        client: compatibility_tag_label(
            &[
                "unknown",
                "codex_subagent",
                "codex_cli",
                "chat_completions_client",
                "responses_client",
            ],
            plan[1],
        ),
        route,
        transport,
        stream: compatibility_tag_label(&["unary", "streaming"], plan[2]),
        tool_surface: compatibility_flag_string(
            plan[3],
            &[
                (32, "approval"),
                (8, "computer"),
                (4, "mcp"),
                (16, "shell"),
                (1, "tools"),
                (2, "web"),
            ],
            "none",
        ),
        continuation: compatibility_flag_string(
            plan[4],
            &[(1, "previous_response"), (4, "session"), (2, "turn_state")],
            "none",
        ),
        request_origin: compatibility_tag_label(&["external", "internal"], plan[6]),
        approval: plan[3] & 32 != 0,
        user_agent: user_agent
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("-")
            .to_string(),
        warnings: compatibility_flag_labels(
            plan[5],
            &[
                (1, "unknown_client_family"),
                (2, "websocket_previous_response_without_turn_state"),
            ],
        ),
    }
}

#[cfg(feature = "mojo")]
fn compatibility_tag_label(labels: &'static [&'static str], tag: i64) -> &'static str {
    usize::try_from(tag)
        .ok()
        .and_then(|index| labels.get(index))
        .copied()
        .unwrap_or(labels[0])
}

#[cfg(feature = "mojo")]
fn compatibility_flag_labels(
    flags: i64,
    labels: &'static [(i64, &'static str)],
) -> Vec<&'static str> {
    labels
        .iter()
        .filter_map(|(flag, label)| (flags & flag != 0).then_some(*label))
        .collect()
}

#[cfg(feature = "mojo")]
fn compatibility_flag_string(
    flags: i64,
    labels: &'static [(i64, &'static str)],
    empty: &'static str,
) -> String {
    let labels = compatibility_flag_labels(flags, labels);
    if labels.is_empty() {
        empty.to_string()
    } else {
        labels.join("+")
    }
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
