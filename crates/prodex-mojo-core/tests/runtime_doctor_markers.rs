#![cfg(feature = "mojo-rich")]
#![allow(unsafe_code)]
use prodex_mojo_core::rich::{
    RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_DECISION,
    RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_EVENT_REASONS,
    RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_SELF_CHECK, runtime_doctor_log_value_is_ignored,
    runtime_doctor_marker_known, runtime_doctor_marker_semantics,
    runtime_doctor_smart_context_decision_is_fallback,
    runtime_doctor_smart_context_fallback_reason_source,
};

#[test]
fn smart_context_fallback_decision_uses_mojo_authority() {
    assert!(!runtime_doctor_smart_context_decision_is_fallback("rewritten").unwrap());
    assert!(!runtime_doctor_smart_context_decision_is_fallback("pass_through").unwrap());
    assert!(runtime_doctor_smart_context_decision_is_fallback("self_check_passthrough").unwrap());
    assert!(runtime_doctor_smart_context_decision_is_fallback("require_exact").unwrap());
}

#[test]
fn log_value_absence_policy_uses_mojo_authority() {
    assert!(runtime_doctor_log_value_is_ignored("").unwrap());
    assert!(runtime_doctor_log_value_is_ignored("-").unwrap());
    assert!(!runtime_doctor_log_value_is_ignored(" - ").unwrap());
    assert!(!runtime_doctor_log_value_is_ignored("value").unwrap());
}

#[test]
fn smart_context_fallback_reason_source_uses_mojo_authority() {
    assert_eq!(
        runtime_doctor_smart_context_fallback_reason_source("require_exact", true, 1).unwrap(),
        RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_EVENT_REASONS,
    );
    assert_eq!(
        runtime_doctor_smart_context_fallback_reason_source("self_check_passthrough", true, 0,)
            .unwrap(),
        RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_SELF_CHECK,
    );
    assert_eq!(
        runtime_doctor_smart_context_fallback_reason_source("require_exact", true, 0).unwrap(),
        RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_DECISION,
    );
}

#[test]
fn long_unknown_marker_is_not_an_invalid_event() {
    for length in [0, 1, 255, 256, 257, 4096, 1_048_640] {
        for symbol in ["x", "🦀"] {
            let text = symbol.repeat(length);
            assert!(!runtime_doctor_marker_known(&text).unwrap());
            let semantics = runtime_doctor_marker_semantics(&text).unwrap();
            assert_eq!(
                (
                    semantics.timeline_phase,
                    semantics.selection_bucket,
                    semantics.route_action,
                    semantics.failure_class
                ),
                (0, 0, 0, 0)
            );
        }
    }
    assert!(runtime_doctor_marker_known("runtime_proxy_queue_overloaded").unwrap());
    assert!(runtime_doctor_marker_known("first_local_chunk").unwrap());
}

#[derive(Clone, Copy)]
#[repr(C)]
struct View {
    ptr: u64,
    len: u64,
}
unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_marker_known_v1(abi: i64, marker: u64, output: u64) -> i64;
    fn prodex_mojo_runtime_doctor_marker_semantics_v2(abi: i64, marker: u64, output: u64) -> i64;
    fn prodex_mojo_runtime_doctor_marker_summary_counts_v1(
        abi: i64,
        markers: u64,
        counts: u64,
        count: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_compact_exit_counts_v1(
        abi: i64,
        markers: u64,
        counts: u64,
        count: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_marker_aggregation_v1(
        abi: i64,
        markers: u64,
        values: u64,
        presence: u64,
        weights: u64,
        input_count: i64,
        representatives: u64,
        kinds: u64,
        value_slots: u64,
        counts: u64,
        output_capacity: i64,
        output_count: u64,
        latest_stale_reason_index: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_route_health_groups_v1(
        abi: i64,
        profiles: u64,
        routes: u64,
        event_flags: u64,
        count: i64,
        representative_indices: u64,
        event_counts: u64,
        latest_event_indices: u64,
        profile_row_indices: u64,
        output_count: u64,
    ) -> i64;
}

#[test]
fn marker_semantics_abi_tags_all_failure_classes() {
    for (marker, expected_class) in [
        ("websocket_connect_overflow_rejected", 1_i64),
        ("profile_auth_proactive_sync_failed", 2),
        ("compact_pressure_shed", 3),
        ("continuation_journal_queue_backpressure", 4),
        ("quota_critical_floor_before_send", 5),
        ("local_rewrite_gemini_live_sidecar_session_error", 6),
    ] {
        let view = View {
            ptr: marker.as_ptr() as u64,
            len: marker.len() as u64,
        };
        let mut output = [i64::MIN; 4];
        let code = unsafe {
            prodex_mojo_runtime_doctor_marker_semantics_v2(
                2,
                &view as *const View as u64,
                output.as_mut_ptr() as u64,
            )
        };
        assert_eq!(code, 0, "marker={marker}");
        assert_eq!(output[3], expected_class, "marker={marker}");
    }
}

#[test]
fn long_marker_support_does_not_accept_invalid_utf8_or_abi() {
    for length in [1, 256, 257, 4096] {
        let mut bytes = vec![b'x'; length];
        bytes[length - 1] = 0xff;
        let view = View {
            ptr: bytes.as_ptr() as u64,
            len: bytes.len() as u64,
        };
        let functions: [(unsafe extern "C" fn(i64, u64, u64) -> i64, i64); 2] = [
            (prodex_mojo_runtime_doctor_marker_known_v1, 1),
            (prodex_mojo_runtime_doctor_marker_semantics_v2, 2),
        ];
        for (function, abi_version) in functions {
            let mut output = [77_i64; 4];
            let code = unsafe {
                function(
                    abi_version,
                    &view as *const View as u64,
                    output.as_mut_ptr() as u64,
                )
            };
            assert_eq!(code, 2);
            assert_eq!(output, [77; 4]);
            let code = unsafe {
                function(
                    abi_version + 8,
                    &view as *const View as u64,
                    output.as_mut_ptr() as u64,
                )
            };
            assert_eq!(code, 1);
            assert_eq!(output, [77; 4]);
        }
    }
}

#[test]
fn marker_summary_counts_abi_tags_fixed_selection_and_failure_totals() {
    let inputs = [
        ("selection_pick", 2_i64),
        ("selection_keep_current", 3),
        ("selection_skip_current", 4),
        ("local_selection_blocked", 5),
        ("websocket_connect_overflow_rejected", 6),
        ("profile_auth_proactive_sync_failed", 7),
        ("compact_pressure_shed", 8),
        ("continuation_journal_queue_backpressure", 9),
        ("quota_critical_floor_before_send", 10),
        ("local_rewrite_gemini_live_sidecar_session_error", 11),
    ];
    let views = inputs
        .iter()
        .map(|(marker, _)| View {
            ptr: marker.as_ptr() as u64,
            len: marker.len() as u64,
        })
        .collect::<Vec<_>>();
    let counts = inputs.map(|(_, count)| count);
    let mut output = [0_i64; 10];
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_summary_counts_v1(
            1,
            views.as_ptr() as u64,
            counts.as_ptr() as u64,
            views.len() as i64,
            output.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(output, [2, 3, 4, 15, 6, 7, 8, 9, 15, 11]);

    let mut unchanged = [77_i64; 10];
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_summary_counts_v1(
            9,
            views.as_ptr() as u64,
            counts.as_ptr() as u64,
            views.len() as i64,
            unchanged.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 4);
    assert_eq!(unchanged, [77; 10]);
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_summary_counts_v1(
            1,
            views.as_ptr() as u64,
            counts.as_ptr() as u64,
            257,
            unchanged.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 3);
    assert_eq!(unchanged, [77; 10]);

    let mut negative_accumulator = [-1_i64; 10];
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_summary_counts_v1(
            1,
            views.as_ptr() as u64,
            counts.as_ptr() as u64,
            views.len() as i64,
            negative_accumulator.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 2);
    assert_eq!(negative_accumulator, [-1; 10]);

    let overflow_inputs = [("selection_pick", i64::MAX), ("selection_pick", 1)];
    let overflow_views = overflow_inputs
        .iter()
        .map(|(marker, _)| View {
            ptr: marker.as_ptr() as u64,
            len: marker.len() as u64,
        })
        .collect::<Vec<_>>();
    let overflow_counts = overflow_inputs.map(|(_, count)| count);
    let mut overflow_accumulator = [0_i64; 10];
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_summary_counts_v1(
            1,
            overflow_views.as_ptr() as u64,
            overflow_counts.as_ptr() as u64,
            overflow_views.len() as i64,
            overflow_accumulator.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 5);
    assert!(overflow_accumulator.iter().all(|count| *count >= 0));
}

#[test]
fn marker_aggregation_abi_groups_context_reasons_continuations_facets_and_sentinels() {
    const WIDTH: usize = 38;
    const FACET_BASE: i64 = 10;
    const CONTEXT_ROUTE: i64 = 1;
    const CONTEXT_LANE: i64 = 2;
    const CONTEXT_PROFILE: i64 = 3;
    const CHAIN_REASON: i64 = 4;
    const STALE_REASON: i64 = 6;
    const PREVIOUS_ROUTE: i64 = 7;
    const PREVIOUS_TRANSPORT: i64 = 8;
    const PREVIOUS_REQUEST_SHAPE: i64 = 9;
    let markers = [
        "selection_pick",
        "selection_pick",
        "chain_retried_owner",
        "stale_continuation",
        "stale_continuation",
        "previous_response_not_found",
        "previous_response_fresh_fallback_blocked",
        "chain_dead_upstream_confirmed",
    ];
    let mut values = vec![View { ptr: 0, len: 0 }; markers.len() * WIDTH];
    let mut presence = vec![0_i64; markers.len()];
    let weights = vec![1_i64; markers.len()];
    let mut set = |event: usize, slot: usize, value: &str| {
        values[event * WIDTH + slot] = View {
            ptr: value.as_ptr() as u64,
            len: value.len() as u64,
        };
        presence[event] |= 1_i64 << slot;
    };
    set(0, 0, "/responses");
    set(0, 1, "responses");
    set(0, 2, "alpha");
    set(0, 6 + 1, "responses");
    set(0, 6 + 2, "/responses");
    set(1, 0, "-");
    set(1, 1, "responses");
    set(1, 2, "alpha");
    set(1, 6 + 1, "responses");
    set(2, 3, "owner_retry");
    set(2, 6 + 3, "owner_retry");
    set(3, 3, "old_reason");
    set(4, 3, "new_reason");
    set(5, 0, "/responses");
    set(5, 4, "http");
    set(6, 5, "session_replayable");
    let facet_values = (0..32)
        .map(|index| format!("facet_{index}"))
        .collect::<Vec<_>>();
    for (index, value) in facet_values.iter().enumerate() {
        set(6, 6 + index, value);
    }
    set(7, 3, "dead_reason");
    let marker_views = markers
        .iter()
        .map(|marker| View {
            ptr: marker.as_ptr() as u64,
            len: marker.len() as u64,
        })
        .collect::<Vec<_>>();
    let mut representatives = vec![-1_i64; 256];
    let mut kinds = vec![-1_i64; 256];
    let mut value_slots = vec![-1_i64; 256];
    let mut counts = vec![-1_i64; 256];
    let mut output_count = -1_i64;
    let mut latest_stale_reason_index = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_aggregation_v1(
            1,
            marker_views.as_ptr() as u64,
            values.as_ptr() as u64,
            presence.as_ptr() as u64,
            weights.as_ptr() as u64,
            markers.len() as i64,
            representatives.as_mut_ptr() as u64,
            kinds.as_mut_ptr() as u64,
            value_slots.as_mut_ptr() as u64,
            counts.as_mut_ptr() as u64,
            representatives.len() as i64,
            &mut output_count as *mut i64 as u64,
            &mut latest_stale_reason_index as *mut i64 as u64,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(latest_stale_reason_index, 4);

    let rows = (0..output_count as usize)
        .map(|index| (kinds[index], value_slots[index], counts[index]))
        .collect::<Vec<_>>();
    assert!(rows.contains(&(0, -1, 2)), "marker totals: {rows:?}");
    assert!(
        rows.contains(&(CONTEXT_ROUTE, 0, 1)),
        "route context: {rows:?}"
    );
    assert!(
        rows.contains(&(CONTEXT_LANE, 1, 2)),
        "lane context: {rows:?}"
    );
    assert!(
        rows.contains(&(CONTEXT_PROFILE, 2, 2)),
        "profile context: {rows:?}"
    );
    assert!(
        rows.contains(&(CHAIN_REASON, 3, 1)),
        "chain reason: {rows:?}"
    );
    assert!(rows.contains(&(5, 3, 1)), "dead-upstream reason: {rows:?}");
    assert_eq!(
        rows.iter()
            .filter(|row| **row == (STALE_REASON, 3, 1))
            .count(),
        2,
        "stale reasons: {rows:?}"
    );
    assert!(
        rows.contains(&(PREVIOUS_ROUTE, 0, 1)),
        "previous route: {rows:?}"
    );
    assert!(
        rows.contains(&(PREVIOUS_TRANSPORT, 4, 1)),
        "previous transport: {rows:?}"
    );
    assert!(
        rows.contains(&(PREVIOUS_REQUEST_SHAPE, 5, 1)),
        "request shape: {rows:?}"
    );
    assert!(
        rows.contains(&(FACET_BASE + 1, 7, 2)),
        "lane facet: {rows:?}"
    );
    assert!(
        rows.contains(&(FACET_BASE + 2, 8, 1)),
        "route facet: {rows:?}"
    );
    assert!(
        rows.contains(&(FACET_BASE + 3, 9, 1)),
        "reason facet: {rows:?}"
    );
    for (index, _) in facet_values.iter().enumerate() {
        assert!(
            rows.contains(&(FACET_BASE + index as i64, 6 + index as i64, 1)),
            "facet {index}: {rows:?}"
        );
    }

    let mut unchanged = [77_i64; 256];
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_aggregation_v1(
            9,
            marker_views.as_ptr() as u64,
            values.as_ptr() as u64,
            presence.as_ptr() as u64,
            weights.as_ptr() as u64,
            markers.len() as i64,
            unchanged.as_mut_ptr() as u64,
            unchanged.as_mut_ptr() as u64,
            unchanged.as_mut_ptr() as u64,
            unchanged.as_mut_ptr() as u64,
            unchanged.len() as i64,
            &mut output_count as *mut i64 as u64,
            &mut latest_stale_reason_index as *mut i64 as u64,
        )
    };
    assert_eq!(status, 4);

    let mut overflow_weights = weights.clone();
    overflow_weights[0] = i64::MAX;
    overflow_weights[1] = 1;
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_aggregation_v1(
            1,
            marker_views.as_ptr() as u64,
            values.as_ptr() as u64,
            presence.as_ptr() as u64,
            overflow_weights.as_ptr() as u64,
            markers.len() as i64,
            representatives.as_mut_ptr() as u64,
            kinds.as_mut_ptr() as u64,
            value_slots.as_mut_ptr() as u64,
            counts.as_mut_ptr() as u64,
            representatives.len() as i64,
            &mut output_count as *mut i64 as u64,
            &mut latest_stale_reason_index as *mut i64 as u64,
        )
    };
    assert_eq!(status, 5);

    let mut invalid_presence = presence.clone();
    invalid_presence[0] = 1_i64 << WIDTH;
    let mut invalid_output_count = 77_i64;
    let mut invalid_latest_stale_reason_index = 77_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_aggregation_v1(
            1,
            marker_views.as_ptr() as u64,
            values.as_ptr() as u64,
            invalid_presence.as_ptr() as u64,
            weights.as_ptr() as u64,
            markers.len() as i64,
            representatives.as_mut_ptr() as u64,
            kinds.as_mut_ptr() as u64,
            value_slots.as_mut_ptr() as u64,
            counts.as_mut_ptr() as u64,
            representatives.len() as i64,
            &mut invalid_output_count as *mut i64 as u64,
            &mut invalid_latest_stale_reason_index as *mut i64 as u64,
        )
    };
    assert_eq!(status, 2);
    assert_eq!(invalid_output_count, 0);
    assert_eq!(invalid_latest_stale_reason_index, -1);

    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_aggregation_v1(
            1,
            marker_views.as_ptr() as u64,
            values.as_ptr() as u64,
            presence.as_ptr() as u64,
            weights.as_ptr() as u64,
            markers.len() as i64,
            representatives.as_mut_ptr() as u64,
            kinds.as_mut_ptr() as u64,
            value_slots.as_mut_ptr() as u64,
            counts.as_mut_ptr() as u64,
            0,
            &mut invalid_output_count as *mut i64 as u64,
            &mut invalid_latest_stale_reason_index as *mut i64 as u64,
        )
    };
    assert_eq!(status, 3);

    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_aggregation_v1(
            1,
            marker_views.as_ptr() as u64,
            values.as_ptr() as u64,
            presence.as_ptr() as u64,
            weights.as_ptr() as u64,
            257,
            representatives.as_mut_ptr() as u64,
            kinds.as_mut_ptr() as u64,
            value_slots.as_mut_ptr() as u64,
            counts.as_mut_ptr() as u64,
            representatives.len() as i64,
            &mut invalid_output_count as *mut i64 as u64,
            &mut invalid_latest_stale_reason_index as *mut i64 as u64,
        )
    };
    assert_eq!(status, 1);
}

#[test]
fn route_health_groups_abi_merges_latest_rows_and_orders_by_event_pressure() {
    let inputs = [
        ("beta", "responses", 1_i64),
        ("alpha", "responses", 1),
        ("alpha", "compact", 0),
        ("beta", "responses", 1),
        ("alpha", "responses", 0),
        ("gamma", "standard", 1),
        ("alpha", "responses", 1),
    ];
    let profiles = inputs
        .iter()
        .map(|(profile, _, _)| View {
            ptr: profile.as_ptr() as u64,
            len: profile.len() as u64,
        })
        .collect::<Vec<_>>();
    let routes = inputs
        .iter()
        .map(|(_, route, _)| View {
            ptr: route.as_ptr() as u64,
            len: route.len() as u64,
        })
        .collect::<Vec<_>>();
    let flags = inputs.map(|(_, _, is_event)| is_event);
    let mut representatives = [i64::MIN; 7];
    let mut event_counts = [i64::MIN; 7];
    let mut latest_events = [i64::MIN; 7];
    let mut profile_rows = [i64::MIN; 7];
    let mut output_count = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_route_health_groups_v1(
            1,
            profiles.as_ptr() as u64,
            routes.as_ptr() as u64,
            flags.as_ptr() as u64,
            inputs.len() as i64,
            representatives.as_mut_ptr() as u64,
            event_counts.as_mut_ptr() as u64,
            latest_events.as_mut_ptr() as u64,
            profile_rows.as_mut_ptr() as u64,
            &mut output_count as *mut i64 as u64,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(output_count, 4);
    assert_eq!(&representatives[..4], &[1, 0, 5, 2]);
    assert_eq!(&event_counts[..4], &[2, 2, 1, 0]);
    assert_eq!(&latest_events[..4], &[6, 3, 5, -1]);
    assert_eq!(&profile_rows[..4], &[4, -1, -1, 2]);

    let mut invalid_flags = flags;
    invalid_flags[0] = 2;
    output_count = 91;
    assert_eq!(
        unsafe {
            prodex_mojo_runtime_doctor_route_health_groups_v1(
                1,
                profiles.as_ptr() as u64,
                routes.as_ptr() as u64,
                invalid_flags.as_ptr() as u64,
                inputs.len() as i64,
                representatives.as_mut_ptr() as u64,
                event_counts.as_mut_ptr() as u64,
                latest_events.as_mut_ptr() as u64,
                profile_rows.as_mut_ptr() as u64,
                &mut output_count as *mut i64 as u64,
            )
        },
        1
    );
    assert_eq!(output_count, 0);
}

#[test]
fn route_health_groups_sorting_moves_late_high_pressure_group_across_two_predecessors() {
    let inputs = [
        ("alpha", "responses", 1_i64),
        ("bravo", "responses", 1),
        ("bravo", "responses", 1),
        ("charlie", "responses", 1),
        ("charlie", "responses", 1),
        ("charlie", "responses", 1),
    ];
    let profiles = inputs
        .iter()
        .map(|(profile, _, _)| View {
            ptr: profile.as_ptr() as u64,
            len: profile.len() as u64,
        })
        .collect::<Vec<_>>();
    let routes = inputs
        .iter()
        .map(|(_, route, _)| View {
            ptr: route.as_ptr() as u64,
            len: route.len() as u64,
        })
        .collect::<Vec<_>>();
    let flags = inputs.map(|(_, _, is_event)| is_event);
    let mut representatives = [-1_i64; 6];
    let mut counts = [-1_i64; 6];
    let mut latest = [-1_i64; 6];
    let mut profile_rows = [-1_i64; 6];
    let mut output_count = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_route_health_groups_v1(
            1,
            profiles.as_ptr() as u64,
            routes.as_ptr() as u64,
            flags.as_ptr() as u64,
            inputs.len() as i64,
            representatives.as_mut_ptr() as u64,
            counts.as_mut_ptr() as u64,
            latest.as_mut_ptr() as u64,
            profile_rows.as_mut_ptr() as u64,
            &mut output_count as *mut i64 as u64,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(output_count, 3);
    assert_eq!(&representatives[..3], &[3, 1, 0]);
    assert_eq!(&counts[..3], &[3, 2, 1]);
    assert_eq!(&latest[..3], &[5, 2, 0]);
}

#[test]
fn compact_exit_counts_abi_sums_aliases_and_rejects_invalid_inputs() {
    let inputs = [
        ("compact_candidate_exhausted", 2_i64),
        ("compact_exit_candidate_exhausted", 3),
        ("compact_committed", 1),
        ("compact_exit_committed", 2),
        ("compact_committed_owner", 1),
        ("compact_exit_committed_owner", 2),
        ("compact_followup_owner", 1),
        ("compact_exit_followup_owner", 2),
        ("compact_lineage_released", 1),
        ("compact_exit_lineage_released", 2),
        ("compact_overload_conservative_retry", 1),
        ("compact_exit_overload_conservative_retry", 2),
        ("compact_precommit_budget_exhausted", 1),
        ("compact_exit_precommit_budget_exhausted", 2),
        ("compact_pressure_shed", 1),
        ("compact_exit_pressure_shed", 2),
        ("compact_quota_unclassified", 1),
        ("compact_exit_quota_unclassified", 2),
        ("compact_retryable_failure", 1),
        ("compact_exit_retryable_failure", 2),
        ("compact_transport_failure", 5),
        ("unrelated_marker", 9),
    ];
    let views = inputs
        .iter()
        .map(|(marker, _)| View {
            ptr: marker.as_ptr() as u64,
            len: marker.len() as u64,
        })
        .collect::<Vec<_>>();
    let counts = inputs.map(|(_, count)| count);
    let mut output = [0_i64; 11];
    let status = unsafe {
        prodex_mojo_runtime_doctor_compact_exit_counts_v1(
            1,
            views.as_ptr() as u64,
            counts.as_ptr() as u64,
            views.len() as i64,
            output.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(output, [5, 3, 3, 3, 3, 3, 3, 3, 3, 3, 5]);

    let mut unchanged = [77_i64; 11];
    let status = unsafe {
        prodex_mojo_runtime_doctor_compact_exit_counts_v1(
            9,
            views.as_ptr() as u64,
            counts.as_ptr() as u64,
            views.len() as i64,
            unchanged.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 4);
    assert_eq!(unchanged, [77; 11]);
    let status = unsafe {
        prodex_mojo_runtime_doctor_compact_exit_counts_v1(
            1,
            views.as_ptr() as u64,
            counts.as_ptr() as u64,
            257,
            unchanged.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 3);
    assert_eq!(unchanged, [77; 11]);

    let invalid_utf8 = [0xff_u8];
    let invalid_view = View {
        ptr: invalid_utf8.as_ptr() as u64,
        len: invalid_utf8.len() as u64,
    };
    let invalid_count = [1_i64];
    let status = unsafe {
        prodex_mojo_runtime_doctor_compact_exit_counts_v1(
            1,
            &invalid_view as *const View as u64,
            invalid_count.as_ptr() as u64,
            1,
            unchanged.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 2);
    assert_eq!(unchanged, [77; 11]);

    let overflow_inputs = [
        ("compact_candidate_exhausted", i64::MAX),
        ("compact_exit_candidate_exhausted", 1),
    ];
    let overflow_views = overflow_inputs
        .iter()
        .map(|(marker, _)| View {
            ptr: marker.as_ptr() as u64,
            len: marker.len() as u64,
        })
        .collect::<Vec<_>>();
    let overflow_counts = overflow_inputs.map(|(_, count)| count);
    let mut overflow_output = [0_i64; 11];
    let status = unsafe {
        prodex_mojo_runtime_doctor_compact_exit_counts_v1(
            1,
            overflow_views.as_ptr() as u64,
            overflow_counts.as_ptr() as u64,
            overflow_views.len() as i64,
            overflow_output.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 5);
    assert!(overflow_output.iter().all(|count| *count >= 0));
}

#[test]
fn compact_exit_counts_adapter_batches_large_marker_sets() {
    let counts = prodex_mojo_core::rich::runtime_doctor_compact_exit_counts(std::iter::repeat_n(
        ("compact_committed", 1),
        257,
    ))
    .unwrap();

    assert_eq!(counts, vec![("committed".to_string(), 257)]);
}
