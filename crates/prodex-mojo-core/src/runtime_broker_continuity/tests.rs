use super::*;

use super::*;

#[test]
fn broker_continuity_kernel_smoke() {
    let plan = continuity_line_plan(
        "request=3 chain_retried_owner profile=second reason=\"quoted reason\"",
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(plan.event, Some(ContinuityEvent::ChainRetriedOwner));
    assert_eq!(plan.reason_source, Some(ContinuityReasonSource::RawLine));

    assert_eq!(effective_score(4, 90, 100, 5).unwrap(), 2);
    assert!(stale_verified(true, None, Some(80), None, 100, 10).unwrap());
    assert_eq!(
        route_kind("websocket").unwrap(),
        ContinuityRouteKind::Websocket
    );
    assert_eq!(
        health_key_kind("__route_health__:responses:main").unwrap(),
        HealthKeyKind::Route
    );
    let current = BrokerBinaryIdentityView {
        version: Some("0.7.0"),
        sha256: Some("abc123"),
        path_present: true,
    };
    let same_sha = BrokerBinaryIdentityView {
        version: Some("0.8.0"),
        sha256: Some("abc123"),
        path_present: false,
    };
    let different = BrokerBinaryIdentityView {
        version: Some("0.8.0"),
        sha256: Some("def456"),
        path_present: false,
    };
    assert!(binary_identity_present(current).unwrap());
    assert!(binary_identity_matches(current, same_sha).unwrap());
    assert_eq!(
        binary_identity_replacement_reason(current, different).unwrap(),
        BrokerReplacementReason::Sha256Mismatch
    );
    assert!(binary_identity_version_mismatch(current, different).unwrap());
    assert_eq!(
        version_guard_plan(
            true,
            current,
            BrokerBinaryIdentityView {
                version: Some("0.7.0"),
                sha256: None,
                path_present: false,
            },
            different,
            0,
            0
        )
        .unwrap(),
        BrokerVersionGuardPlan {
            outcome: BrokerVersionGuardOutcome::Replaced,
            use_version_identity: true,
            replacement_reason: Some(BrokerReplacementReason::VersionMismatch),
        }
    );
    assert_eq!(
        parse_prodex_version(
            "  prodex 0.7.0
"
        )
        .unwrap(),
        Some("0.7.0")
    );
    assert_eq!(parse_prodex_version("codex 0.7.0").unwrap(), None);
    assert_eq!(
        continuity_event_kind("chain_dead_upstream_confirmed").unwrap(),
        Some(ContinuityEvent::ChainDeadUpstreamConfirmed)
    );
    assert_eq!(
        log_fingerprint_relation(20, 2, 0, Some((10, 1, 0))).unwrap(),
        BrokerLogFingerprintRelation::Append
    );
    assert_eq!(
        log_fingerprint_relation(9, 2, 0, Some((10, 1, 0))).unwrap(),
        BrokerLogFingerprintRelation::Rotated
    );
    assert_eq!(lru_evict_index(&[5, 2, 9], Some(1)).unwrap(), Some(0));
    assert_eq!(lru_evict_index(&[5], Some(0)).unwrap(), Some(0));
    assert_eq!(lru_evict_index(&[], None).unwrap(), None);
}
