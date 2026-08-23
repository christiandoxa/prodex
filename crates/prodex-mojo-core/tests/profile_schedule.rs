#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::{ProfileScheduleInput, ProfileScoreInput, profile_schedule_batch};

fn input(
    order_index: i64,
    provider_priority: i64,
    total_inputs: (i64, i64, i64, i64, i64, bool, i64),
    state: (bool, i64, i64, i64, i64, bool, bool),
) -> ProfileScheduleInput {
    let (
        weekly_pressure,
        five_hour_pressure,
        scale_bps,
        weekly_remaining,
        five_hour_remaining,
        windows_complete,
        weekly_weight,
    ) = total_inputs;
    let (
        in_selection_cooldown,
        last_selected_at,
        weekly_reset_at,
        five_hour_reset_at,
        quota_source,
        preferred,
        affinity_preferred,
    ) = state;
    ProfileScheduleInput {
        score: ProfileScoreInput {
            weekly_pressure,
            five_hour_pressure,
            scale_bps,
            weekly_remaining,
            five_hour_remaining,
            windows_complete,
            weekly_weight,
        },
        provider_priority,
        in_selection_cooldown,
        last_selected_at,
        weekly_reset_at,
        five_hour_reset_at,
        quota_source,
        preferred,
        affinity_preferred,
        order_index,
    }
}

fn score(input: &ProfileScheduleInput) -> (i64, i64, i64, i64) {
    let scale = |pressure: i64| {
        if pressure == i64::MAX {
            return i64::MAX;
        }
        pressure
            .saturating_mul(input.score.scale_bps.max(0))
            .checked_div(10_000)
            .unwrap_or(i64::MAX)
    };
    let weekly = scale(input.score.weekly_pressure);
    let five_hour = scale(input.score.five_hour_pressure);
    let reserve_bias = if !input.score.windows_complete
        || input.score.weekly_remaining == 0
        || input.score.five_hour_remaining == 0
    {
        i64::MAX / 4
    } else if input.score.weekly_remaining <= 10 || input.score.five_hour_remaining <= 5 {
        1_000_000
    } else if input.score.weekly_remaining <= 20 || input.score.five_hour_remaining <= 10 {
        250_000
    } else {
        0
    };
    (
        reserve_bias
            .saturating_add(weekly.saturating_mul(input.score.weekly_weight))
            .saturating_add(five_hour),
        weekly,
        five_hour,
        input
            .score
            .weekly_remaining
            .min(input.score.five_hour_remaining),
    )
}

fn within_bps(candidate: i64, best: i64, bps: i64) -> bool {
    candidate <= best
        || (best > 0
            && i128::from(candidate) * 10_000 <= i128::from(best) * i128::from(10_000 + bps))
}

fn ordered_by_oracle(inputs: &[ProfileScheduleInput]) -> Vec<usize> {
    let scores = inputs.iter().map(score).collect::<Vec<_>>();
    let best_provider = inputs
        .iter()
        .map(|input| input.provider_priority)
        .min()
        .unwrap_or(i64::MAX);
    let best_total = inputs
        .iter()
        .enumerate()
        .filter(|(_, input)| input.provider_priority == best_provider)
        .map(|(index, _)| scores[index].0)
        .min()
        .unwrap_or(i64::MAX);
    let near = |index: usize| {
        inputs[index].provider_priority == best_provider
            && within_bps(scores[index].0, best_total, 1_000)
    };
    let mut order = (0..inputs.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        let left_near = near(*left);
        let right_near = near(*right);
        let left_key = [
            (
                inputs[*left].provider_priority,
                inputs[*right].provider_priority,
            ),
            (i64::from(!left_near), i64::from(!right_near)),
            (
                i64::from(left_near && inputs[*left].in_selection_cooldown),
                i64::from(right_near && inputs[*right].in_selection_cooldown),
            ),
            (
                if left_near {
                    inputs[*left].last_selected_at
                } else {
                    i64::MIN
                },
                if right_near {
                    inputs[*right].last_selected_at
                } else {
                    i64::MIN
                },
            ),
            (scores[*left].0, scores[*right].0),
            (scores[*left].1, scores[*right].1),
            (scores[*left].2, scores[*right].2),
            (scores[*right].3, scores[*left].3),
            (
                inputs[*left].weekly_reset_at,
                inputs[*right].weekly_reset_at,
            ),
            (
                inputs[*left].five_hour_reset_at,
                inputs[*right].five_hour_reset_at,
            ),
            (inputs[*left].quota_source, inputs[*right].quota_source),
            (
                i64::from(!inputs[*left].preferred),
                i64::from(!inputs[*right].preferred),
            ),
            (inputs[*left].order_index, inputs[*right].order_index),
        ];
        for (left_value, right_value) in left_key {
            if left_value != right_value {
                return left_value.cmp(&right_value);
            }
        }
        left.cmp(right)
    });

    if let Some(position) = order.iter().position(|index| {
        inputs[*index].affinity_preferred && !inputs[*index].in_selection_cooldown
    }) && position > 0
    {
        let affinity = order[position];
        let selected = order[0];
        if inputs[affinity].provider_priority == inputs[selected].provider_priority
            && within_bps(scores[affinity].0, scores[selected].0, 500)
        {
            order.remove(position);
            order.insert(0, affinity);
        }
    }
    order
}

#[test]
fn profile_schedule_matches_rust_oracle_and_handles_boundaries() {
    let inputs = vec![
        input(
            0,
            0,
            (100, 0, 10_000, 90, 90, true, 10),
            (true, 100, 30, 40, 0, false, false),
        ),
        input(
            1,
            0,
            (105, 0, 10_000, 80, 80, true, 10),
            (false, 90, 20, 30, 1, true, true),
        ),
        input(
            2,
            0,
            (110, 0, 10_000, 70, 70, true, 10),
            (false, 80, 10, 20, 0, false, false),
        ),
        input(
            3,
            1,
            (1, 1, 20_000, 100, 100, true, 8),
            (false, i64::MIN, i64::MAX, i64::MAX, 0, false, false),
        ),
        input(
            4,
            2,
            (i64::MAX - 1, 0, 20_000, 50, 50, false, 1),
            (false, i64::MIN, i64::MAX, i64::MAX, 1, false, false),
        ),
    ];
    let expected = ordered_by_oracle(&inputs);
    let actual = profile_schedule_batch(&inputs).expect("valid schedule inputs");
    assert_eq!(actual, expected);

    assert_eq!(profile_schedule_batch(&[]).unwrap(), Vec::<usize>::new());

    let boundary = |order_index, weekly_remaining, five_hour_remaining, windows_complete| {
        input(
            order_index,
            0,
            (
                0,
                0,
                10_000,
                weekly_remaining,
                five_hour_remaining,
                windows_complete,
                10,
            ),
            (false, i64::MIN, 10, 10, 0, false, false),
        )
    };
    let boundaries = [
        boundary(0, 90, 90, true),
        boundary(1, 20, 10, true),
        boundary(2, 10, 5, true),
        boundary(3, 0, 90, true),
        boundary(4, 90, 90, false),
    ];
    assert_eq!(
        profile_schedule_batch(&boundaries).expect("valid pressure boundaries"),
        [0, 1, 2, 4, 3]
    );
}

#[test]
fn profile_schedule_prefers_preferred_candidate_on_tie() {
    let candidates = [
        input(
            0,
            0,
            (100, 100, 10_000, 90, 90, true, 10),
            (false, 0, 10, 10, 0, false, false),
        ),
        input(
            0,
            0,
            (100, 100, 10_000, 90, 90, true, 10),
            (false, 0, 10, 10, 0, true, false),
        ),
    ];

    assert_eq!(profile_schedule_batch(&candidates).unwrap(), [1, 0]);
}
