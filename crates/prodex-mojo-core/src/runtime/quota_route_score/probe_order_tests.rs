use super::*;

fn summary(weekly: i64, five_hour: i64) -> QuotaSummarySortKeyInput {
    QuotaSummarySortKeyInput {
        route_band: 0,
        weekly_status: 0,
        five_hour_status: 0,
        weekly_remaining: weekly,
        five_hour_remaining: five_hour,
        weekly_reset_at: i64::MAX,
        five_hour_reset_at: i64::MIN,
    }
}

#[test]
fn compiled_probe_order_preserves_quota_priority_missing_facts_unsigned_order_and_stable_ties() {
    let mut thin = summary(100, 100);
    thin.weekly_status = 1;
    let inputs = [
        QuotaProbeOrderInput {
            original_order: 0,
            summary: None,
        },
        QuotaProbeOrderInput {
            original_order: usize::MAX,
            summary: Some(summary(20, 80)),
        },
        QuotaProbeOrderInput {
            original_order: 2,
            summary: Some(summary(90, 90)),
        },
        QuotaProbeOrderInput {
            original_order: 0,
            summary: Some(summary(20, 80)),
        },
        QuotaProbeOrderInput {
            original_order: 0,
            summary: Some(summary(20, 80)),
        },
        QuotaProbeOrderInput {
            original_order: 0,
            summary: Some(thin),
        },
    ];
    assert_eq!(
        quota_probe_order(inputs.into_iter()),
        Ok(QuotaProbeOrderPlan::Reorder(vec![2, 3, 4, 1, 5, 0]))
    );
    let mut earlier = summary(50, 50);
    earlier.weekly_reset_at = i64::MIN;
    assert_eq!(
        quota_probe_order(
            [
                QuotaProbeOrderInput {
                    original_order: 0,
                    summary: Some(summary(50, 50))
                },
                QuotaProbeOrderInput {
                    original_order: usize::MAX,
                    summary: Some(earlier)
                },
            ]
            .into_iter()
        ),
        Ok(QuotaProbeOrderPlan::Reorder(vec![1, 0]))
    );
}

#[test]
fn compiled_probe_order_keeps_trivial_inputs_without_acquiring_facts() {
    for count in 0..=1 {
        let inputs = (0..count)
            .map(|_| -> QuotaProbeOrderInput { panic!("trivial order acquired quota facts") });
        assert_eq!(
            quota_probe_order(inputs),
            Ok(QuotaProbeOrderPlan::KeepInput)
        );
    }
}

#[test]
fn compiled_probe_order_supports_large_pools_without_a_new_count_cap() {
    let count = 65_537;
    let inputs = (0..count).map(|index| QuotaProbeOrderInput {
        original_order: count - index,
        summary: None,
    });
    assert_eq!(
        quota_probe_order(inputs),
        Ok(QuotaProbeOrderPlan::Reorder((0..count).rev().collect()))
    );
}

#[test]
fn probe_order_abi_rejects_late_bad_rows_before_scratch_or_output_writes() {
    let rows = [
        Row {
            original_order: 0,
            present: 0,
            fields: [0; 7],
        },
        Row {
            original_order: 1,
            present: 1,
            fields: [5, 0, 0, 50, 50, 0, 0],
        },
    ];
    let mut scratch = [71_i64; 20];
    let mut output = [71_i64; 2];
    let raw = rows.as_ptr() as u64;
    let temporary = scratch.as_mut_ptr() as u64;
    let destination = output.as_mut_ptr() as u64;
    let call = |version, rows, count, scratch, words, output, capacity| unsafe {
        prodex_runtime_quota_probe_order_v1(version, rows, count, scratch, words, output, capacity)
    };
    assert_eq!(call(2, raw, 2, temporary, 20, destination, 2), -4);
    for count in [-1, i64::MAX] {
        assert_eq!(call(1, raw, count, temporary, 20, destination, 2), -1);
    }
    assert_eq!(call(1, 0, 2, 0, 0, 0, 0), 5);
    assert_eq!(call(1, raw, 2, 1, 20, destination, 2), -1);
    assert_eq!(call(1, raw, 2, temporary, 19, destination, 2), -3);
    assert_eq!(call(1, raw, 2, temporary, 20, destination, 1), -3);
    assert_eq!(call(1, raw, 2, temporary, 20, destination, 2), -1);
    assert_eq!(scratch, [71; 20]);
    assert_eq!(output, [71; 2]);
}
