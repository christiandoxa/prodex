#[test]
fn runtime_proxy_http_message_followup_with_session_quota_does_not_rotate_or_fresh_fallback() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
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
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
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

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-main",
                "session_id": "sess-replayable",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "continue after quota pressure",
                    }],
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    let status = response.status().as_u16();
    let body = response.text().expect("responses body should decode");
    assert_eq!(
        status, 200,
        "quota SSE response should pass through from the owning profile: {body}"
    );
    assert!(
        body.contains("insufficient_quota"),
        "quota failure should pass through instead of becoming a fresh response: {body}"
    );
    assert!(
        !body.contains("\"id\":\"resp-second\""),
        "proxy must not replace quota-blocked message context with a fresh response: {body}"
    );

    let responses_accounts = backend.responses_accounts();
    assert_eq!(
        responses_accounts,
        vec!["main-account".to_string()],
        "quota-blocked message follow-up should not rotate off previous_response owner"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "proxy should not send a fresh retry for quota-blocked message follow-ups: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-main\""),
        "upstream request must preserve previous_response_id under quota pressure: {}",
        responses_bodies[0]
    );
    assert!(
        responses_bodies[0].contains("\"session_id\":\"sess-replayable\""),
        "upstream request must preserve session_id under quota pressure: {}",
        responses_bodies[0]
    );

    let log_tail = wait_for_runtime_log_tail_until(
        || fs::read(&proxy.log_path).ok(),
        |log| log.contains("upstream_usage_limit_passthrough") || log.contains("quota_blocked"),
        2_000,
        5_000,
        20,
    );
    let log = String::from_utf8_lossy(&log_tail);
    assert!(
        !log.contains("previous_response_fresh_fallback reason=quota_blocked"),
        "quota-blocked message follow-up must not drop previous_response_id: {log}"
    );
    assert!(
        !log.contains("quota_blocked_affinity_released"),
        "quota-blocked message follow-up must keep previous_response affinity: {log}"
    );
}
