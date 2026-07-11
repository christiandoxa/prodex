use super::*;

#[test]
fn runtime_proxy_http_preserves_codex_0142_backend_headers_and_payload() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let turn_metadata = serde_json::json!({
        "session_id": "sess-0142",
        "thread_id": "thread-0142",
        "turn_id": "turn-0142",
        "window_id": "thread-0142:1",
    })
    .to_string();
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/prodex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("authorization", "Bearer caller-token")
        .header("chatgpt-account-id", "caller-account")
        .header("connection", "keep-alive, x-local-hop")
        .header("x-local-hop", "strip-me")
        .header("session-id", "sess-0142")
        .header("thread-id", "thread-0142")
        .header("x-client-request-id", "thread-0142")
        .header("x-codex-turn-metadata", turn_metadata.as_str())
        .header("x-codex-turn-state", "turn-state-0142")
        .header("x-codex-beta-features", "goals,plugins")
        .header("x-oai-attestation", "attestation-0142")
        .header("openai-beta", "responses_websockets=2026-02-06")
        .header("x-responsesapi-include-timing-metrics", "true")
        .header("x-openai-internal-codex-responses-lite", "true")
        .header("user-agent", "codex-cli/0.142.0")
        .body(
            serde_json::json!({
                "model": "gpt-5",
                "input": "hello",
                "client_metadata": {
                    "session_id": "sess-0142",
                    "thread_id": "thread-0142",
                    "x-codex-turn-metadata": turn_metadata,
                    "x-codex-ws-stream-request-start-ms": "12345",
                    "ws_request_header_x_openai_internal_codex_responses_lite": "true"
                }
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let responses_headers = backend.responses_headers();
    assert_eq!(
        responses_headers.len(),
        1,
        "backend should observe one responses request: {responses_headers:?}"
    );
    let headers = &responses_headers[0];
    for (name, expected) in [
        ("session-id", "sess-0142"),
        ("thread-id", "thread-0142"),
        ("x-client-request-id", "thread-0142"),
        ("x-codex-turn-metadata", turn_metadata.as_str()),
        ("x-codex-turn-state", "turn-state-0142"),
        ("x-codex-beta-features", "goals,plugins"),
        ("x-oai-attestation", "attestation-0142"),
        ("openai-beta", "responses_websockets=2026-02-06"),
        ("x-responsesapi-include-timing-metrics", "true"),
        ("x-openai-internal-codex-responses-lite", "true"),
        ("user-agent", "codex-cli/0.142.0"),
    ] {
        assert_eq!(
            headers.get(name).map(String::as_str),
            Some(expected),
            "backend header {name} should be forwarded"
        );
    }
    assert_eq!(
        headers.get("authorization").map(String::as_str),
        Some("Bearer test-token"),
        "proxy must replace caller Authorization with selected profile auth"
    );
    assert_eq!(
        headers.get("chatgpt-account-id").map(String::as_str),
        Some("second-account"),
        "proxy must replace caller ChatGPT account with selected profile account"
    );
    assert!(
        !headers.contains_key("x-local-hop"),
        "proxy must not forward headers named by Connection"
    );

    let responses_bodies = backend.responses_bodies();
    let body = serde_json::from_str::<serde_json::Value>(&responses_bodies[0])
        .expect("upstream request body should be JSON");
    assert_eq!(body["client_metadata"]["session_id"], "sess-0142");
    assert_eq!(body["client_metadata"]["thread_id"], "thread-0142");
    assert_eq!(
        body["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
            .and_then(|value| value.get("turn_id").cloned()),
        Some(serde_json::json!("turn-0142"))
    );
    assert_eq!(
        body["client_metadata"]["ws_request_header_x_openai_internal_codex_responses_lite"],
        "true"
    );
}

#[test]
fn runtime_proxy_http_does_not_synthesize_user_agent() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .no_proxy()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/prodex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(serde_json::json!({"model": "gpt-5", "input": "hello"}).to_string())
        .send()
        .expect("responses request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let responses_headers = backend.responses_headers();
    assert_eq!(
        responses_headers.len(),
        1,
        "backend should observe one responses request: {responses_headers:?}"
    );
    assert!(
        !responses_headers[0].contains_key("user-agent"),
        "proxy should not synthesize User-Agent; upstream Codex owns client/version headers"
    );
}
