use super::*;
use std::collections::BTreeMap;

#[test]
fn continuation_store_compaction_removes_orphan_lineage_bindings() {
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
                    profile_name: "alpha".to_string(),
                    bound_at: 100,
                },
            ),
            (
                keep_lineage_key.clone(),
                ResponseProfileBinding {
                    profile_name: "alpha".to_string(),
                    bound_at: 100,
                },
            ),
            (
                lineage_key.clone(),
                ResponseProfileBinding {
                    profile_name: "alpha".to_string(),
                    bound_at: 100,
                },
            ),
            (
                "missing-profile".to_string(),
                ResponseProfileBinding {
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
        !compacted
            .response_profile_bindings
            .contains_key("missing-profile")
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
                profile_name: "alpha".to_string(),
                bound_at: 100,
            },
        ),
        (
            lineage_key,
            ResponseProfileBinding {
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
