#![cfg(feature = "mojo-quota")]

use prodex_mojo_core::{MojoError, quota_pool::*};

fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    *state
}

fn oracle(inputs: &[OpenAiQuotaPoolInput]) -> OpenAiQuotaPoolAggregation {
    let mut output = OpenAiQuotaPoolAggregation::default();
    for input in inputs {
        if input.five_hour.is_none() && input.weekly.is_none() {
            continue;
        }
        output.profiles_with_data += 1;
        if input.ready {
            output.ready_profiles_with_data += 1;
        }
        if let Some(window) = input.five_hour {
            output.five_hour_profiles_with_data += 1;
            output.five_hour_pool_remaining += window.remaining_percent;
            if input.ready {
                output.ready_five_hour_profiles_with_data += 1;
                output.ready_five_hour_pool_remaining += window.remaining_percent;
            }
            if window.reset_at != i64::MAX {
                output.earliest_five_hour_reset_at = Some(
                    output
                        .earliest_five_hour_reset_at
                        .map_or(window.reset_at, |current| current.min(window.reset_at)),
                );
            }
        }
        if let Some(window) = input.weekly {
            output.weekly_profiles_with_data += 1;
            output.weekly_pool_remaining += window.remaining_percent;
            if input.ready {
                output.ready_weekly_profiles_with_data += 1;
                output.ready_weekly_pool_remaining += window.remaining_percent;
            }
            if window.reset_at != i64::MAX {
                output.earliest_weekly_reset_at = Some(
                    output
                        .earliest_weekly_reset_at
                        .map_or(window.reset_at, |current| current.min(window.reset_at)),
                );
            }
        }
    }
    output
}

#[test]
fn openai_pool_aggregate_matches_rust_oracle_for_generated_rows() {
    let mut state = 0x7175_6f74_615f_706f_u64;
    for case in 0..2_000 {
        let count = (next(&mut state) % 65) as usize;
        let mut inputs = Vec::with_capacity(count);
        for _ in 0..count {
            let make_window = |state: &mut u64| {
                (next(state) & 3 != 0).then(|| QuotaPoolWindowInput {
                    remaining_percent: (next(state) % 101) as i64,
                    reset_at: if next(state).is_multiple_of(13) {
                        i64::MAX
                    } else {
                        (next(state) % 20_001) as i64 - 10_000
                    },
                })
            };
            inputs.push(OpenAiQuotaPoolInput {
                five_hour: make_window(&mut state),
                weekly: make_window(&mut state),
                ready: next(&mut state) & 1 != 0,
            });
        }
        assert_eq!(
            openai_quota_pool_aggregate(&inputs),
            Ok(oracle(&inputs)),
            "case={case}"
        );
    }
}

#[test]
fn openai_pool_aggregate_has_no_1024_profile_cap() {
    let input = OpenAiQuotaPoolInput {
        five_hour: Some(QuotaPoolWindowInput {
            remaining_percent: 100,
            reset_at: i64::MAX,
        }),
        weekly: Some(QuotaPoolWindowInput {
            remaining_percent: 75,
            reset_at: 10,
        }),
        ready: true,
    };
    let aggregate = openai_quota_pool_aggregate(&[input; 2_048]).unwrap();
    assert_eq!(aggregate.profiles_with_data, 2_048);
    assert_eq!(aggregate.ready_profiles_with_data, 2_048);
    assert_eq!(aggregate.five_hour_pool_remaining, 204_800);
    assert_eq!(aggregate.weekly_pool_remaining, 153_600);
    assert_eq!(aggregate.earliest_five_hour_reset_at, None);
    assert_eq!(aggregate.earliest_weekly_reset_at, Some(10));
}

#[test]
fn openai_pool_aggregate_rejects_invalid_normalized_percent() {
    let input = OpenAiQuotaPoolInput {
        five_hour: Some(QuotaPoolWindowInput {
            remaining_percent: 101,
            reset_at: i64::MAX,
        }),
        weekly: None,
        ready: false,
    };
    assert_eq!(
        openai_quota_pool_aggregate(&[input]),
        Err(MojoError::InvalidInput)
    );
}
