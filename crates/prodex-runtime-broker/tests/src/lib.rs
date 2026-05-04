use super::*;

fn test_registry() -> RuntimeBrokerRegistry {
    RuntimeBrokerRegistry {
        pid: 42,
        listen_addr: "127.0.0.1:4567".to_string(),
        started_at: 100,
        upstream_base_url: "https://upstream.example".to_string(),
        include_code_review: true,
        upstream_no_proxy: false,
        smart_context_enabled: false,
        current_profile: "work".to_string(),
        instance_token: "broker-token".to_string(),
        admin_token: "admin-token".to_string(),
        prodex_version: Some("0.7.0".to_string()),
        executable_path: Some("/tmp/prodex".to_string()),
        executable_sha256: Some("abc123".to_string()),
        openai_mount_path: Some("/backend-api/prodex".to_string()),
    }
}

#[test]
fn registry_builds_admin_urls_and_matches_launch_config() {
    let registry = test_registry();

    assert_eq!(
        RuntimeBrokerAdminRoute::from_path("/__prodex/runtime/metrics/prometheus"),
        Some(RuntimeBrokerAdminRoute::MetricsPrometheus)
    );
    assert_eq!(
        registry.health_url(),
        "http://127.0.0.1:4567/__prodex/runtime/health"
    );
    assert_eq!(
        registry.metrics_prometheus_url(),
        "http://127.0.0.1:4567/__prodex/runtime/metrics/prometheus"
    );
    assert!(registry.matches_launch_config("https://upstream.example", true, false, false));
    assert!(!registry.matches_launch_config("https://other.example", true, false, false));
    assert!(!registry.matches_launch_config("https://upstream.example", true, false, true));
}

#[test]
fn registry_json_defaults_smart_context_disabled() {
    let registry: RuntimeBrokerRegistry = serde_json::from_str(
        r#"{
            "pid": 42,
            "listen_addr": "127.0.0.1:4567",
            "started_at": 100,
            "upstream_base_url": "https://upstream.example",
            "include_code_review": true,
            "current_profile": "work",
            "instance_token": "broker-token",
            "admin_token": "admin-token"
        }"#,
    )
    .expect("legacy registry should deserialize");

    assert!(!registry.smart_context_enabled);
    assert!(registry.matches_launch_config("https://upstream.example", true, false, false));
}

#[test]
fn registry_helpers_format_mount_paths_targets_and_startup_grace() {
    let registry = test_registry();

    assert_eq!(
        runtime_broker_registry_openai_mount_path(&registry).as_deref(),
        Some("/backend-api/prodex")
    );
    assert_eq!(
        runtime_broker_legacy_openai_mount_path("/backend-api/prodex/v", "0.6.0"),
        "/backend-api/prodex/v0.6.0"
    );
    assert_eq!(format_runtime_broker_metrics_targets(&[]), "-");
    assert_eq!(
        format_runtime_broker_metrics_targets(&[
            "http://127.0.0.1:1/metrics".to_string(),
            "http://127.0.0.1:2/metrics".to_string(),
        ]),
        "http://127.0.0.1:1/metrics (+1 more)"
    );
    assert_eq!(runtime_broker_startup_grace_seconds(1_250, 5), 5);
    assert_eq!(runtime_broker_startup_grace_seconds(5_250, 1), 7);
}

#[test]
fn admin_helpers_plan_errors_and_activation_success() {
    assert_eq!(
        runtime_broker_validate_admin_token(Some("secret"), "secret"),
        Ok(())
    );
    assert_eq!(
        runtime_broker_validate_admin_token(None, "secret"),
        Err(runtime_broker_admin_forbidden_error())
    );
    assert_eq!(
        runtime_broker_validate_activation_method("GET"),
        Err(RuntimeBrokerAdminError::new(
            405,
            "method_not_allowed",
            "runtime broker activation requires POST",
        ))
    );
    assert_eq!(
        runtime_broker_validate_activation_profile(Some("  work  ")),
        Ok("work".to_string())
    );
    assert_eq!(
        runtime_broker_activation_profile_from_json(br#"{"current_profile":"  work  "}"#),
        Ok("work".to_string())
    );
    assert_eq!(
        runtime_broker_activation_profile_from_json(br#"{"current_profile":""}"#),
        Err(RuntimeBrokerAdminError::new(
            400,
            "invalid_request",
            "runtime broker activation requires a non-empty current_profile",
        ))
    );
    assert!(runtime_broker_validate_activation_profile(Some(" ")).is_err());
    assert_eq!(
        runtime_broker_activation_success("work"),
        RuntimeBrokerActivationSuccess {
            ok: true,
            current_profile: "work".to_string(),
        }
    );
}

#[test]
fn health_from_metadata_preserves_identity_fields() {
    let metadata = RuntimeBrokerMetadata {
        broker_key: "key".to_string(),
        listen_addr: "127.0.0.1:4567".to_string(),
        started_at: 100,
        current_profile: "work".to_string(),
        include_code_review: true,
        upstream_no_proxy: false,
        instance_token: "broker-token".to_string(),
        admin_token: "admin-token".to_string(),
        prodex_version: Some("0.7.0".to_string()),
        executable_path: Some("/tmp/prodex".to_string()),
        executable_sha256: Some("abc123".to_string()),
    };

    let health = RuntimeBrokerHealth::from_metadata(&metadata, 42, 3, true);

    assert_eq!(health.pid, 42);
    assert_eq!(health.active_requests, 3);
    assert_eq!(health.persistence_role, "owner");
    assert_eq!(health.current_profile, "work");
    assert_eq!(health.executable_sha256.as_deref(), Some("abc123"));
}

#[test]
fn registry_reuse_decision_requires_launch_match_and_matching_health() {
    let registry = test_registry();
    let launch_config = RuntimeBrokerLaunchConfig {
        upstream_base_url: "https://upstream.example",
        include_code_review: true,
        upstream_no_proxy: false,
        smart_context_enabled: false,
    };
    let health = RuntimeBrokerHealth {
        pid: registry.pid,
        started_at: registry.started_at,
        current_profile: registry.current_profile.clone(),
        include_code_review: registry.include_code_review,
        active_requests: 0,
        instance_token: registry.instance_token.clone(),
        persistence_role: "owner".to_string(),
        prodex_version: registry.prodex_version.clone(),
        executable_path: registry.executable_path.clone(),
        executable_sha256: registry.executable_sha256.clone(),
    };

    assert_eq!(
        runtime_broker_registry_reuse_decision(&registry, Some(&health), launch_config),
        RuntimeBrokerRegistryReuseDecision::Reuse
    );
    assert_eq!(
        runtime_broker_registry_reuse_decision(&registry, None, launch_config),
        RuntimeBrokerRegistryReuseDecision::MissingMatchingHealth
    );
    assert_eq!(
        runtime_broker_registry_reuse_decision(
            &registry,
            Some(&health),
            RuntimeBrokerLaunchConfig {
                upstream_base_url: "https://other.example",
                include_code_review: true,
                upstream_no_proxy: false,
                smart_context_enabled: false,
            },
        ),
        RuntimeBrokerRegistryReuseDecision::LaunchConfigMismatch
    );
    assert_eq!(
        runtime_broker_registry_reuse_decision(
            &registry,
            Some(&health),
            RuntimeBrokerLaunchConfig {
                upstream_base_url: "https://upstream.example",
                include_code_review: true,
                upstream_no_proxy: false,
                smart_context_enabled: true,
            },
        ),
        RuntimeBrokerRegistryReuseDecision::LaunchConfigMismatch
    );
}

#[test]
fn broker_process_args_encode_optional_boolean_switches() {
    let config = RuntimeBrokerSpawnConfig {
        current_profile: "default",
        upstream_base_url: "https://upstream.example",
        include_code_review: true,
        upstream_no_proxy: true,
        smart_context_enabled: true,
        model_context_window_tokens: Some(65_536),
        broker_key: "key",
        instance_token: "broker-token",
        admin_token: "admin-token",
        listen_addr: Some("127.0.0.1:4567"),
    };
    let args = runtime_broker_process_args(config);

    assert_eq!(args[0], OsString::from("__runtime-broker"));
    assert!(args.contains(&OsString::from("--include-code-review")));
    assert!(args.contains(&OsString::from("--upstream-no-proxy")));
    assert!(args.contains(&OsString::from("--smart-context")));
    assert!(args.contains(&OsString::from("--model-context-window-tokens")));
    assert!(args.contains(&OsString::from("65536")));
    assert!(args.contains(&OsString::from("--listen-addr")));
    assert!(args.contains(&OsString::from("127.0.0.1:4567")));

    let plan = runtime_broker_process_command_plan("/bin/prodex", "/tmp/prodex-home", config);

    assert_eq!(plan.executable, PathBuf::from("/bin/prodex"));
    assert_eq!(plan.prodex_home, PathBuf::from("/tmp/prodex-home"));
    assert!(plan.args.contains(&OsString::from("--instance-token")));

    let disabled_args = runtime_broker_process_args(RuntimeBrokerSpawnConfig {
        smart_context_enabled: false,
        listen_addr: None,
        ..config
    });
    assert!(!disabled_args.contains(&OsString::from("--smart-context")));
    assert!(!disabled_args.contains(&OsString::from("--model-context-window-tokens")));
}

#[test]
fn broker_key_is_scoped_to_smart_context_mode() {
    let normal = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        false,
        None,
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );
    let smart = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        true,
        None,
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );

    assert_ne!(normal, smart);
}

#[test]
fn broker_key_is_scoped_to_smart_context_window_when_enabled() {
    let default_window = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        true,
        None,
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );
    let custom_window = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        true,
        Some(65_536),
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );

    assert_ne!(default_window, custom_window);
}

#[test]
fn continuation_metrics_aggregate_lifecycle_signals_and_staleness() {
    let statuses = RuntimeContinuationStatuses {
        response: BTreeMap::from([
            (
                "resp-warm".to_string(),
                RuntimeContinuationBindingStatus {
                    state: RuntimeContinuationBindingLifecycle::Warm,
                    failure_count: 2,
                    not_found_streak: 1,
                    ..RuntimeContinuationBindingStatus::default()
                },
            ),
            (
                "resp-stale".to_string(),
                RuntimeContinuationBindingStatus {
                    state: RuntimeContinuationBindingLifecycle::Verified,
                    last_verified_at: Some(80),
                    failure_count: 3,
                    ..RuntimeContinuationBindingStatus::default()
                },
            ),
        ]),
        turn_state: BTreeMap::from([(
            "turn-suspect".to_string(),
            RuntimeContinuationBindingStatus {
                state: RuntimeContinuationBindingLifecycle::Suspect,
                not_found_streak: 4,
                ..RuntimeContinuationBindingStatus::default()
            },
        )]),
        session_id: BTreeMap::from([(
            "session-dead".to_string(),
            RuntimeContinuationBindingStatus {
                state: RuntimeContinuationBindingLifecycle::Dead,
                failure_count: 1,
                ..RuntimeContinuationBindingStatus::default()
            },
        )]),
    };

    let metrics = runtime_broker_continuation_metrics(&statuses, 100, 10);

    assert_eq!(metrics.response_bindings, 2);
    assert_eq!(metrics.turn_state_bindings, 1);
    assert_eq!(metrics.session_id_bindings, 1);
    assert_eq!(metrics.warm, 1);
    assert_eq!(metrics.verified, 1);
    assert_eq!(metrics.suspect, 1);
    assert_eq!(metrics.dead, 1);
    assert_eq!(
        metrics.failure_counts,
        RuntimeBrokerContinuationSignalMetrics {
            response: 5,
            turn_state: 0,
            session_id: 1,
        }
    );
    assert_eq!(
        metrics.not_found_streaks,
        RuntimeBrokerContinuationSignalMetrics {
            response: 1,
            turn_state: 4,
            session_id: 0,
        }
    );
    assert_eq!(
        metrics.stale_verified_bindings,
        RuntimeBrokerContinuationSignalMetrics {
            response: 1,
            turn_state: 0,
            session_id: 0,
        }
    );
}

#[test]
fn previous_response_continuity_metrics_count_active_known_routes_only() {
    let profile_health = BTreeMap::from([
        (
            "__previous_response_not_found__:responses:resp-1".to_string(),
            RuntimeProfileHealth {
                score: 4,
                updated_at: 90,
            },
        ),
        (
            "__previous_response_not_found__:compact:resp-2".to_string(),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 99,
            },
        ),
        (
            "__previous_response_not_found__:websocket:resp-3".to_string(),
            RuntimeProfileHealth {
                score: 1,
                updated_at: 0,
            },
        ),
        (
            "__previous_response_not_found__:unknown:resp-4".to_string(),
            RuntimeProfileHealth {
                score: 9,
                updated_at: 99,
            },
        ),
        (
            "main".to_string(),
            RuntimeProfileHealth {
                score: 9,
                updated_at: 99,
            },
        ),
    ]);

    let metrics = runtime_broker_previous_response_continuity_metrics(&profile_health, 100, 5);

    assert_eq!(
        metrics.negative_cache_entries,
        RuntimeBrokerRouteContinuityMetrics {
            responses: 1,
            compact: 1,
            websocket: 0,
            standard: 0,
        }
    );
    assert_eq!(
        metrics.negative_cache_failures,
        RuntimeBrokerRouteContinuityMetrics {
            responses: 2,
            compact: 2,
            websocket: 0,
            standard: 0,
        }
    );
}

#[test]
fn continuity_failure_reason_metrics_merge_and_subtract_saturating() {
    let mut metrics = RuntimeBrokerContinuityFailureReasonMetrics {
        chain_retried_owner: BTreeMap::from([
            ("previous_response_not_found".to_string(), 2),
            ("stale".to_string(), 1),
        ]),
        chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 3)]),
        stale_continuation: BTreeMap::from([("watchdog".to_string(), 1)]),
    };

    runtime_broker_merge_continuity_failure_reason_metrics(
        &mut metrics,
        RuntimeBrokerContinuityFailureReasonMetrics {
            chain_retried_owner: BTreeMap::from([
                ("previous_response_not_found".to_string(), 4),
                ("new".to_string(), 1),
            ]),
            chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 2)]),
            stale_continuation: BTreeMap::from([("watchdog".to_string(), 3)]),
        },
    );

    assert_eq!(
        metrics.chain_retried_owner,
        BTreeMap::from([
            ("new".to_string(), 1),
            ("previous_response_not_found".to_string(), 6),
            ("stale".to_string(), 1),
        ])
    );
    assert_eq!(
        runtime_broker_subtract_continuity_failure_reason_metrics(
            metrics,
            &RuntimeBrokerContinuityFailureReasonMetrics {
                chain_retried_owner: BTreeMap::from([
                    ("new".to_string(), 2),
                    ("previous_response_not_found".to_string(), 1),
                ]),
                chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 2)]),
                stale_continuation: BTreeMap::from([("watchdog".to_string(), 10)]),
            },
        ),
        RuntimeBrokerContinuityFailureReasonMetrics {
            chain_retried_owner: BTreeMap::from([
                ("previous_response_not_found".to_string(), 5),
                ("stale".to_string(), 1),
            ]),
            chain_dead_upstream_confirmed: BTreeMap::from([("dead".to_string(), 3)]),
            stale_continuation: BTreeMap::new(),
        }
    );
}

#[test]
fn continuity_failure_reason_metrics_parse_text_and_json_logs() {
    let log = br#"[2026-04-22 10:00:00.000 +00:00] request=3 chain_retried_owner profile=second reason=previous_response_not_found_locked_affinity
{"timestamp":"2026-04-22 10:00:01.000 +00:00","event":"stale_continuation","message":"stale_continuation reason=websocket_reuse_watchdog_locked_affinity","fields":{"reason":"websocket_reuse_watchdog_locked_affinity"}}
[2026-04-22 10:00:01.500 +00:00] request=3 ignored_marker reason=ignored
{"timestamp":"2026-04-22 10:00:01.750 +00:00","message":"chain_dead_upstream_confirmed reason=\"json message reason\"","fields":{}}
[2026-04-22 10:00:02.000 +00:00] request=3 chain_dead_upstream_confirmed reason="quoted reason"
"#;

    let metrics = runtime_broker_continuity_failure_reason_metrics_from_log_bytes(log);

    assert_eq!(
        metrics.chain_retried_owner,
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1,)])
    );
    assert_eq!(
        metrics.stale_continuation,
        BTreeMap::from([("websocket_reuse_watchdog_locked_affinity".to_string(), 1,)])
    );
    assert_eq!(
        metrics.chain_dead_upstream_confirmed,
        BTreeMap::from([
            ("json message reason".to_string(), 1),
            ("quoted reason".to_string(), 1),
        ])
    );
}

#[test]
fn continuity_failure_reason_metrics_merge_live_without_double_counting() {
    let parsed = RuntimeBrokerContinuityFailureReasonMetrics {
        stale_continuation: BTreeMap::from([("previous_response_not_found".to_string(), 2)]),
        ..RuntimeBrokerContinuityFailureReasonMetrics::default()
    };
    let baseline = RuntimeBrokerContinuityFailureReasonMetrics {
        stale_continuation: BTreeMap::from([("previous_response_not_found".to_string(), 1)]),
        ..RuntimeBrokerContinuityFailureReasonMetrics::default()
    };
    let live = RuntimeBrokerContinuityFailureReasonMetrics {
        stale_continuation: BTreeMap::from([
            ("previous_response_not_found".to_string(), 1),
            ("watchdog".to_string(), 2),
        ]),
        ..RuntimeBrokerContinuityFailureReasonMetrics::default()
    };

    let metrics =
        runtime_broker_continuity_failure_reason_metrics_with_live(parsed, &baseline, live);

    assert_eq!(
        metrics.stale_continuation,
        BTreeMap::from([
            ("previous_response_not_found".to_string(), 2),
            ("watchdog".to_string(), 2),
        ])
    );
}

#[test]
fn degraded_health_metrics_separates_profiles_and_routes() {
    let profile_health = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileHealth {
                score: 3,
                updated_at: 98,
            },
        ),
        (
            "__route_health__:responses:main".to_string(),
            RuntimeProfileHealth {
                score: 5,
                updated_at: 99,
            },
        ),
        (
            "__auth_failure__:main".to_string(),
            RuntimeProfileHealth {
                score: 5,
                updated_at: 99,
            },
        ),
        (
            "stale".to_string(),
            RuntimeProfileHealth {
                score: 1,
                updated_at: 90,
            },
        ),
    ]);

    assert_eq!(
        runtime_broker_degraded_health_metrics(&profile_health, 100, 2),
        RuntimeBrokerDegradedHealthMetrics {
            profiles: 1,
            routes: 1,
        }
    );
}

#[test]
fn metrics_snapshot_input_builds_broker_metrics_dto() {
    let metadata = RuntimeBrokerMetadata {
        broker_key: "key".to_string(),
        listen_addr: "127.0.0.1:4567".to_string(),
        started_at: 100,
        current_profile: "main".to_string(),
        include_code_review: false,
        upstream_no_proxy: false,
        instance_token: "instance".to_string(),
        admin_token: "admin".to_string(),
        prodex_version: Some("0.7.0".to_string()),
        executable_path: Some("/tmp/prodex".to_string()),
        executable_sha256: Some("abc123".to_string()),
    };
    let lane = RuntimeBrokerLaneMetrics {
        active: 1,
        limit: 4,
        admissions_total: 10,
        releases_total: 9,
        global_limit_rejections_total: 1,
        lane_limit_rejections_total: 2,
        release_underflows_total: 0,
    };
    let traffic = RuntimeBrokerTrafficMetrics {
        responses: lane.clone(),
        compact: lane.clone(),
        websocket: lane.clone(),
        standard: lane,
    };
    let profile_inflight = BTreeMap::from([("main".to_string(), 2)]);
    let profile_health = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileHealth {
                score: 3,
                updated_at: 99,
            },
        ),
        (
            "__route_health__:responses:main".to_string(),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 99,
            },
        ),
        (
            "__previous_response_not_found__:responses:resp-1".to_string(),
            RuntimeProfileHealth {
                score: 4,
                updated_at: 99,
            },
        ),
    ]);
    let continuation_statuses = RuntimeContinuationStatuses {
        response: BTreeMap::from([(
            "resp-1".to_string(),
            RuntimeContinuationBindingStatus {
                state: RuntimeContinuationBindingLifecycle::Verified,
                last_verified_at: Some(80),
                failure_count: 2,
                ..RuntimeContinuationBindingStatus::default()
            },
        )]),
        ..RuntimeContinuationStatuses::default()
    };

    let metrics = runtime_broker_metrics_from_snapshot_input(RuntimeBrokerMetricsSnapshotInput {
        metadata: &metadata,
        pid: 42,
        active_requests: 3,
        persistence_owner: true,
        active_request_limit: 8,
        local_overload_backoff_remaining_seconds: 5,
        runtime_state_lock_wait: RuntimeStateLockWaitMetrics {
            wait_total_ns: 10,
            wait_count: 1,
            wait_max_ns: 10,
        },
        traffic,
        profile_inflight: &profile_inflight,
        profile_retry_backoff_until: &BTreeMap::from([("main".to_string(), 101)]),
        profile_transport_backoff_until: &BTreeMap::from([("main".to_string(), 101)]),
        profile_route_circuit_open_until: &BTreeMap::from([("main".to_string(), 99)]),
        profile_health: &profile_health,
        continuation_statuses: &continuation_statuses,
        continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics {
            stale_continuation: BTreeMap::from([("watchdog".to_string(), 1)]),
            ..RuntimeBrokerContinuityFailureReasonMetrics::default()
        },
        now: 100,
        health_decay_seconds: 10,
        stale_verified_seconds: 10,
        previous_response_negative_cache_seconds: 10,
    })
    .with_guard_counters(RuntimeBrokerMetricsGuardCounters {
        active_request_release_underflows_total: 1,
        profile_inflight_admissions_total: 2,
        profile_inflight_releases_total: 3,
        profile_inflight_release_underflows_total: 4,
    });

    assert_eq!(metrics.health.pid, 42);
    assert_eq!(metrics.health.persistence_role, "owner");
    assert_eq!(metrics.health.active_requests, 3);
    assert_eq!(metrics.profile_inflight.get("main"), Some(&2));
    assert_eq!(metrics.retry_backoffs, 1);
    assert_eq!(metrics.transport_backoffs, 1);
    assert_eq!(metrics.route_circuits, 0);
    assert_eq!(metrics.degraded_profiles, 1);
    assert_eq!(metrics.degraded_routes, 1);
    assert_eq!(metrics.continuations.verified, 1);
    assert_eq!(metrics.continuations.stale_verified_bindings.response, 1);
    assert_eq!(
        metrics
            .previous_response_continuity
            .negative_cache_entries
            .responses,
        1
    );
    assert_eq!(metrics.active_request_release_underflows_total, 1);
    assert_eq!(
        metrics
            .continuity_failure_reasons
            .stale_continuation
            .get("watchdog"),
        Some(&1)
    );
}

#[test]
fn prodex_binary_identity_key_prefers_version_and_sha() {
    let identity = RuntimeProdexBinaryIdentity {
        prodex_version: Some("0.7.0".to_string()),
        executable_path: Some(PathBuf::from("/tmp/prodex")),
        executable_sha256: Some("abc123".to_string()),
    };

    assert_eq!(
        runtime_prodex_binary_identity_key(&identity),
        "version=0.7.0;sha256=abc123"
    );
}

#[test]
fn prodex_binary_identity_match_prefers_sha_then_version() {
    let current = RuntimeProdexBinaryIdentity {
        prodex_version: Some("0.7.0".to_string()),
        executable_path: None,
        executable_sha256: Some("abc123".to_string()),
    };
    let other_same_sha = RuntimeProdexBinaryIdentity {
        prodex_version: Some("0.8.0".to_string()),
        executable_path: None,
        executable_sha256: Some("abc123".to_string()),
    };
    let other_same_version = RuntimeProdexBinaryIdentity {
        prodex_version: Some("0.7.0".to_string()),
        executable_path: None,
        executable_sha256: None,
    };

    assert!(runtime_prodex_binary_identity_matches(
        &current,
        &other_same_sha
    ));
    assert!(runtime_prodex_binary_identity_matches(
        &current,
        &other_same_version
    ));
}

#[test]
fn observed_binary_identity_prefers_matching_health_then_registry_then_process() {
    let mut registry = test_registry();
    registry.executable_sha256 = None;
    let process_identity = RuntimeProdexBinaryIdentity {
        prodex_version: Some("0.9.0".to_string()),
        executable_path: None,
        executable_sha256: Some("process-sha".to_string()),
    };
    let health = RuntimeBrokerHealth {
        pid: 42,
        started_at: 100,
        current_profile: "work".to_string(),
        include_code_review: true,
        active_requests: 0,
        instance_token: "broker-token".to_string(),
        persistence_role: "owner".to_string(),
        prodex_version: Some("0.8.0".to_string()),
        executable_path: None,
        executable_sha256: Some("health-sha".to_string()),
    };

    let observed =
        runtime_broker_observed_binary_identity(&registry, Some(&health), Some(&process_identity));

    assert_eq!(observed.prodex_version.as_deref(), Some("0.8.0"));
    assert_eq!(observed.executable_sha256.as_deref(), Some("health-sha"));

    let mismatched_health = RuntimeBrokerHealth {
        instance_token: "other-token".to_string(),
        ..health
    };
    let observed = runtime_broker_observed_binary_identity(
        &registry,
        Some(&mismatched_health),
        Some(&process_identity),
    );

    assert_eq!(observed.prodex_version.as_deref(), Some("0.7.0"));
    assert_eq!(observed.executable_path, Some(PathBuf::from("/tmp/prodex")));
}

#[test]
fn replacement_reason_prefers_sha_then_version_then_presence() {
    let current = RuntimeProdexBinaryIdentity {
        prodex_version: Some("0.7.0".to_string()),
        executable_path: None,
        executable_sha256: Some("abc123".to_string()),
    };

    assert_eq!(
        runtime_broker_replacement_reason(
            &current,
            &RuntimeProdexBinaryIdentity {
                executable_sha256: Some("def456".to_string()),
                ..RuntimeProdexBinaryIdentity::default()
            },
        ),
        "sha256_mismatch"
    );
    assert_eq!(
        runtime_broker_replacement_reason(
            &current,
            &RuntimeProdexBinaryIdentity {
                prodex_version: Some("0.8.0".to_string()),
                ..RuntimeProdexBinaryIdentity::default()
            },
        ),
        "version_mismatch"
    );
    assert_eq!(
        runtime_broker_replacement_reason(&current, &RuntimeProdexBinaryIdentity::default()),
        "identity_unresolved"
    );
}

#[test]
fn version_guard_decision_defers_active_then_replaces() {
    let current_binary = RuntimeProdexBinaryIdentity {
        prodex_version: Some("0.7.0".to_string()),
        executable_path: None,
        executable_sha256: Some("abc123".to_string()),
    };
    let current_version = RuntimeProdexBinaryIdentity {
        prodex_version: Some("0.7.0".to_string()),
        executable_path: None,
        executable_sha256: None,
    };
    let observed = RuntimeProdexBinaryIdentity {
        prodex_version: Some("0.8.0".to_string()),
        executable_path: None,
        executable_sha256: Some("def456".to_string()),
    };

    let decision = runtime_broker_version_guard_decision(
        true,
        &current_binary,
        &current_version,
        &observed,
        1,
        0,
    );
    assert_eq!(
        decision.outcome,
        RuntimeBrokerVersionGuardOutcome::DeferredActiveRequests
    );
    assert_eq!(decision.current_identity.executable_sha256, None);

    let decision = runtime_broker_version_guard_decision(
        true,
        &current_binary,
        &current_version,
        &observed,
        0,
        0,
    );
    assert_eq!(decision.outcome, RuntimeBrokerVersionGuardOutcome::Replaced);
    assert_eq!(decision.replacement_reason, Some("version_mismatch"));
}

#[test]
fn parse_prodex_version_output_requires_prodex_binary_name() {
    assert_eq!(
        parse_prodex_version_output("prodex 0.7.0\n"),
        Some("0.7.0".to_string())
    );
    assert_eq!(parse_prodex_version_output("codex 0.7.0\n"), None);
    assert_eq!(parse_prodex_version_output("prodex\n"), None);
}
