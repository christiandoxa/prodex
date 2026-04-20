#[test]
fn runtime_proxy_translates_anthropic_messages_to_responses_and_back() {
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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
            profile_inflight: BTreeMap::new(),
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
    let temp_dir = TestDir::new();
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

#[test]
fn runtime_proxy_returns_anthropic_overloaded_error_when_interactive_capacity_is_full() {
    let _limit_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT", "4");
    let _lane_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT", "1");
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(1, 1);

    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_slow_stream();
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
    let first_url = format!("http://{}/v1/messages", proxy.listen_addr);
    let second_url = first_url.clone();
    let (first_started_tx, first_started_rx) = std::sync::mpsc::channel();
    let first = thread::spawn(move || {
        let client = Client::builder().build().expect("client");
        let response = client
            .post(first_url)
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
                            "content": "hold the interactive slot"
                        }
                    ]
                })
                .to_string(),
            )
            .send()
            .expect("first anthropic request should start");
        assert_eq!(
            response.status(),
            reqwest::StatusCode::OK,
            "first anthropic request should hold the interactive slot"
        );
        first_started_tx
            .send(())
            .expect("first anthropic request should signal readiness");
        thread::sleep(Duration::from_millis(250));
        let body = response
            .text()
            .expect("first anthropic stream should decode");
        assert!(body.contains("event: message_start"));
    });

    first_started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("first anthropic request should start before the overload probe");

    let client = Client::builder().build().expect("client");
    let response = client
        .post(second_url)
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "messages": [
                    {
                        "role": "user",
                        "content": "second request"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("second anthropic request should receive an overload response");

    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = response.json().expect("error body should parse");
    assert_eq!(
        body.get("type").and_then(serde_json::Value::as_str),
        Some("error")
    );
    assert_eq!(
        body.get("error")
            .and_then(|error| error.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("overloaded_error")
    );
    assert!(
        body.get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("temporarily saturated")),
        "unexpected error body: {body}"
    );

    first.join().expect("first request should join");
}

#[test]
fn runtime_proxy_waits_for_anthropic_inflight_relief_then_succeeds() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(20, 250);

    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
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
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("inflight guard should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25));
        drop(inflight_guard);
    });

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
            "messages": [
                {
                    "role": "user",
                    "content": "second request should wait instead of failing"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let response = proxy_runtime_anthropic_messages_request(42, &request, &shared)
        .expect("anthropic request should complete after inflight relief");

    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered anthropic response");
    };
    assert_eq!(parts.status, 200, "unexpected status after inflight wait");
    let body: serde_json::Value =
        serde_json::from_slice(&parts.body).expect("response body should parse");
    assert_eq!(
        body.get("type").and_then(serde_json::Value::as_str),
        Some("message")
    );

    release.join().expect("release thread should join");

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_started route=responses"),
        "interactive inflight wait should be logged"
    );
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "interactive inflight wait completion should be logged"
    );
    assert!(
        log.contains("useful=true"),
        "successful Anthropics inflight wait should log useful relief"
    );
    assert!(
        log.contains("wake_source=inflight_release"),
        "successful Anthropics inflight wait should log inflight_release wake source"
    );
}

#[test]
fn runtime_proxy_waits_for_responses_inflight_relief_then_succeeds() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(40, 250);

    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
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
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("inflight guard should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25));
        drop(inflight_guard);
    });

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::json!({
            "input": "second request should wait instead of failing"
        })
        .to_string()
        .into_bytes(),
    };
    let response = proxy_runtime_responses_request(43, &request, &shared)
        .expect("responses request should complete after inflight relief");

    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered responses reply");
    };
    assert_eq!(parts.status, 200, "unexpected status after inflight wait");
    let body: serde_json::Value =
        serde_json::from_slice(&parts.body).expect("response body should parse");
    assert_eq!(
        body.get("id").and_then(serde_json::Value::as_str),
        Some("resp-second")
    );

    release.join().expect("release thread should join");

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_started route=responses"),
        "responses inflight wait should be logged"
    );
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "responses inflight wait completion should be logged"
    );
    assert!(
        log.contains("useful=true"),
        "successful responses inflight wait should log useful relief"
    );
    assert!(
        log.contains("wake_source=inflight_release"),
        "successful responses inflight wait should log inflight_release wake source"
    );
}

#[test]
fn runtime_profile_inflight_relief_wait_returns_immediately_after_prior_release() {
    let temp_dir = TestDir::new();
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
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_profile_inflight_release_revision(&shared);
    shared
        .lane_admission
        .inflight_release_revision
        .fetch_add(1, Ordering::SeqCst);

    let started_at = Instant::now();
    assert!(wait_for_runtime_profile_inflight_relief_since(
        &shared,
        Duration::from_millis(100),
        observed_revision,
    ));
    assert!(
        started_at.elapsed() < ci_timing_upper_bound_ms(20, 100),
        "release-aware inflight wait should not sleep after the release was already observed"
    );
}

#[test]
fn runtime_profile_inflight_relief_wait_ignores_active_request_release_notify() {
    let temp_dir = TestDir::new();
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
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_profile_inflight_release_revision(&shared);
    let active_guard = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("active request slot should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(active_guard);
    });

    assert!(
        !wait_for_runtime_profile_inflight_relief_since(
            &shared,
            Duration::from_millis(100),
            observed_revision,
        ),
        "active-request release notify should not count as inflight relief"
    );

    release
        .join()
        .expect("active-request release thread should join");
    assert_eq!(
        runtime_profile_inflight_release_revision(&shared),
        observed_revision,
        "active-request release should not change inflight release revision"
    );
}

#[test]
fn runtime_probe_refresh_wait_returns_immediately_after_progress_is_observed() {
    let temp_dir = TestDir::new();
    let _shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_probe_refresh_revision();
    note_runtime_probe_refresh_progress();

    let started_at = Instant::now();
    assert!(wait_for_runtime_probe_refresh_since(
        Duration::from_millis(100),
        observed_revision,
    ));
    assert!(
        started_at.elapsed() < ci_timing_upper_bound_ms(20, 100),
        "probe-refresh wait should not sleep after progress was already observed"
    );
}

#[test]
fn runtime_probe_refresh_wait_ignores_lane_release_notify() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let temp_dir = TestDir::new();
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
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_probe_refresh_revision();
    let active_guard = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("active request slot should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(active_guard);
    });

    assert!(
        !wait_for_runtime_probe_refresh_since(Duration::from_millis(100), observed_revision),
        "lane-release notify should not count as probe-refresh progress"
    );

    release
        .join()
        .expect("active-request release thread should join");
    assert_eq!(
        runtime_probe_refresh_revision(),
        observed_revision,
        "lane-release notify should not change probe refresh revision"
    );
}

#[test]
fn runtime_probe_refresh_apply_waits_for_busy_runtime_state() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let temp_dir = TestDir::new();
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
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_probe_refresh_revision();
    let runtime_guard = shared.runtime.lock().expect("runtime lock should succeed");
    let apply_shared = shared.clone();
    let apply_thread = thread::spawn(move || {
        apply_runtime_profile_probe_result(
            &apply_shared,
            "main",
            AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            Ok(usage_with_main_windows(90, 3600, 90, 604_800)),
        )
    });
    thread::sleep(Duration::from_millis(20));

    assert_eq!(
        runtime_probe_refresh_revision(),
        observed_revision,
        "probe refresh revision should not advance until the runtime lock is released"
    );
    assert!(
        !wait_for_runtime_probe_refresh_since(Duration::from_millis(20), observed_revision),
        "probe refresh wait should still time out while the apply thread is blocked on runtime state"
    );

    drop(runtime_guard);
    apply_thread
        .join()
        .expect("apply thread should join")
        .expect("probe apply should succeed after the runtime lock is released");

    assert!(
        wait_for_runtime_probe_refresh_since(Duration::from_millis(100), observed_revision),
        "successful probe apply should wake probe-refresh waiters once fresh data lands"
    );
    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        runtime.profile_probe_cache.contains_key("main"),
        "probe apply should update the probe cache after waiting for the runtime lock"
    );
    assert!(
        runtime.profile_usage_snapshots.contains_key("main"),
        "probe apply should update usage snapshots after waiting for the runtime lock"
    );
}

#[test]
fn runtime_probe_inline_execution_logs_context_without_queue_lag() {
    let temp_dir = TestDir::new();
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
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    execute_runtime_probe_attempt_inline_for_test(
        &shared,
        "main",
        "startup_probe_warmup",
        Err("timeout".to_string()),
    );

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("startup_probe_warmup_error profile=main error=timeout"),
        "inline probe execution should keep the context-specific marker: {log}"
    );
    assert!(
        !log.contains("startup_probe_warmup_error profile=main lag_ms="),
        "inline probe execution should not report queue lag: {log}"
    );
}

#[test]
fn runtime_probe_queued_execution_logs_refresh_marker_with_queue_lag() {
    let temp_dir = TestDir::new();
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
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    execute_runtime_probe_attempt_queued_for_test(
        &shared,
        "main",
        Err("timeout".to_string()),
        Duration::from_millis(50),
        Instant::now(),
    );

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("profile_probe_refresh_error profile=main lag_ms="),
        "queued probe execution should keep the queue-aware marker: {log}"
    );
    assert!(
        log.contains("error=timeout"),
        "queued probe execution should preserve the fetch error: {log}"
    );
}

#[test]
fn runtime_probe_refresh_suppresses_nonlocal_upstream_in_tests_and_wakes_waiters() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");
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
                        codex_home: main_home.clone(),
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let backlog_before = runtime_probe_refresh_queue_backlog();
    let observed_revision = runtime_probe_refresh_revision();
    schedule_runtime_probe_refresh(&shared, "main", &main_home);

    assert_eq!(
        runtime_probe_refresh_queue_backlog(),
        backlog_before,
        "suppressed nonlocal probe refresh should not add background queue work"
    );
    assert!(
        wait_for_runtime_probe_refresh_since(ci_timing_upper_bound_ms(20, 200), observed_revision),
        "suppressed nonlocal probe refresh should still wake probe-refresh waiters"
    );

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime.profile_probe_cache.contains_key("main"),
        "suppressed nonlocal probe refresh should not write probe cache state"
    );
    drop(runtime);

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("profile_probe_refresh_suppressed profile=main reason=test_nonlocal_upstream"),
        "suppressed nonlocal probe refresh should be logged"
    );
}

#[test]
fn runtime_probe_refresh_nonlocal_upstream_detection_keeps_loopback_exact() {
    assert!(
        runtime_probe_refresh_nonlocal_upstream_for_test("https://chatgpt.com/backend-api"),
        "nonlocal upstreams should stay suppressed in tests"
    );
    assert!(
        runtime_probe_refresh_nonlocal_upstream_for_test(
            "https://localhost.example.com/backend-api"
        ),
        "host matching should stay exact instead of substring-based"
    );
    assert!(
        !runtime_probe_refresh_nonlocal_upstream_for_test("http://localhost:1234/backend-api"),
        "localhost loopback should stay allowed in tests"
    );
    assert!(
        !runtime_probe_refresh_nonlocal_upstream_for_test("http://127.0.0.1:1234/backend-api"),
        "IPv4 loopback should stay allowed in tests"
    );
    assert!(
        !runtime_probe_refresh_nonlocal_upstream_for_test("http://[::1]:1234/backend-api"),
        "IPv6 loopback should stay allowed in tests"
    );
}

#[test]
fn runtime_probe_refresh_allows_loopback_upstream_in_tests() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");
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
                        codex_home: main_home.clone(),
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: closed_loopback_backend_base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_probe_refresh_revision();
    schedule_runtime_probe_refresh(&shared, "main", &main_home);

    assert!(
        wait_for_runtime_probe_refresh_since(
            ci_timing_upper_bound_ms(250, 1_000),
            observed_revision
        ),
        "loopback upstreams should still run through the background refresh path in tests"
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        runtime.profile_probe_cache.contains_key("main"),
        "loopback probe refresh should still apply a probe result in tests"
    );
    drop(runtime);

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        !log.contains(
            "profile_probe_refresh_suppressed profile=main reason=test_nonlocal_upstream"
        ),
        "loopback probe refresh should not be suppressed in tests"
    );
}

#[test]
fn runtime_proxy_responses_inflight_relief_times_out_without_relief() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(20, 20);

    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");

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
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
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
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let _inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("inflight guard should be acquired");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::json!({
            "input": "request should time out without inflight relief"
        })
        .to_string()
        .into_bytes(),
    };
    let response = proxy_runtime_responses_request(44, &request, &shared)
        .expect("responses request should return a local timeout result");

    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered responses reply");
    };
    assert_eq!(
        parts.status, 503,
        "unexpected status without inflight relief"
    );
    let body: serde_json::Value =
        serde_json::from_slice(&parts.body).expect("response body should parse");
    assert_eq!(
        body.get("error")
            .and_then(|error| error.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("service_unavailable")
    );

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_started route=responses"),
        "responses inflight timeout should log wait start"
    );
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "responses inflight timeout should log wait finish"
    );
    assert!(
        log.contains("useful=false"),
        "timeout without relief should log useful=false"
    );
    assert!(
        log.contains("wake_source=timeout"),
        "timeout without relief should log wake_source=timeout"
    );
}

#[test]
fn runtime_proxy_wait_scopes_to_session_owner_relief() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(40, 250);

    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

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
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([
                    (
                        "main".to_string(),
                        ProfileEntry {
                            codex_home: main_home,
                            managed: true,
                            email: Some("main@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                    (
                        "second".to_string(),
                        ProfileEntry {
                            codex_home: second_home,
                            managed: true,
                            email: Some("second@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                ]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::from([(
                    "sess-main".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: Local::now().timestamp(),
                    },
                )]),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "second".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([
                (
                    "main".to_string(),
                    RuntimeProfileProbeCacheEntry {
                        checked_at: Local::now().timestamp(),
                        auth: AuthSummary {
                            label: "chatgpt".to_string(),
                            quota_compatible: true,
                        },
                        result: Ok(usage.clone()),
                    },
                ),
                (
                    "second".to_string(),
                    RuntimeProfileProbeCacheEntry {
                        checked_at: Local::now().timestamp(),
                        auth: AuthSummary {
                            label: "chatgpt".to_string(),
                            quota_compatible: true,
                        },
                        result: Ok(usage.clone()),
                    },
                ),
            ]),
            profile_usage_snapshots: BTreeMap::from([
                ("main".to_string(), snapshot),
                (
                    "second".to_string(),
                    runtime_profile_usage_snapshot_from_usage(&usage),
                ),
            ]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let main_inflight = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("main inflight guard should be acquired");
    let second_inflight =
        acquire_runtime_profile_inflight_guard(&shared, "second", "responses_http")
            .expect("second inflight guard should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(second_inflight);
    });

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-main".to_string()),
        ],
        body: serde_json::json!({
            "input": "request should only count owner relief as useful"
        })
        .to_string()
        .into_bytes(),
    };
    let excluded_profiles = BTreeSet::new();
    assert!(
        !runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait::new(
            45,
            &request,
            &shared,
            &excluded_profiles,
            RuntimeRouteKind::Responses,
            Instant::now(),
            true,
            Some("main"),
        ),)
        .expect("owner-scoped wait should complete"),
        "non-owner release should not count as useful relief"
    );

    release
        .join()
        .expect("non-owner release thread should join");
    drop(main_inflight);

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "owner-scoped wait should log completion"
    );
    assert!(
        log.contains("useful=false"),
        "non-owner release should not be logged as useful relief"
    );
    assert!(
        log.contains("wake_source=inflight_release"),
        "non-owner release should still be logged as an inflight release wake"
    );
}
