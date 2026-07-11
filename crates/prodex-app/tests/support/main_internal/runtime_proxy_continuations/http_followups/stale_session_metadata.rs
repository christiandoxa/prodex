#[test]
fn runtime_proxy_http_message_followup_with_session_header_does_not_fresh_fallback() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
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
        response_profile_bindings: BTreeMap::from([(
            "resp-missing".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
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
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("session_id", "sess-replayable")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-missing",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "continue the same conversation",
                    }],
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(
        response.status().as_u16(),
        409,
        "session header must not make a message follow-up replayable"
    );
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"code\":\"stale_continuation\""),
        "client should see stale_continuation instead of raw continuity loss: {body}"
    );
    assert!(
        !body.contains("\"id\":\"resp-second\""),
        "proxy must not replace lost context with a fresh response: {body}"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "proxy should not send a fresh retry for session-header message follow-ups: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-missing\""),
        "upstream request must preserve previous_response chain: {}",
        responses_bodies[0]
    );
}

#[test]
fn runtime_proxy_http_message_followup_with_turn_metadata_session_does_not_fresh_fallback() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
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
        response_profile_bindings: BTreeMap::from([(
            "resp-missing".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
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

    let turn_metadata = serde_json::json!({
        "session_id": "sess-replayable"
    })
    .to_string();
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("x-codex-turn-metadata", turn_metadata.clone())
        .body(
            serde_json::json!({
                "previous_response_id": "resp-missing",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "continue the same conversation",
                    }],
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(
        response.status().as_u16(),
        409,
        "turn metadata session_id must not make a message follow-up replayable"
    );
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"code\":\"stale_continuation\""),
        "client should see stale_continuation instead of raw continuity loss: {body}"
    );
    assert!(
        !body.contains("\"id\":\"resp-second\""),
        "proxy must not replace lost context with a fresh response: {body}"
    );

    let responses_accounts = backend.responses_accounts();
    assert_eq!(
        responses_accounts,
        vec!["second-account".to_string()],
        "metadata session follow-up should stay on owning profile without fallback"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "proxy should not send a fresh retry for metadata session message follow-ups: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-missing\""),
        "upstream request must preserve previous_response chain: {}",
        responses_bodies[0]
    );

    let responses_headers = backend.responses_headers();
    assert_eq!(
        responses_headers.len(),
        1,
        "backend should record the single upstream attempt: {responses_headers:?}"
    );
    assert_eq!(
        responses_headers[0]
            .get("x-codex-turn-metadata")
            .map(String::as_str),
        Some(turn_metadata.as_str()),
        "turn metadata should be preserved on the upstream continuation"
    );
}
