#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::{
    ProfileBackoffMergeAction, ProfileHealthScoreInput, ProfileRecoveryCandidate,
    RUNTIME_PROFILE_SCHEDULE_MAX_COUNT, profile_backoff_merge_action,
    profile_circuit_half_open_seconds, profile_health_sort_key_batch, profile_recovery_plan_batch,
};

#[test]
fn half_open_circuit_timing_abi_caps_mojo_policy_boundaries() {
    for (score, base, maximum, expected) in [
        (0, 5, 60, 5),
        (4, 5, 60, 5),
        (5, 5, 60, 10),
        (7, 5, 60, 40),
        (u32::MAX, 10, 60, 60),
    ] {
        assert_eq!(
            profile_circuit_half_open_seconds(score, 4, base, maximum)
                .expect("valid half-open circuit input"),
            expected,
            "score={score}"
        );
    }
    assert!(profile_circuit_half_open_seconds(0, 4, 5, -1).is_err());
}

#[test]
fn profile_health_batch_matches_boundary_expectations() {
    let inputs = [
        ProfileHealthScoreInput {
            global_score: 1,
            global_updated_at: 100,
            route_health_score: 2,
            route_health_updated_at: 100,
            route_bad_pairing_score: 3,
            route_bad_pairing_updated_at: 100,
            coupled_health_score: 4,
            coupled_health_updated_at: 100,
            coupled_bad_pairing_score: 2,
            coupled_bad_pairing_updated_at: 100,
            route_performance_score: 8,
            route_performance_updated_at: 100,
            coupled_performance_score: 4,
            coupled_performance_updated_at: 100,
        },
        ProfileHealthScoreInput {
            global_score: u32::MAX,
            global_updated_at: i64::MIN,
            route_health_score: 0,
            route_health_updated_at: i64::MAX,
            route_bad_pairing_score: 5,
            route_bad_pairing_updated_at: 99,
            coupled_health_score: 7,
            coupled_health_updated_at: 99,
            coupled_bad_pairing_score: 1,
            coupled_bad_pairing_updated_at: 99,
            route_performance_score: 9,
            route_performance_updated_at: 99,
            coupled_performance_score: 3,
            coupled_performance_updated_at: 99,
        },
    ];
    let actual =
        profile_health_sort_key_batch(&inputs, 102, 2, 4, 8).expect("valid profile health batch");
    assert_eq!(actual, [16, 18]);
}

#[test]
fn profile_health_batch_rejects_more_than_abi_capacity() {
    let input = ProfileHealthScoreInput {
        global_score: 1,
        global_updated_at: 0,
        route_health_score: 0,
        route_health_updated_at: 0,
        route_bad_pairing_score: 0,
        route_bad_pairing_updated_at: 0,
        coupled_health_score: 0,
        coupled_health_updated_at: 0,
        coupled_bad_pairing_score: 0,
        coupled_bad_pairing_updated_at: 0,
        route_performance_score: 0,
        route_performance_updated_at: 0,
        coupled_performance_score: 0,
        coupled_performance_updated_at: 0,
    };
    let inputs = vec![input; 257];
    assert!(profile_health_sort_key_batch(&inputs, 0, 2, 4, 8).is_err());
}

#[test]
fn backoff_merge_action_handles_missing_ties_and_signed_extremes() {
    for (existing, incoming, value_present, expected) in [
        (None, None, true, ProfileBackoffMergeAction::Insert),
        (
            None,
            Some(i64::MIN),
            true,
            ProfileBackoffMergeAction::Insert,
        ),
        (Some(10), Some(11), true, ProfileBackoffMergeAction::Insert),
        (Some(10), Some(10), true, ProfileBackoffMergeAction::Insert),
        (Some(11), Some(10), true, ProfileBackoffMergeAction::Keep),
        (
            Some(i64::MIN),
            Some(i64::MAX),
            true,
            ProfileBackoffMergeAction::Insert,
        ),
        (
            Some(i64::MAX),
            Some(i64::MIN),
            true,
            ProfileBackoffMergeAction::Keep,
        ),
        (
            Some(i64::MAX),
            Some(i64::MAX),
            false,
            ProfileBackoffMergeAction::Remove,
        ),
    ] {
        assert_eq!(
            profile_backoff_merge_action(existing, incoming, value_present)
                .expect("valid backoff update timestamps"),
            expected,
            "existing={existing:?} incoming={incoming:?} value_present={value_present}"
        );
    }
}

#[test]
fn recovery_batch_uses_latest_profile_blocker_and_earliest_profile_across_chunks() {
    let healthy = ProfileRecoveryCandidate {
        eligible: true,
        retry_until: None,
        transport_until: None,
        circuit_until: None,
    };
    let mut inputs = vec![healthy; RUNTIME_PROFILE_SCHEDULE_MAX_COUNT];
    inputs[0] = ProfileRecoveryCandidate {
        eligible: true,
        retry_until: Some(90),
        transport_until: Some(120),
        circuit_until: Some(80),
    };
    inputs[1] = ProfileRecoveryCandidate {
        eligible: true,
        retry_until: None,
        transport_until: Some(110),
        circuit_until: None,
    };
    inputs[2] = ProfileRecoveryCandidate {
        eligible: false,
        retry_until: Some(60),
        transport_until: None,
        circuit_until: None,
    };
    inputs[3] = ProfileRecoveryCandidate {
        eligible: true,
        retry_until: Some(49),
        transport_until: None,
        circuit_until: None,
    };
    inputs.push(ProfileRecoveryCandidate {
        eligible: true,
        retry_until: None,
        transport_until: None,
        circuit_until: Some(80),
    });

    let plan = profile_recovery_plan_batch(&inputs, 50).expect("valid recovery batch");
    assert_eq!(plan.can_clear.len(), inputs.len());
    assert_eq!(&plan.can_clear[..4], &[false, false, false, true]);
    assert!(!plan.can_clear[RUNTIME_PROFILE_SCHEDULE_MAX_COUNT]);
    assert_eq!(plan.earliest_recovery_at, Some(80));
}
