use super::*;

#[test]
fn quota_summary_log_formatter_preserves_extremes_and_rejects_invalid_labels() {
    assert_eq!(
        format_quota_summary_log_fields(QuotaSummaryLogFields {
            pressure_band: "quota_unknown",
            five_hour_status: "critical",
            five_hour_remaining: i64::MIN,
            five_hour_reset_at: i64::MAX,
            weekly_status: "exhausted",
            weekly_remaining: 0,
            weekly_reset_at: -1,
        })
        .unwrap(),
        "quota_band=quota_unknown five_hour_status=critical five_hour_remaining=-9223372036854775808 five_hour_reset_at=9223372036854775807 weekly_status=exhausted weekly_remaining=0 weekly_reset_at=-1",
    );

    let too_long = "z".repeat(65);
    assert_eq!(
        format_quota_summary_log_fields(QuotaSummaryLogFields {
            pressure_band: &too_long,
            five_hour_status: "critical",
            five_hour_remaining: 10,
            five_hour_reset_at: 20,
            weekly_status: "ready",
            weekly_remaining: 30,
            weekly_reset_at: 40,
        }),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        format_quota_summary_log_fields(QuotaSummaryLogFields {
            pressure_band: "quota_ready",
            five_hour_status: "",
            five_hour_remaining: 10,
            five_hour_reset_at: 20,
            weekly_status: "ready",
            weekly_remaining: 30,
            weekly_reset_at: 40,
        }),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn usage_snapshot_retention_matches_saturating_boundary_contract() {
    let boundary = [
        i64::MIN,
        i64::MIN + 1,
        -1000,
        -1,
        0,
        1,
        120,
        i64::MAX - 1,
        i64::MAX,
    ];
    for profile_present in [false, true] {
        for &now in &boundary {
            for &checked_at in &boundary {
                for &retention_seconds in &boundary {
                    let expected =
                        profile_present && checked_at >= now.saturating_sub(retention_seconds);
                    let actual = usage_snapshot_should_retain(
                        profile_present,
                        checked_at,
                        now,
                        retention_seconds,
                    )
                    .expect("Mojo retention policy must produce a boolean");
                    assert_eq!(
                        actual, expected,
                        "profile_present={profile_present}, now={now}, checked_at={checked_at}, retention_seconds={retention_seconds}"
                    );
                }
            }
        }
    }
}

fn snapshot_fact() -> RuntimeQuotaSnapshotCompareFacts {
    RuntimeQuotaSnapshotCompareFacts {
        previous_present: true,
        previous_five_hour_remaining: 100,
        previous_five_hour_reset_at: i64::MIN,
        previous_weekly_remaining: 0,
        previous_weekly_reset_at: i64::MAX,
        next_five_hour_remaining: 100,
        next_five_hour_reset_at: i64::MIN,
        next_weekly_remaining: 0,
        next_weekly_reset_at: i64::MAX,
        five_hour_status_equal: true,
        weekly_status_equal: true,
        plan_type_equal: true,
        previous_checked_at: 100,
        now: 110,
        touch_interval_seconds: 60,
    }
}

#[test]
fn compiled_quota_snapshot_compare_detects_individual_differences_and_stale_touches() {
    let initial = snapshot_fact();
    assert_eq!(
        snapshot_compare_plan(initial, true),
        Ok(RuntimeQuotaSnapshotComparePlan {
            materially_matches: true,
            should_persist: false,
        }),
    );
    let mut stale = initial;
    stale.now = 161;
    assert_eq!(
        snapshot_compare_plan(stale, true),
        Ok(RuntimeQuotaSnapshotComparePlan {
            materially_matches: true,
            should_persist: true,
        }),
    );
    let mut missing = initial;
    missing.previous_present = false;
    assert_eq!(
        snapshot_compare_plan(missing, true),
        Ok(RuntimeQuotaSnapshotComparePlan {
            materially_matches: false,
            should_persist: true,
        }),
    );
    for mutate in [
        |f: &mut RuntimeQuotaSnapshotCompareFacts| f.next_five_hour_remaining = 99,
        |f: &mut RuntimeQuotaSnapshotCompareFacts| f.next_five_hour_reset_at += 1,
        |f: &mut RuntimeQuotaSnapshotCompareFacts| f.next_weekly_remaining = 1,
        |f: &mut RuntimeQuotaSnapshotCompareFacts| f.next_weekly_reset_at -= 1,
        |f: &mut RuntimeQuotaSnapshotCompareFacts| f.five_hour_status_equal = false,
        |f: &mut RuntimeQuotaSnapshotCompareFacts| f.weekly_status_equal = false,
        |f: &mut RuntimeQuotaSnapshotCompareFacts| f.plan_type_equal = false,
    ] {
        let mut changed = initial;
        mutate(&mut changed);
        assert_eq!(
            snapshot_compare_plan(changed, true),
            Ok(RuntimeQuotaSnapshotComparePlan {
                materially_matches: false,
                should_persist: true,
            }),
        );
    }
    assert_eq!(
        snapshot_compare_plan(initial, false),
        Ok(RuntimeQuotaSnapshotComparePlan {
            materially_matches: true,
            should_persist: false,
        }),
    );
    let mut saturated = initial;
    saturated.previous_checked_at = i64::MIN;
    saturated.now = i64::MAX;
    saturated.touch_interval_seconds = i64::MAX;
    assert_eq!(
        snapshot_compare_plan(saturated, true)
            .unwrap()
            .should_persist,
        false
    );
}

#[test]
fn compiled_quota_snapshot_compare_abi_rejects_shape_and_boolean_flags_before_writing() {
    let mut inputs = [0_i64; 15];
    let mut output = [73_i64; 2];
    let source = inputs.as_ptr() as usize as u64;
    let result = output.as_mut_ptr() as usize as u64;
    for (version, operation, address, count, expected) in [
        (2, 0, source, 15, 4),
        (1, -1, source, 15, 1),
        (1, 2, source, 15, 1),
        (1, 0, 0, 15, 1),
        (1, 0, source, 14, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_runtime_state_quota_snapshot_compare_v1(
                    version, operation, address, count, result,
                )
            },
            expected,
        );
        assert_eq!(output, [73; 2]);
    }
    for index in 8..12 {
        inputs[index] = 2;
        assert_eq!(
            unsafe {
                prodex_runtime_state_quota_snapshot_compare_v1(
                    1,
                    0,
                    inputs.as_ptr() as usize as u64,
                    15,
                    result,
                )
            },
            1,
        );
        assert_eq!(output, [73; 2]);
        inputs[index] = 0;
    }
    assert_eq!(
        unsafe { prodex_runtime_state_quota_snapshot_compare_v1(1, 0, source, 15, 0) },
        1,
    );
    assert_eq!(output, [73; 2]);
}

#[test]
fn compiled_startup_probe_batch_retains_order_and_warm_budget() {
    let input = [
        StartupProbeRefreshObservation {
            probe_fresh: true,
            snapshot_usable: false,
        },
        StartupProbeRefreshObservation {
            probe_fresh: false,
            snapshot_usable: true,
        },
        StartupProbeRefreshObservation {
            probe_fresh: false,
            snapshot_usable: false,
        },
        StartupProbeRefreshObservation {
            probe_fresh: false,
            snapshot_usable: false,
        },
        StartupProbeRefreshObservation {
            probe_fresh: false,
            snapshot_usable: false,
        },
    ];
    assert_eq!(
        startup_probe_refresh_batch(&input, 2),
        Ok(StartupProbeRefreshBatchPlan {
            selected: vec![false, false, true, true, false],
            remaining: 0,
        }),
    );
    assert_eq!(
        startup_probe_refresh_batch(&input, 0),
        Ok(StartupProbeRefreshBatchPlan {
            selected: vec![false; 5],
            remaining: 0,
        }),
    );
    assert_eq!(
        startup_probe_refresh_batch(&[], 10),
        Ok(StartupProbeRefreshBatchPlan {
            selected: Vec::new(),
            remaining: 10,
        }),
    );
    let mut many = vec![
        StartupProbeRefreshObservation {
            probe_fresh: true,
            snapshot_usable: true
        };
        65_536
    ];
    many[65_534] = StartupProbeRefreshObservation {
        probe_fresh: false,
        snapshot_usable: false,
    };
    many[65_535] = many[65_534];
    let plan = startup_probe_refresh_batch(&many, 1).unwrap();
    assert_eq!(plan.selected.len(), 65_536);
    assert_eq!(plan.selected.iter().filter(|flag| **flag).count(), 1);
    assert!(plan.selected[65_534]);
    assert!(!plan.selected[65_535]);
    assert_eq!(plan.remaining, 0);
    many.push(many[0]);
    assert_eq!(
        startup_probe_refresh_batch(&many, 1),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        startup_probe_refresh_batch(&input, usize::MAX),
        Err(MojoError::InvalidInput),
    );
}

#[test]
fn compiled_startup_probe_batch_abi_rejects_late_invalid_row_without_writes() {
    let mut rows = [0_i64, 0, 1, 0, 0, 0];
    let mut output = [83_i64; 3];
    let mut remaining = 79_i64;
    let source = rows.as_ptr() as usize as u64;
    let destination = output.as_mut_ptr() as usize as u64;
    let rest = (&mut remaining as *mut i64) as usize as u64;
    for (version, fields, count, warm, capacity, remaining_ptr, expected) in [
        (2, source, 3, 2, 3, rest, 4),
        (1, 0, 3, 2, 3, rest, 1),
        (1, source, -1, 2, 3, rest, 1),
        (1, source, 65_537, 2, 3, rest, 1),
        (1, source, 3, -1, 3, rest, 1),
        (1, source, 3, 2, 2, rest, 1),
        (1, source, 3, 2, 3, 0, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_runtime_state_startup_probe_batch_v1(
                    version,
                    fields,
                    count,
                    warm,
                    destination,
                    capacity,
                    remaining_ptr,
                )
            },
            expected,
        );
        assert_eq!(output, [83; 3]);
        assert_eq!(remaining, 79);
    }
    rows[5] = 2;
    assert_eq!(
        unsafe {
            prodex_runtime_state_startup_probe_batch_v1(
                1,
                rows.as_ptr() as usize as u64,
                3,
                2,
                destination,
                3,
                rest,
            )
        },
        1,
    );
    assert_eq!(output, [83; 3]);
    assert_eq!(remaining, 79);
}

#[test]
fn startup_probe_refresh_candidate_plan_matches_independent_oracle() {
    for probe_fresh in [false, true] {
        for snapshot_usable in [false, true] {
            for remaining in [0, 1, 2, 17, i64::MAX as usize] {
                let selected = remaining > 0 && !probe_fresh && !snapshot_usable;
                let expected = StartupProbeRefreshCandidatePlan {
                    selected,
                    remaining: remaining.saturating_sub(usize::from(selected)),
                };
                assert_eq!(
                    startup_probe_refresh_candidate_plan(probe_fresh, snapshot_usable, remaining,)
                        .expect("Mojo startup probe plan must preserve the boundary contract"),
                    expected,
                    "probe_fresh={probe_fresh} snapshot_usable={snapshot_usable} remaining={remaining}",
                );
            }
        }
    }
}

#[test]
fn runtime_state_quota_kernel_smoke() {
    assert!(timestamp_touch_should_persist(0, 11, 10).unwrap());
    assert!(usage_snapshot_should_retain(true, 130, 250, 120).unwrap());
    assert!(!usage_snapshot_should_retain(true, 129, 250, 120).unwrap());
    assert!(!usage_snapshot_should_retain(false, 200, 250, 120).unwrap());
    assert!(usage_snapshot_should_retain(true, i64::MIN, i64::MIN, i64::MAX).unwrap());
    assert_eq!(
        probe_cache_freshness(90, 100, 5, 20).unwrap(),
        ProbeCacheFreshness::StaleUsable
    );
    assert_eq!(
        snapshot_usability(true, 120, false, i64::MAX, 0, 100, 10).unwrap(),
        SnapshotUsabilityPlan {
            usable: true,
            hold_active: true,
            hold_expired: false,
        }
    );
    let plan =
        probe_usage_snapshot_apply_plan(true, true, 95, Some(110), Some(120), true, 100, 30, 10)
            .unwrap();
    assert!(!plan.snapshot_should_persist);
    assert_eq!(plan.blocking_reset_at, Some(120));
    assert_eq!(plan.retry_backoff_until, Some(120));
    assert!(plan.retry_backoff_changed);

    assert_eq!(
        cached_summary_source_plan(false, true, true).unwrap(),
        CachedQuotaSummaryPlan {
            summary: CachedQuotaSummaryKind::Snapshot,
            source: CachedQuotaSourceKind::Snapshot,
        }
    );
    assert_eq!(
        cached_model_summary_source_plan(CACHED_MODEL_RETIRED, false, true, true, true, false,)
            .unwrap(),
        CachedQuotaSummaryPlan {
            summary: CachedQuotaSummaryKind::Retired,
            source: CachedQuotaSourceKind::Snapshot,
        }
    );
    assert!(cached_model_summary_force_unknown(CACHED_MODEL_LUNA, true).unwrap());
    assert_eq!(
        unknown_window_override(false, Some(123)).unwrap(),
        Some(123)
    );
    assert_eq!(unknown_window_override(true, Some(123)).unwrap(), None);
}
