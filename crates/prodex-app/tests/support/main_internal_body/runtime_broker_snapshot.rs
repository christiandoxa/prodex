use super::*;

#[test]
fn runtime_proxy_broker_live_log_snapshot_is_authenticated_and_bounded() {
    let _runtime_lock = acquire_test_runtime_lock();
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    register_runtime_broker_metadata(
        &proxy.log_path,
        RuntimeBrokerMetadata {
            broker_key: runtime_broker_key_with_smart_context(
                &backend.base_url(),
                false,
                false,
                false,
                None,
            ),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            instance_id: "live-log-instance".to_string(),
            admin_token: runtime_broker_test_secret("secret"),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: None,
            executable_sha256: None,
        },
    );
    let baseline_cursor = runtime_proxy_live_log_snapshot(&proxy.log_path, 0, 0)
        .expect("live log baseline should exist")
        .cursor;
    runtime_proxy_log_to_path(&proxy.log_path, "live_log_snapshot_test event=visible");
    runtime_proxy_flush_logs_for_path(&proxy.log_path).expect("live log should flush");
    let live_snapshot = runtime_proxy_live_log_snapshot(&proxy.log_path, baseline_cursor, 8)
        .expect("live log should exist");
    assert!(
        live_snapshot
            .entries
            .iter()
            .any(|entry| entry.line.contains("live_log_snapshot_test"))
    );

    let client = Client::builder().no_proxy().build().expect("client");
    let unauthorized = client
        .get(format!(
            "http://{}/__prodex/runtime/log/snapshot",
            proxy.listen_addr
        ))
        .send()
        .expect("unauthorized snapshot request should complete");
    assert_eq!(unauthorized.status().as_u16(), 403);

    for (query, expected_message) in [
        ("after=nope", "runtime log snapshot cursor is invalid"),
        (
            "limit=0",
            "runtime log snapshot limit is outside the supported bound",
        ),
    ] {
        let response = client
            .get(format!(
                "http://{}/__prodex/runtime/log/snapshot?{query}",
                proxy.listen_addr
            ))
            .header("X-Prodex-Admin-Token", "secret")
            .send()
            .unwrap();
        assert_eq!(response.status().as_u16(), 400);
        let body: serde_json::Value = response.json().unwrap();
        assert_eq!(body["error"]["code"], "invalid_request");
        assert_eq!(body["error"]["message"], expected_message);
    }

    let response = client
        .get(format!(
            "http://{}/__prodex/runtime/log/snapshot?after={baseline_cursor}&limit=8",
            proxy.listen_addr,
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("authorized snapshot request should complete");
    assert_eq!(response.status().as_u16(), 200);
    let body = response
        .json::<serde_json::Value>()
        .expect("snapshot should be JSON");
    assert!(
        body["entries"]
            .as_array()
            .is_some_and(|entries| entries.iter().any(|entry| {
                entry["line"]
                    .as_str()
                    .is_some_and(|line| line.contains("live_log_snapshot_test"))
            }))
    );
}
