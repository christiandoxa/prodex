use super::*;

fn runtime_proxy_affinity_test_shared(name: &str) -> RuntimeRotationProxyShared {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "prodex-affinity-test-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("runtime affinity test root should exist");
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    };

    RuntimeRotationProxyShared {
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        upstream_no_proxy: false,
        auto_redeem_enabled: false,
        compact_client: reqwest::Client::new(),
        async_client: reqwest::Client::new(),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state: AppState::default(),
            upstream_base_url: "http://127.0.0.1".to_string(),
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
            profile_health: BTreeMap::new(),
        })),
        log_path: env::temp_dir().join(format!(
            "prodex-affinity-test-{name}-{}-{unique}.log",
            std::process::id()
        )),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 8,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
            responses: 8,
            compact: 8,
            websocket: 8,
            standard: 8,
        }),
    }
}

fn runtime_proxy_affinity_add_profile(shared: &RuntimeRotationProxyShared, profile_name: &str) {
    let mut runtime = shared
        .runtime
        .lock()
        .expect("runtime test state should lock");
    let root = runtime.paths.root.clone();
    runtime.state.profiles.insert(
        profile_name.to_string(),
        ProfileEntry {
            codex_home: root.join(profile_name),
            managed: false,
            email: None,
            provider: ProfileProvider::Openai,
        },
    );
}

fn runtime_proxy_affinity_bind_session(
    shared: &RuntimeRotationProxyShared,
    session_id: &str,
    profile_name: &str,
) {
    shared
        .runtime
        .lock()
        .expect("runtime test state should lock")
        .session_id_bindings
        .insert(
            session_id.to_string(),
            ResponseProfileBinding {
                profile_name: profile_name.to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
}

fn runtime_proxy_affinity_bind_compact_turn_state(
    shared: &RuntimeRotationProxyShared,
    turn_state: &str,
    profile_name: &str,
) {
    shared
        .runtime
        .lock()
        .expect("runtime test state should lock")
        .turn_state_bindings
        .insert(
            runtime_compact_turn_state_lineage_key(turn_state),
            ResponseProfileBinding {
                profile_name: profile_name.to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
}

#[test]
fn prompt_cache_profile_bindings_prune_by_ttl_and_limit() {
    clear_runtime_prompt_cache_profile_bindings();
    let now = 1_000_000;
    remember_runtime_prompt_cache_profile_at(
        "main",
        Some("prompt-cache-expired"),
        now - RUNTIME_PROMPT_CACHE_AFFINITY_RETENTION_SECONDS - 1,
    );

    assert_eq!(
        runtime_prompt_cache_bound_profile_at(Some("prompt-cache-expired"), now),
        None
    );

    for index in 0..(RUNTIME_PROMPT_CACHE_AFFINITY_LIMIT + 2) {
        remember_runtime_prompt_cache_profile_at(
            "main",
            Some(&format!("prompt-cache-limit-{index}")),
            now + index as i64,
        );
    }

    let bindings = runtime_prompt_cache_profile_bindings()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(bindings.len(), RUNTIME_PROMPT_CACHE_AFFINITY_LIMIT);
    assert!(!bindings.contains_key("prompt-cache-limit-0"));
    assert!(bindings.contains_key(&format!(
        "prompt-cache-limit-{}",
        RUNTIME_PROMPT_CACHE_AFFINITY_LIMIT + 1
    )));
}

#[test]
fn prompt_cache_profile_hit_updates_owner_on_higher_cached_tokens() {
    clear_runtime_prompt_cache_profile_bindings();
    let now = 1_000_000;
    let prompt_cache_key = "prompt-cache-hit-owner";

    remember_runtime_prompt_cache_profile_at("main", Some(prompt_cache_key), now);
    assert_eq!(
        observe_runtime_prompt_cache_profile_hit_at("main", Some(prompt_cache_key), 128, now + 1),
        RuntimePromptCacheProfileObservation::OwnerUnchanged
    );
    assert_eq!(
        runtime_prompt_cache_bound_profile_at(Some(prompt_cache_key), now + 1).as_deref(),
        Some("main")
    );

    remember_runtime_prompt_cache_profile_at("second", Some(prompt_cache_key), now + 2);
    assert_eq!(
        runtime_prompt_cache_bound_profile_at(Some(prompt_cache_key), now + 2).as_deref(),
        Some("main")
    );

    assert_eq!(
        observe_runtime_prompt_cache_profile_hit_at("second", Some(prompt_cache_key), 64, now + 3),
        RuntimePromptCacheProfileObservation::ExistingOwnerPreserved
    );
    assert_eq!(
        runtime_prompt_cache_bound_profile_at(Some(prompt_cache_key), now + 3).as_deref(),
        Some("main")
    );

    assert_eq!(
        observe_runtime_prompt_cache_profile_hit_at("second", Some(prompt_cache_key), 256, now + 4,),
        RuntimePromptCacheProfileObservation::OwnerChanged
    );
    assert_eq!(
        runtime_prompt_cache_bound_profile_at(Some(prompt_cache_key), now + 4).as_deref(),
        Some("second")
    );
}

#[test]
fn prompt_cache_profile_hit_ignores_zero_or_absent_cached_tokens() {
    clear_runtime_prompt_cache_profile_bindings();
    let shared = runtime_proxy_affinity_test_shared("prompt-cache-zero-hit");
    let now = 1_000_000;
    let prompt_cache_key = "prompt-cache-zero-hit";

    remember_runtime_prompt_cache_profile_at("main", Some(prompt_cache_key), now);
    assert_eq!(
        observe_runtime_prompt_cache_profile_hit_at("second", Some(prompt_cache_key), 0, now + 1),
        RuntimePromptCacheProfileObservation::NoCachedTokens
    );
    assert_eq!(
        runtime_prompt_cache_bound_profile_at(Some(prompt_cache_key), now + 1).as_deref(),
        Some("main")
    );

    super::super::log_runtime_token_usage(super::super::RuntimeTokenUsageLog {
        shared: &shared,
        request_id: 1,
        transport: "http",
        profile_name: "second",
        source: "test",
        prompt_cache_key: Some(prompt_cache_key),
        model_name: None,
        usage: None,
    });
    assert_eq!(
        runtime_prompt_cache_bound_profile_at(Some(prompt_cache_key), now + 2).as_deref(),
        Some("main")
    );
}

#[test]
fn prompt_cache_token_usage_logs_bounded_redacted_cache_telemetry() {
    clear_runtime_prompt_cache_profile_bindings();
    let shared = runtime_proxy_affinity_test_shared("prompt-cache-token-usage-log");
    let prompt_cache_key = "raw-cache-key-must-not-leak";
    let expected_hash = runtime_proxy_crate::smart_context_hash_text(prompt_cache_key);

    super::super::log_runtime_token_usage(super::super::RuntimeTokenUsageLog {
        shared: &shared,
        request_id: 77,
        transport: "http",
        profile_name: "main",
        source: "unit-test",
        prompt_cache_key: Some(prompt_cache_key),
        model_name: None,
        usage: Some(RuntimeTokenUsage {
            input_tokens: 300,
            cached_input_tokens: 120,
            output_tokens: 40,
            reasoning_tokens: 7,
        }),
    });
    super::super::log_runtime_token_usage(super::super::RuntimeTokenUsageLog {
        shared: &shared,
        request_id: 78,
        transport: "websocket",
        profile_name: "main",
        source: "unit-test",
        prompt_cache_key: Some(prompt_cache_key),
        model_name: None,
        usage: Some(RuntimeTokenUsage {
            input_tokens: 320,
            cached_input_tokens: 100,
            output_tokens: 41,
            reasoning_tokens: 8,
        }),
    });

    let log = fs::read_to_string(&shared.log_path).expect("token usage log should exist");
    assert!(log.contains("token_usage"));
    assert!(log.contains("request=77"));
    assert!(log.contains("route=responses"));
    assert!(log.contains("profile=main"));
    assert!(log.contains("source=unit-test"));
    assert!(log.contains("prompt_cache_key=present"));
    assert!(log.contains(&format!("prompt_cache_key_hash={expected_hash}")));
    assert!(log.contains("prompt_cache_owner=owner_inserted"));
    assert!(log.contains("prompt_cache_owner=owner_unchanged"));
    assert!(log.contains("cached_input_tokens=120"));
    assert!(log.contains("uncached_input_tokens=180"));
    assert!(log.contains("transport=websocket"));
    assert!(!log.contains(prompt_cache_key));
}

#[test]
fn runtime_proxy_affinity_request_context_prefers_compact_turn_state_over_session() {
    let shared = runtime_proxy_affinity_test_shared("compact-turn-state");
    runtime_proxy_affinity_add_profile(&shared, "session-owner");
    runtime_proxy_affinity_add_profile(&shared, "compact-owner");
    runtime_proxy_affinity_bind_session(&shared, "session-1", "session-owner");
    runtime_proxy_affinity_bind_compact_turn_state(&shared, "turn-state-1", "compact-owner");

    let affinity = derive_runtime_response_route_affinity_for_request(
        &shared,
        RuntimeResponseRouteAffinityRequest {
            request_turn_state: Some("turn-state-1"),
            request_session_id: Some("session-1"),
            ..RuntimeResponseRouteAffinityRequest::default()
        },
    )
    .expect("route affinity should derive");

    assert_eq!(
        affinity.bound_session_profile.as_deref(),
        Some("session-owner")
    );
    assert_eq!(
        affinity.compact_followup_profile,
        Some(("compact-owner".to_string(), "turn_state"))
    );
    assert_eq!(affinity.session_profile, None);
    assert_eq!(affinity.pinned_profile.as_deref(), Some("compact-owner"));
}

#[test]
fn runtime_proxy_affinity_refresh_context_updates_slots_and_logs_presence() {
    let shared = runtime_proxy_affinity_test_shared("refresh");
    let mut bound_session_profile = Some("old-bound-session".to_string());
    let mut compact_followup_profile = Some(("old-compact".to_string(), "turn_state"));
    let mut compact_session_profile = Some("old-compact-session".to_string());
    let mut session_profile = Some("old-session".to_string());
    let mut pinned_profile = Some("old-pinned".to_string());

    refresh_and_log_runtime_response_route_affinity_for_request(
        RuntimeResponseRouteAffinityLogContext {
            shared: &shared,
            request_id: 42,
            websocket_session_id: Some(7),
            reason: "test_refresh",
        },
        RuntimeResponseRouteAffinityRequest {
            websocket_session_profile: Some("websocket-owner"),
            ..RuntimeResponseRouteAffinityRequest::default()
        },
        RuntimeResponseRouteAffinityRefreshSlots {
            bound_session_profile: &mut bound_session_profile,
            compact_followup_profile: &mut compact_followup_profile,
            compact_session_profile: &mut compact_session_profile,
            session_profile: &mut session_profile,
            pinned_profile: &mut pinned_profile,
        },
    )
    .expect("route affinity refresh should succeed");

    assert_eq!(bound_session_profile, None);
    assert_eq!(compact_followup_profile, None);
    assert_eq!(compact_session_profile, None);
    assert_eq!(session_profile.as_deref(), Some("websocket-owner"));
    assert_eq!(pinned_profile, None);

    let log = fs::read_to_string(&shared.log_path).expect("route affinity log should exist");
    assert!(
        log.contains("request=42 websocket_session=7 route_affinity_recompute reason=test_refresh")
    );
    assert!(log.contains("previous_response_id_present=false"));
    assert!(log.contains("request_turn_state_present=false"));
    assert!(log.contains("session_profile=Some(\"websocket-owner\")"));
}
