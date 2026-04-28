use super::*;

#[test]
fn runtime_sse_tap_reader_keeps_response_affinity_when_prelude_splits_event() {
    let temp_dir = TestDir::isolated();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
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
    let runtime = RuntimeRotationState {
        paths: paths.clone(),
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "second".to_string(),
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
    };
    let shared = RuntimeRotationProxyShared {
        upstream_no_proxy: false,
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    let prelude =
            b"event: response.created\r\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second";
    let remainder =
            b"\"}}\r\n\r\nevent: response.completed\r\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-second\"}}\r\n\r\n";
    let mut reader = RuntimeSseTapReader::new(
        Cursor::new(prelude.to_vec()).chain(Cursor::new(remainder.to_vec())),
        shared.clone(),
        "second".to_string(),
        prelude,
        &[],
        None,
        None,
    );
    let mut body = Vec::new();
    reader
        .read_to_end(&mut body)
        .expect("split SSE payload should be readable");

    let persisted = wait_for_state(&paths, |state| {
        state
            .response_profile_bindings
            .get("resp-second")
            .is_none_or(|binding| binding.profile_name != "main")
    });
    assert!(
        persisted
            .response_profile_bindings
            .get("resp-second")
            .is_none_or(|binding| binding.profile_name != "main"),
        "stale previous_response_id should not stay pinned to the wrong owner"
    );
}
