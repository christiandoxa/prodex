use super::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn continuation_store_compaction_removes_orphan_lineage_bindings_but_keeps_unknown_owners() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let lineage_key = format!(
        "{}{}:{}:{}",
        RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX,
        "missing-parent".len(),
        "missing-parent",
        "turn"
    );
    let keep_lineage_key = format!(
        "{}{}:{}:{}",
        RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX,
        "resp".len(),
        "resp",
        "turn"
    );
    let continuations = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([
            (
                "resp".to_string(),
                ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: "alpha".to_string(),
                    bound_at: 100,
                },
            ),
            (
                keep_lineage_key.clone(),
                ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: "alpha".to_string(),
                    bound_at: 100,
                },
            ),
            (
                lineage_key.clone(),
                ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: "alpha".to_string(),
                    bound_at: 100,
                },
            ),
            (
                "missing-profile".to_string(),
                ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: "missing".to_string(),
                    bound_at: 100,
                },
            ),
        ]),
        ..RuntimeContinuationStore::default()
    };

    let compacted = compact_runtime_continuation_store(
        continuations,
        &profiles,
        200,
        RuntimeContinuationCompactionPolicy::default(),
    );

    assert!(compacted.response_profile_bindings.contains_key("resp"));
    assert!(
        compacted
            .response_profile_bindings
            .contains_key(&keep_lineage_key)
    );
    assert!(
        !compacted
            .response_profile_bindings
            .contains_key(&lineage_key)
    );
    assert!(
        compacted
            .response_profile_bindings
            .contains_key("missing-profile")
    );
    assert_eq!(
        runtime_hard_binding_owner(
            &RuntimeHardBindingIdentity::response("missing-profile").unwrap(),
            &compacted.response_profile_bindings,
            &compacted.turn_state_bindings,
            &compacted.session_id_bindings,
            &compacted.session_profile_bindings,
            &profiles,
        ),
        RuntimeHardBindingOwner::Unavailable("missing".to_string())
    );
}

#[test]
fn continuation_status_helpers_touch_verify_and_mark_suspect() {
    let policy = RuntimeContinuationStatusPolicy {
        touch_persist_interval_seconds: 10,
        suspect_grace_seconds: 5,
        suspect_not_found_streak_limit: 2,
        confidence_max: 8,
        verified_confidence_bonus: 2,
        touch_confidence_bonus: 1,
        suspect_confidence_penalty: 1,
    };
    let mut statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-1",
        100,
        Some("responses"),
        policy,
    ));
    let status = statuses.response.get("resp-1").expect("verified status");
    assert_eq!(status.state, RuntimeContinuationBindingLifecycle::Verified);
    assert_eq!(status.confidence, 2);
    assert_eq!(status.last_verified_route.as_deref(), Some("responses"));
    assert!(!runtime_continuation_status_should_refresh_verified(
        Some(status),
        105,
        Some("responses"),
        policy,
    ));
    assert!(runtime_continuation_status_should_refresh_verified(
        Some(status),
        111,
        Some("responses"),
        policy,
    ));

    assert!(runtime_mark_continuation_status_suspect(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-1",
        112,
        policy,
    ));
    assert!(runtime_continuation_status_recently_suspect(
        statuses.response.get("resp-1"),
        113,
        policy,
    ));
    assert!(runtime_mark_continuation_status_suspect(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-1",
        114,
        policy,
    ));
    assert_eq!(
        statuses.response.get("resp-1").map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn lineage_key_helpers_round_trip_and_filter_internal_keys() {
    let lineage_key = runtime_response_turn_state_lineage_key("resp:with:colon", "turn");
    assert_eq!(
        runtime_response_turn_state_lineage_parts(&lineage_key),
        Some(("resp:with:colon", "turn"))
    );
    assert_eq!(
        runtime_compact_session_lineage_key("session"),
        "__compact_session__:session"
    );
    assert_eq!(
        runtime_compact_turn_state_lineage_key("turn"),
        "__compact_turn_state__:turn"
    );

    let bindings = BTreeMap::from([
        (
            "external".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "alpha".to_string(),
                bound_at: 100,
            },
        ),
        (
            lineage_key,
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "alpha".to_string(),
                bound_at: 100,
            },
        ),
    ]);

    assert_eq!(
        runtime_external_response_profile_bindings(&bindings)
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["external".to_string()]
    );
}

#[test]
fn hard_binding_identity_round_trips_without_debugging_identifier_values() {
    let identity =
        RuntimeHardBindingIdentity::new(Some("response-1"), Some("turn-1"), Some("session-1"))
            .expect("identity should contain a continuation component");
    let encoded = serde_json::to_string(&identity).expect("identity should serialize");
    let decoded: RuntimeHardBindingIdentity =
        serde_json::from_str(&encoded).expect("identity should deserialize");

    assert_eq!(decoded, identity);
    let debug = format!("{identity:?}");
    assert!(!debug.contains("response-1"));
    assert!(!debug.contains("turn-1"));
    assert!(!debug.contains("session-1"));
}

#[test]
fn hard_binding_identity_accepts_legacy_empty_optional_fields() {
    let decoded: RuntimeHardBindingIdentity =
        serde_json::from_str(r#"{"response_id":"", "turn_state":" ", "session_id":null}"#)
            .expect("legacy empty identity fields should remain readable");
    assert_eq!(decoded, RuntimeHardBindingIdentity::default());
}

fn public_binding_identity(seed: char) -> RuntimeProviderBindingIdentity {
    serde_json::from_value(serde_json::json!({
        "provider": "openai",
        "credential_identity": format!("sha256:{}", seed.to_string().repeat(64)),
        "endpoint_identity": format!("sha256:{}", "b".repeat(64)),
        "profile_identity": format!("sha256:{}", "a".repeat(64)),
    }))
    .expect("fixture identity should validate")
}

#[test]
fn persisted_binding_resolution_preserves_legacy_uncertainty_and_exact_identity() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let exact = ResponseProfileBinding {
        profile_name: "alpha".to_string(),
        bound_at: 10,
        binding_identity: Some(public_binding_identity('1')),
    };
    let legacy = binding("alpha", 10);
    let response_bindings = BTreeMap::from([("response".to_string(), exact.clone())]);
    let resolution = runtime_hard_binding_resolution(
        &RuntimeHardBindingIdentity::response("response").unwrap(),
        &response_bindings,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &profiles,
    );
    assert_eq!(
        resolution.binding_identity(),
        exact.binding_identity.as_ref()
    );
    assert!(matches!(
        resolution,
        RuntimeHardBindingResolution::Owned { .. }
    ));

    let legacy_resolution = runtime_hard_binding_resolution(
        &RuntimeHardBindingIdentity::response("legacy").unwrap(),
        &BTreeMap::from([("legacy".to_string(), legacy)]),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &profiles,
    );
    assert!(legacy_resolution.binding_identity().is_none());
}

#[test]
fn persisted_binding_resolution_rejects_conflicting_exact_identities() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let response = BTreeMap::from([(
        "response".to_string(),
        ResponseProfileBinding {
            profile_name: "alpha".to_string(),
            bound_at: 1,
            binding_identity: Some(public_binding_identity('1')),
        },
    )]);
    let turn = BTreeMap::from([(
        "turn".to_string(),
        ResponseProfileBinding {
            profile_name: "alpha".to_string(),
            bound_at: 2,
            binding_identity: Some(public_binding_identity('2')),
        },
    )]);
    assert!(matches!(
        runtime_hard_binding_resolution(
            &RuntimeHardBindingIdentity::new(Some("response"), Some("turn"), None).unwrap(),
            &response,
            &turn,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &profiles,
        ),
        RuntimeHardBindingResolution::Conflict
    ));
}

#[test]
fn lineage_keys_and_deserialized_identities_reject_unbounded_values() {
    let oversized = "x".repeat(RUNTIME_HARD_BINDING_COMPONENT_MAX_BYTES + 1);
    assert!(RuntimeHardBindingIdentity::response(&oversized).is_none());
    assert!(
        runtime_compact_session_lineage_key(&oversized).len() <= RUNTIME_HARD_BINDING_KEY_MAX_BYTES
    );
    assert!(
        serde_json::from_str::<RuntimeHardBindingIdentity>(&format!(
            r#"{{"response_id":"{oversized}"}}"#
        ))
        .is_err()
    );
}

#[test]
fn compaction_converts_unbounded_profile_identity_to_fail_closed_conflict() {
    let oversized_profile =
        "profile-".to_string() + &"x".repeat(RUNTIME_HARD_BINDING_COMPONENT_MAX_BYTES);
    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "response".to_string(),
                binding(&oversized_profile, 1),
            )]),
            ..RuntimeContinuationStore::default()
        },
        &BTreeMap::new(),
        10,
        RuntimeContinuationCompactionPolicy::default(),
    );

    assert_eq!(
        compacted.response_profile_bindings["response"].profile_name,
        prodex_state::HARD_BINDING_CONFLICT_PROFILE
    );
}

#[test]
fn hard_binding_owner_detects_conflicts_and_unavailable_profiles() {
    let profiles = BTreeMap::from([
        ("alpha".to_string(), profile()),
        (
            "beta".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/profile-beta"),
                ..profile()
            },
        ),
    ]);
    let identity =
        RuntimeHardBindingIdentity::new(Some("response"), Some("turn"), Some("session")).unwrap();
    let response = BTreeMap::from([("response".to_string(), binding("alpha", 1))]);
    let turn = BTreeMap::from([("turn".to_string(), binding("beta", 1))]);

    let shared_owner = BTreeMap::from([("response".to_string(), binding("alpha", 1))]);
    let shared_turn = BTreeMap::from([("turn".to_string(), binding("alpha", 1))]);
    let shared_session = BTreeMap::from([("session".to_string(), binding("alpha", 1))]);
    assert_eq!(
        runtime_hard_binding_owner(
            &identity,
            &shared_owner,
            &shared_turn,
            &shared_session,
            &BTreeMap::new(),
            &profiles,
        ),
        RuntimeHardBindingOwner::Owned("alpha".to_string())
    );

    assert_eq!(
        runtime_hard_binding_owner(
            &identity,
            &response,
            &turn,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &profiles,
        ),
        RuntimeHardBindingOwner::Conflict
    );

    let unavailable = BTreeMap::from([("response".to_string(), binding("missing", 1))]);
    assert_eq!(
        runtime_hard_binding_owner(
            &RuntimeHardBindingIdentity::response("response").unwrap(),
            &unavailable,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &profiles,
        ),
        RuntimeHardBindingOwner::Unavailable("missing".to_string())
    );

    let unavailable_turn = BTreeMap::from([("turn".to_string(), binding("also-missing", 1))]);
    assert_eq!(
        runtime_hard_binding_owner(
            &RuntimeHardBindingIdentity::new(Some("response"), Some("turn"), None).unwrap(),
            &unavailable,
            &unavailable_turn,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &profiles,
        ),
        RuntimeHardBindingOwner::Conflict
    );
}

#[test]
fn hard_binding_merge_marks_conflicts_and_compaction_evicts_oldest() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let existing = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([("same".to_string(), binding("alpha", 1))]),
        ..RuntimeContinuationStore::default()
    };
    let incoming = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([("same".to_string(), binding("missing", 2))]),
        ..RuntimeContinuationStore::default()
    };
    let merged = merge_runtime_continuation_store(
        &existing,
        &incoming,
        &profiles,
        10,
        RuntimeContinuationCompactionPolicy::default(),
    );
    assert_eq!(
        merged.response_profile_bindings["same"].profile_name,
        prodex_state::HARD_BINDING_CONFLICT_PROFILE
    );

    let legacy = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([("legacy".to_string(), binding("missing", 2))]),
        ..RuntimeContinuationStore::default()
    };
    let merged_legacy = merge_runtime_continuation_store(
        &RuntimeContinuationStore::default(),
        &legacy,
        &profiles,
        10,
        RuntimeContinuationCompactionPolicy::default(),
    );
    assert_eq!(
        merged_legacy.response_profile_bindings["legacy"].profile_name,
        "missing"
    );

    let mut continuations = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([
            ("old".to_string(), binding("alpha", 1)),
            ("middle".to_string(), binding("alpha", 2)),
            ("new".to_string(), binding("alpha", 3)),
            (
                "conflict".to_string(),
                binding(prodex_state::HARD_BINDING_CONFLICT_PROFILE, 0),
            ),
        ]),
        ..RuntimeContinuationStore::default()
    };
    let policy = RuntimeContinuationCompactionPolicy {
        response_binding_limit: 2,
        ..RuntimeContinuationCompactionPolicy::default()
    };
    continuations = compact_runtime_continuation_store(continuations, &profiles, 10, policy);

    assert!(continuations.response_profile_bindings.contains_key("new"));
    assert!(
        continuations
            .response_profile_bindings
            .contains_key("conflict")
    );
    assert!(!continuations.response_profile_bindings.contains_key("old"));
}

fn binding(profile_name: &str, bound_at: i64) -> ResponseProfileBinding {
    ResponseProfileBinding {
        binding_identity: None,
        profile_name: profile_name.to_string(),
        bound_at,
    }
}
