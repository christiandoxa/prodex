use super::*;

#[test]
fn runtime_proxy_translates_anthropic_messages_to_responses_and_back() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/v1/messages?beta=true",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .header("x-claude-code-session-id", "claude-session-42")
        .header("User-Agent", "claude-cli/test")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "max_tokens": 256,
                "messages": [
                    {
                        "role": "user",
                        "content": "hello from Claude"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("anthropic proxy request should succeed");

    assert!(
        response.status().is_success(),
        "unexpected status: {}",
        response.status()
    );
    let body: serde_json::Value = response.json().expect("anthropic response should parse");
    assert_eq!(
        body.get("type").and_then(serde_json::Value::as_str),
        Some("message")
    );
    assert_eq!(
        body.get("model").and_then(serde_json::Value::as_str),
        Some("claude-sonnet-4-6")
    );

    let headers = backend.responses_headers();
    let request_headers = headers
        .first()
        .expect("backend should capture translated request headers");
    assert_eq!(
        request_headers.get("session_id").map(String::as_str),
        Some("claude-session-42")
    );
    assert_eq!(
        request_headers.get("user-agent").map(String::as_str),
        Some("claude-cli/test")
    );
    assert_eq!(
        request_headers
            .get("chatgpt-account-id")
            .map(String::as_str),
        Some("second-account")
    );
    assert!(
        !request_headers.contains_key("x-prodex-internal-request-origin"),
        "internal prodex request origin header must not leak upstream"
    );

    let translated_body = backend
        .responses_bodies()
        .into_iter()
        .next()
        .expect("backend should capture translated request body");
    let translated_json: serde_json::Value =
        serde_json::from_str(&translated_body).expect("translated request body should parse");
    assert_eq!(
        translated_json
            .get("model")
            .and_then(serde_json::Value::as_str),
        Some("gpt-5.3-codex")
    );
    assert_eq!(
        translated_json
            .get("input")
            .and_then(serde_json::Value::as_array)
            .and_then(|input| input.first())
            .and_then(|item| item.get("content"))
            .and_then(serde_json::Value::as_str),
        Some("hello from Claude")
    );
}

#[test]
fn runtime_proxy_anthropic_messages_retries_tool_result_transcript_on_another_profile() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/v1/messages?beta=true",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "stream": true,
                "messages": [
                    {
                        "role": "user",
                        "content": "Run pwd"
                    },
                    {
                        "role": "assistant",
                        "content": [
                            {
                                "type": "tool_use",
                                "id": "toolu_1",
                                "name": "shell",
                                "input": {
                                    "cmd": "pwd"
                                }
                            }
                        ]
                    },
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "tool_result",
                                "tool_use_id": "toolu_1",
                                "content": "ok"
                            }
                        ]
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("anthropic proxy request should succeed");

    assert_eq!(
        response.status(),
        reqwest::StatusCode::OK,
        "unexpected anthropic status: {}",
        response.status()
    );
    let body = response
        .text()
        .expect("anthropic stream body should decode");
    assert!(
        body.contains("event: message_start"),
        "unexpected body: {body}"
    );
    assert!(
        body.contains("event: message_stop"),
        "unexpected body: {body}"
    );
    assert!(
        !body.contains("You've hit your usage limit"),
        "fresh anthropic tool-result transcript should rotate instead of surfacing quota: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
}

#[test]
fn runtime_proxy_streams_anthropic_messages_from_buffered_responses() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "stream-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/v1/messages?beta=true",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .header("x-claude-code-session-id", "claude-session-42")
        .header("User-Agent", "claude-cli/test")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "stream": true,
                "messages": [
                    {
                        "role": "user",
                        "content": "hello from Claude"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("anthropic proxy request should succeed");

    assert!(
        response.status().is_success(),
        "unexpected status: {}",
        response.status()
    );
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let body = response.text().expect("stream body should decode");
    assert!(body.contains("event: message_start"));
    assert!(body.contains("event: message_stop"));
}

#[test]
fn runtime_proxy_streams_anthropic_mcp_messages_without_buffering() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_anthropic_mcp_stream();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");
    let usage = usage_with_main_windows(90, 3600, 90, 604_800);
    let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: profile_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: Local::now().timestamp(),
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage.clone()),
                },
            )]),
            profile_usage_snapshots: BTreeMap::from([("main".to_string(), snapshot)]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-api-key".to_string(), "dummy".to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ],
        body: serde_json::json!({
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
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "List the workspace files."
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let response = proxy_runtime_anthropic_messages_request(42, &request, &shared)
        .expect("anthropic mcp request should succeed");

    let RuntimeResponsesReply::Streaming(response) = response else {
        panic!("expected streaming anthropic mcp response");
    };
    let mut body = String::new();
    let mut reader = response.body;
    reader
        .read_to_string(&mut body)
        .expect("streaming anthropic mcp body should read");
    assert!(body.contains("\"mcp_tool_use\""));
    assert!(body.contains("\"local_fs\""));
    assert!(body.contains("\"mcp_tool_result\""));
    assert!(body.contains("\"stop_reason\":\"end_turn\""));
}

#[test]
fn runtime_proxy_continues_anthropic_web_search_server_tool_responses() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_anthropic_web_search_followup();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/v1/messages?beta=true",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .header("User-Agent", "claude-cli/test")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "messages": [
                    {
                        "role": "user",
                        "content": "Cari berita terbaru reksadana Indonesia"
                    }
                ],
                "tools": [
                    {
                        "type": "web_search_20260209",
                        "name": "web_search"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("anthropic proxy request should succeed");

    assert!(
        response.status().is_success(),
        "unexpected status: {}",
        response.status()
    );
    let body: serde_json::Value = response.json().expect("anthropic response should parse");
    assert_eq!(
        body.get("stop_reason").and_then(serde_json::Value::as_str),
        Some("end_turn")
    );
    assert_eq!(
        body.get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.last())
            .and_then(|item| item.get("text"))
            .and_then(serde_json::Value::as_str),
        Some("Ringkasan terbaru reksadana Indonesia.")
    );
    assert_eq!(
        body.get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );

    let requests = backend.responses_bodies();
    assert_eq!(
        requests.len(),
        2,
        "proxy should issue a follow-up continuation"
    );

    let first_request: serde_json::Value =
        serde_json::from_str(&requests[0]).expect("first request should parse");
    assert_eq!(
        first_request
            .get("input")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    let second_request: serde_json::Value =
        serde_json::from_str(&requests[1]).expect("second request should parse");
    assert_eq!(
        second_request
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str),
        Some("resp_ws_followup_1")
    );
    assert!(
        second_request.get("input").is_none(),
        "follow-up request should not replay the original input"
    );
    assert_eq!(
        second_request
            .get("stream")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}
