use super::*;

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
