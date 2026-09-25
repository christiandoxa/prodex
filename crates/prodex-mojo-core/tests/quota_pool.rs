#![cfg(feature = "mojo-quota")]

use prodex_mojo_core::{
    MojoError,
    quota::{
        MainQuotaAggregation, MainQuotaAggregationInput, QUOTA_MAIN_AGGREGATION_MAX_COUNT,
        main_quota_aggregate_batch,
    },
    quota_pool::*,
};

#[test]
fn openai_pool_aggregate_matches_fixed_expected_values() {
    let window = |remaining_percent, reset_at| {
        Some(QuotaPoolWindowInput {
            remaining_percent,
            reset_at,
        })
    };
    let inputs = [
        OpenAiQuotaPoolInput {
            five_hour: window(80, 50),
            weekly: window(95, i64::MAX),
            ready: true,
        },
        OpenAiQuotaPoolInput {
            five_hour: window(0, 40),
            weekly: None,
            ready: false,
        },
        OpenAiQuotaPoolInput {
            five_hour: None,
            weekly: window(25, -5),
            ready: true,
        },
        OpenAiQuotaPoolInput {
            five_hour: None,
            weekly: None,
            ready: true,
        },
    ];

    assert_eq!(
        openai_quota_pool_aggregate(&inputs),
        Ok(OpenAiQuotaPoolAggregation {
            profiles_with_data: 3,
            ready_profiles_with_data: 2,
            five_hour_profiles_with_data: 2,
            weekly_profiles_with_data: 2,
            ready_five_hour_profiles_with_data: 1,
            ready_weekly_profiles_with_data: 2,
            five_hour_pool_remaining: 80,
            weekly_pool_remaining: 120,
            ready_five_hour_pool_remaining: 80,
            ready_weekly_pool_remaining: 120,
            earliest_five_hour_reset_at: Some(40),
            earliest_weekly_reset_at: Some(-5),
        })
    );
}

#[test]
fn main_pool_aggregate_matches_fixed_expected_values_and_keeps_its_bound() {
    let inputs = [
        MainQuotaAggregationInput {
            remaining_percent: Some(30),
            reset_at: Some(90),
        },
        MainQuotaAggregationInput {
            remaining_percent: None,
            reset_at: Some(1),
        },
        MainQuotaAggregationInput {
            remaining_percent: Some(70),
            reset_at: Some(40),
        },
        MainQuotaAggregationInput {
            remaining_percent: None,
            reset_at: None,
        },
    ];
    assert_eq!(
        main_quota_aggregate_batch(&inputs),
        Ok(MainQuotaAggregation {
            profiles_with_data: 2,
            pool_remaining: 100,
            earliest_reset_at: Some(40),
        })
    );

    let input = MainQuotaAggregationInput {
        remaining_percent: Some(1),
        reset_at: None,
    };
    let max_rows = vec![input; QUOTA_MAIN_AGGREGATION_MAX_COUNT];
    assert_eq!(
        main_quota_aggregate_batch(&max_rows),
        Ok(MainQuotaAggregation {
            profiles_with_data: 1_024,
            pool_remaining: 1_024,
            earliest_reset_at: None,
        })
    );
    assert_eq!(
        main_quota_aggregate_batch(&[input; QUOTA_MAIN_AGGREGATION_MAX_COUNT + 1]),
        Err(MojoError::InvalidInput)
    );
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
