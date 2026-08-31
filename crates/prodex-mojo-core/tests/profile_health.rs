#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::{ProfileHealthScoreInput, profile_health_sort_key_batch};

fn effective(score: u32, updated_at: i64, now: i64, decay_seconds: i64) -> u32 {
    let elapsed = if now <= updated_at {
        0
    } else {
        now.saturating_sub(updated_at)
    };
    score.saturating_sub((elapsed / decay_seconds.max(1)).min(u32::MAX as i64) as u32)
}

fn expected(
    input: ProfileHealthScoreInput,
    now: i64,
    health_decay: i64,
    bad_pairing_decay: i64,
    performance_decay: i64,
) -> u32 {
    let effective_score = |score, updated_at, decay| effective(score, updated_at, now, decay);
    let coupling = effective_score(
        input.coupled_health_score,
        input.coupled_health_updated_at,
        health_decay,
    )
    .saturating_add(effective_score(
        input.coupled_bad_pairing_score,
        input.coupled_bad_pairing_updated_at,
        bad_pairing_decay,
    )) / 2;
    effective_score(input.global_score, input.global_updated_at, health_decay)
        .saturating_add(effective_score(
            input.route_health_score,
            input.route_health_updated_at,
            health_decay,
        ))
        .saturating_add(effective_score(
            input.route_bad_pairing_score,
            input.route_bad_pairing_updated_at,
            bad_pairing_decay,
        ))
        .saturating_add(coupling)
        .saturating_add(effective_score(
            input.route_performance_score,
            input.route_performance_updated_at,
            performance_decay,
        ))
        .saturating_add(
            effective_score(
                input.coupled_performance_score,
                input.coupled_performance_updated_at,
                performance_decay,
            ) / 2,
        )
}

#[test]
fn profile_health_batch_matches_rust_boundary_oracle() {
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
        profile_health_sort_key_batch(&inputs, 100, 2, 4, 8).expect("valid profile health batch");
    let expected = inputs
        .iter()
        .copied()
        .map(|input| expected(input, 100, 2, 4, 8))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
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
