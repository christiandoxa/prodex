use super::*;

#[test]
fn runtime_load_plan_keeps_latest_per_log_counts_and_stable_order() {
    let observations = [
        InfoRunwayObservation {
            timestamp: 100,
            profile: "β",
            five_hour_remaining: 0,
            weekly_remaining: 0,
        },
        InfoRunwayObservation {
            timestamp: 99,
            profile: "a",
            five_hour_remaining: 0,
            weekly_remaining: 0,
        },
        InfoRunwayObservation {
            timestamp: 100,
            profile: "a",
            five_hour_remaining: 0,
            weekly_remaining: 0,
        },
        InfoRunwayObservation {
            timestamp: 100,
            profile: "a",
            five_hour_remaining: 0,
            weekly_remaining: 0,
        },
        InfoRunwayObservation {
            timestamp: -1,
            profile: "old",
            five_hour_remaining: 0,
            weekly_remaining: 0,
        },
    ];
    let inflight = [
        InfoRuntimeInflight {
            log: 0,
            profile: "a",
            units: 5,
        },
        InfoRuntimeInflight {
            log: 0,
            profile: "β",
            units: 1,
        },
        InfoRuntimeInflight {
            log: 1,
            profile: "a",
            units: 7,
        },
        InfoRuntimeInflight {
            log: 0,
            profile: "a",
            units: 2,
        },
    ];
    let config = InfoRuntimeLoadConfig {
        now: 100,
        recent_window_seconds: 0,
        lookback_seconds: 1,
    };
    assert_eq!(
        info_runtime_load_plan(&observations, &inflight, config),
        Ok(InfoRuntimeLoadPlan {
            observation_indices: vec![1, 2, 3, 0],
            active_inflight_units: 10,
            recent_selection_events: 3,
            recent_first_timestamp: Some(100),
            recent_last_timestamp: Some(100),
        })
    );
    assert_eq!(
        info_runtime_load_plan(&[], &[], config),
        Ok(InfoRuntimeLoadPlan {
            observation_indices: vec![],
            active_inflight_units: 0,
            recent_selection_events: 0,
            recent_first_timestamp: None,
            recent_last_timestamp: None,
        })
    );
}

#[test]
fn runtime_load_abi_rejects_malformed_records_before_output_writes() {
    let mut output = [71_u64; 6];
    let malformed = LoadRow {
        group: 1,
        timestamp: 0,
        units: 0,
        profile: InfoStringView { ptr: 0, len: 0 },
    };
    for (abi, rows, words, expected) in [
        (2, 0, 6, 4),
        (1, -1, 6, 1),
        (1, 0, 4, 3),
        (1, 1, 6, 1),
        (1, i64::MAX, 6, 1),
        (1, 0, -1, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_info_runtime_load_plan_v1(
                    abi,
                    (&malformed as *const LoadRow) as u64,
                    rows,
                    0,
                    0,
                    0,
                    0,
                    0,
                    output.as_mut_ptr() as u64,
                    words,
                )
            },
            expected
        );
        assert_eq!(output, [71; 6]);
    }
}

#[test]
fn runtime_load_windows_saturate_at_signed_timestamp_limits() {
    let observations = [i64::MIN, i64::MAX].map(|timestamp| InfoRunwayObservation {
        timestamp,
        profile: "fixture",
        five_hour_remaining: 0,
        weekly_remaining: 0,
    });
    let config = InfoRuntimeLoadConfig {
        now: i64::MIN,
        recent_window_seconds: 1,
        lookback_seconds: 1,
    };
    assert_eq!(
        info_runtime_load_plan(&observations, &[], config),
        Ok(InfoRuntimeLoadPlan {
            observation_indices: vec![0, 1],
            active_inflight_units: 0,
            recent_selection_events: 2,
            recent_first_timestamp: Some(i64::MIN),
            recent_last_timestamp: Some(i64::MAX),
        })
    );
    assert_eq!(
        info_runtime_load_plan(
            &observations,
            &[],
            InfoRuntimeLoadConfig {
                now: i64::MAX,
                recent_window_seconds: -1,
                lookback_seconds: -1,
            }
        ),
        Ok(InfoRuntimeLoadPlan {
            observation_indices: vec![1],
            active_inflight_units: 0,
            recent_selection_events: 1,
            recent_first_timestamp: Some(i64::MAX),
            recent_last_timestamp: Some(i64::MAX),
        })
    );
}

#[test]
fn runtime_load_plan_handles_large_reverse_ordered_observations() {
    let observations = (0..4096)
        .map(|index| InfoRunwayObservation {
            timestamp: 4095 - index,
            profile: "fixture",
            five_hour_remaining: 0,
            weekly_remaining: 0,
        })
        .collect::<Vec<_>>();
    let plan = info_runtime_load_plan(
        &observations,
        &[],
        InfoRuntimeLoadConfig {
            now: 4095,
            recent_window_seconds: 4095,
            lookback_seconds: 4095,
        },
    )
    .unwrap();
    assert_eq!(
        plan,
        InfoRuntimeLoadPlan {
            observation_indices: (0..4096).rev().collect(),
            active_inflight_units: 0,
            recent_selection_events: 4096,
            recent_first_timestamp: Some(0),
            recent_last_timestamp: Some(4095),
        }
    );
}
