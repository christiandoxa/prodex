#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::{ProfileHealthScoreInput, profile_health_sort_key_batch};

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
