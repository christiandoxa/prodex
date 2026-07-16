fn compat_replay_request_from_fixture(value: &Value) -> RuntimeProxyRequest {
    let headers = value
        .get("headers")
        .and_then(Value::as_array)
        .expect("request fixture should include headers array")
        .iter()
        .map(|entry| {
            let entry = entry
                .as_object()
                .expect("header fixture entry should be an object");
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .expect("header fixture entry should include name");
            let value = entry
                .get("value")
                .and_then(Value::as_str)
                .expect("header fixture entry should include value");
            (name.to_string(), value.to_string())
        })
        .collect();
    let body = value.get("body").cloned().unwrap_or(Value::Null);

    RuntimeProxyRequest {
        method: value
            .get("method")
            .and_then(Value::as_str)
            .expect("request fixture should include method")
            .to_string(),
        path_and_query: value
            .get("path_and_query")
            .and_then(Value::as_str)
            .expect("request fixture should include path_and_query")
            .to_string(),
        headers,
        body: if body.is_null() {
            Vec::new()
        } else {
            serde_json::to_vec(&body).expect("request fixture body should serialize")
        },
    }
}

fn compat_replay_surface_value(surface: RuntimeRequestCompatibilitySurface) -> Value {
    serde_json::json!({
        "stage": surface.stage,
        "family": surface.family,
        "client": surface.client,
        "route": surface.route,
        "transport": surface.transport,
        "stream": surface.stream,
        "tool_surface": surface.tool_surface,
        "continuation": surface.continuation,
        "request_origin": surface.request_origin,
        "approval": surface.approval,
        "user_agent": surface.user_agent,
        "warnings": surface.warnings,
    })
}

fn compat_replay_sse_payload_values(text: &str) -> Value {
    let mut data_lines = Vec::new();
    let mut payloads = Vec::new();
    let mut flush = |data_lines: &mut Vec<String>| {
        if let Some(value) = parse_runtime_sse_payload(data_lines) {
            payloads.push(value);
        }
        data_lines.clear();
    };

    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            flush(&mut data_lines);
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.strip_prefix(' ').unwrap_or(data).to_string());
        }
    }
    flush(&mut data_lines);

    Value::Array(payloads)
}

fn compat_replay_sse_inspection_value(progress: RuntimeSseInspectionProgress) -> Value {
    match progress {
        RuntimeSseInspectionProgress::Hold {
            response_ids,
            turn_state,
        } => serde_json::json!({
            "kind": "hold",
            "response_ids": response_ids,
            "turn_state": turn_state,
        }),
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        } => serde_json::json!({
            "kind": "commit",
            "response_ids": response_ids,
            "turn_state": turn_state,
        }),
        RuntimeSseInspectionProgress::QuotaBlocked => serde_json::json!({
            "kind": "quota_blocked",
        }),
        RuntimeSseInspectionProgress::Overloaded => serde_json::json!({
            "kind": "overloaded",
        }),
        RuntimeSseInspectionProgress::PreviousResponseNotFound => serde_json::json!({
            "kind": "previous_response_not_found",
        }),
    }
}

fn compat_replay_translated_request_value(request: &RuntimeProxyRequest) -> Value {
    serde_json::json!({
        "method": request.method.as_str(),
        "path_and_query": request.path_and_query.as_str(),
        "headers": request
            .headers
            .iter()
            .map(|(name, value)| serde_json::json!({
                "name": name.as_str(),
                "value": value.as_str(),
            }))
            .collect::<Vec<_>>(),
        "body": serde_json::from_slice::<Value>(&request.body)
            .expect("translated request body should be valid JSON"),
    })
}

fn compat_replay_fake_secret(parts: &[&str]) -> String {
    parts.concat()
}

fn compat_replay_fake_named_secret(name: &str) -> String {
    compat_replay_fake_secret(&["fixture_", name, "_notreal_", "12345"])
}

fn compat_replay_fake_api_key(prefix: &str, name: &str) -> String {
    compat_replay_fake_secret(&[prefix, "fixture-", name, "-notreal-", "123456789"])
}

fn compat_replay_sample_fake_secrets() -> Vec<String> {
    vec![
        compat_replay_fake_named_secret("authorization"),
        compat_replay_fake_api_key("sk-live-", "header"),
        compat_replay_fake_named_secret("body_api_key"),
        compat_replay_fake_named_secret("sse_token"),
    ]
}

fn compat_replay_anthropic_request(headers: Vec<(&str, &str)>, body: Value) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: headers
            .into_iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        body: body.to_string().into_bytes(),
    }
}

fn compat_replay_translate_anthropic_request(request: &RuntimeProxyRequest) -> String {
    let translated =
        translate_runtime_anthropic_messages_request(request).expect("translation should succeed");
    compat_replay_normalize_json(&translated.translated_request.body)
}

fn compat_replay_websocket_handshake_request(headers: Vec<(&str, &str)>) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: headers
            .into_iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        body: Vec::new(),
    }
}

fn compat_replay_anthropic_contract_body() -> Value {
    serde_json::json!({
        "model": "claude-sonnet-4-6",
        "stream": true,
        "mcp_servers": [
            {
                "name": "local_fs",
                "url": "https://mcp.example.com/sse"
            }
        ],
        "tools": [
            {
                "type": "mcp_toolset",
                "mcp_server_name": "local_fs",
                "name": "filesystem"
            },
            {
                "type": "mcp_toolset",
                "mcp_server_name": "local_fs",
                "name": "filesystem_readonly"
            }
        ],
        "messages": [
            {
                "role": "system",
                "content": "You are a compatibility replay harness."
            },
            {
                "role": "user",
                "content": "List the workspace files."
            }
        ]
    })
}

fn compat_replay_anthropic_contract_variants() -> Vec<RuntimeProxyRequest> {
    let body = compat_replay_anthropic_contract_body();
    vec![
        compat_replay_anthropic_request(
            vec![
                ("Content-Type", "application/json"),
                ("x-api-key", "dummy"),
                ("anthropic-version", "2023-06-01"),
            ],
            body.clone(),
        ),
        compat_replay_anthropic_request(
            vec![
                ("anthropic-version", "2023-06-01"),
                ("x-api-key", "dummy"),
                ("Content-Type", "application/json"),
                ("User-Agent", "claude-cli/test"),
            ],
            body.clone(),
        ),
        compat_replay_anthropic_request(
            vec![
                ("x-api-key", "dummy"),
                ("Content-Type", "application/json"),
                ("anthropic-version", "2023-06-01"),
                ("x-claude-code-session-id", "claude-session-42"),
            ],
            body,
        ),
    ]
}
