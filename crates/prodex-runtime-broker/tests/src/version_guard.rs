use std::path::PathBuf;

use super::*;

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
