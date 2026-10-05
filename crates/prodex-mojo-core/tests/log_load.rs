#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::log_load::{LogLoadAggregateInput, aggregate_summary, aggregate_update};

#[test]
fn required_mojo_classifies_routine_load_events_exactly() {
    for event_name in [
        "profile_inflight_saturated",
        "runtime_proxy_active_limit_reached",
        "runtime_proxy_lane_limit_reached",
    ] {
        let plan = aggregate_update(LogLoadAggregateInput {
            event_name,
            previous_key: None,
            observation_key: "",
            elapsed_ns: 0,
            occurrences: 0,
            unique_run_ids: &[],
            run_count_overflow: false,
            run_id: None,
        })
        .unwrap();
        assert!(plan.routine, "{event_name}");
    }
    let plan = aggregate_update(LogLoadAggregateInput {
        event_name: "profile_inflight",
        previous_key: None,
        observation_key: "",
        elapsed_ns: 0,
        occurrences: 0,
        unique_run_ids: &[],
        run_count_overflow: false,
        run_id: None,
    })
    .unwrap();
    assert!(!plan.routine);
}

#[test]
fn required_mojo_plans_freshness_deduplication_and_saturation() {
    let previous_runs = vec!["r0001".to_string()];
    let boundary = aggregate_update(LogLoadAggregateInput {
        event_name: "profile_inflight",
        previous_key: Some("profile_inflight\u{1f}main"),
        observation_key: "profile_inflight\u{1f}main",
        elapsed_ns: 5_000_000_000,
        occurrences: u64::MAX,
        unique_run_ids: &previous_runs,
        run_count_overflow: false,
        run_id: Some("r0001"),
    })
    .unwrap();
    assert!(boundary.coalesce);
    assert_eq!(boundary.occurrences, u64::MAX);
    assert!(!boundary.append_run);
    assert!(!boundary.run_count_overflow);

    let expired = aggregate_update(LogLoadAggregateInput {
        event_name: "profile_inflight",
        previous_key: Some("profile_inflight\u{1f}main"),
        observation_key: "profile_inflight\u{1f}main",
        elapsed_ns: 5_000_000_001,
        occurrences: 12,
        unique_run_ids: &previous_runs,
        run_count_overflow: false,
        run_id: Some("r0002"),
    })
    .unwrap();
    assert!(!expired.coalesce);
    assert_eq!(expired.occurrences, 1);
    assert!(expired.append_run);
    assert!(!expired.run_count_overflow);

    let full_runs = (0..256)
        .map(|index| format!("r{index:04x}"))
        .collect::<Vec<_>>();
    let full = aggregate_update(LogLoadAggregateInput {
        event_name: "profile_inflight",
        previous_key: Some("same"),
        observation_key: "same",
        elapsed_ns: 0,
        occurrences: 4,
        unique_run_ids: &full_runs,
        run_count_overflow: false,
        run_id: Some("r0100"),
    })
    .unwrap();
    assert!(full.coalesce);
    assert_eq!(full.occurrences, 5);
    assert!(!full.append_run);
    assert!(full.run_count_overflow);
}

#[test]
fn required_mojo_formats_bounded_aggregate_summaries_exactly() {
    assert_eq!(aggregate_summary(2, 2, false).unwrap(), " · ×2 · 2 runs");
    assert_eq!(
        aggregate_summary(256, 256, true).unwrap(),
        " · ×256 · 256+ runs"
    );
    assert!(aggregate_summary(1, 257, false).is_err());
}
