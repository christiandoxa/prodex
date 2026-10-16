use super::*;

#[test]
fn runtime_doctor_reason_spans_are_mojo_owned_with_strict_abi() {
    for (value, expected) in [
        ("", vec![]),
        ("-", vec![]),
        (",,,", vec![]),
        ("one", vec!["one"]),
        (
            "  first, \u{2003}- , second , , 東京\u{a0}",
            vec!["first", "second", "東京"],
        ),
        ("-ish, --, -", vec!["-ish", "--"]),
    ] {
        assert_eq!(
            runtime_doctor_split_reason_field(value).unwrap(),
            expected,
            "{value}"
        );
    }
    let source = view(" one, two ");
    let mut count = -1_i64;
    let result = mojo_mut_pointer_address(&mut count);
    let input = mojo_pointer_address(&source);
    let call = |version, output, capacity| unsafe {
        prodex_mojo_runtime_doctor_reason_spans_v1(version, input, output, capacity, result)
    };
    assert_eq!(call(2, 0, 0), 4);
    assert_eq!(call(1, 0, 3), 1);
    assert_eq!(count, -1);
    assert_eq!(call(1, 0, 0), 0);
    assert_eq!(count, 2);
    let mut spans = [0_u64; 4];
    let output = mojo_mut_pointer_address(spans.as_mut_ptr());
    assert_eq!(call(1, output, 1), 3);
    assert_eq!(call(1, output, 2), 0);
    assert_eq!(spans, [1, 4, 6, 9]);
}

#[test]
fn runtime_doctor_smart_context_numeric_plans_use_compiled_mojo() {
    let plan = runtime_doctor_smart_context_bytes_plan(Some(40), Some(15), None).unwrap();
    assert_eq!(plan.saved_bytes, Some(25));
    assert_eq!(plan.estimated_tokens_saved, Some(7));
    assert_eq!(
        runtime_doctor_smart_context_bytes_plan(Some(2), Some(50), None)
            .unwrap()
            .saved_bytes,
        Some(0),
    );
    assert_eq!(
        runtime_doctor_smart_context_bytes_plan(None, Some(50), None)
            .unwrap()
            .saved_bytes,
        None,
    );
    assert_eq!(
        runtime_doctor_smart_context_bytes_plan(None, None, Some(usize::MAX))
            .unwrap()
            .estimated_tokens_saved,
        Some(4_611_686_018_427_387_904),
    );
    assert_eq!(
        runtime_doctor_smart_context_bytes_plan(Some(40), Some(15), Some(3))
            .unwrap()
            .saved_bytes,
        Some(3),
    );

    let initial = [0, 0, 0, usize::MAX - 1, 0, 0, 0, 0, 0, 0];
    let delta = [8, 3, 5, 2, 7, 11, 13];
    let rewritten =
        runtime_doctor_smart_context_totals_plan(initial, delta, Some("rewritten")).unwrap();
    assert_eq!(
        rewritten.counters,
        [1, 1, 0, usize::MAX, 3, 5, 2, 7, 11, 13]
    );
    assert_eq!(rewritten.decision_kind, 1);
    let pass =
        runtime_doctor_smart_context_totals_plan(rewritten.counters, delta, Some("pass_through"))
            .unwrap();
    assert_eq!(pass.counters[1], 1);
    assert_eq!(pass.counters[2], 0);
    assert_eq!(pass.decision_kind, 2);
    let fallback =
        runtime_doctor_smart_context_totals_plan(pass.counters, delta, Some("require_exact"))
            .unwrap();
    assert_eq!(fallback.counters[2], 1);
    assert_eq!(
        fallback.decision_kind,
        RUNTIME_DOCTOR_SMART_CONTEXT_DECISION_FALLBACK
    );
    let no_decision =
        runtime_doctor_smart_context_totals_plan(fallback.counters, delta, None).unwrap();
    assert_eq!(no_decision.decision_kind, 0);
    assert_eq!(no_decision.counters[1], 1);
    assert_eq!(no_decision.counters[2], 1);

    let mut bytes_output = [77_u64; 3];
    let bytes_out = mojo_mut_pointer_address(bytes_output.as_mut_ptr());
    assert_eq!(
        unsafe { prodex_mojo_runtime_doctor_smart_context_bytes_v1(2, 7, 20, 4, 5, bytes_out) },
        4
    );
    assert_eq!(
        unsafe { prodex_mojo_runtime_doctor_smart_context_bytes_v1(1, 8, 20, 4, 5, bytes_out) },
        1
    );
    assert_eq!(bytes_output, [77; 3]);
    let current = [0_u64; 10];
    let delta = [0_u64; 7];
    let decision = view("rewritten");
    let mut totals_output = [77_u64; 11];
    let mut invoke = |version, count, present| unsafe {
        prodex_mojo_runtime_doctor_smart_context_totals_v1(
            version,
            mojo_pointer_address(current.as_ptr()),
            mojo_pointer_address(delta.as_ptr()),
            mojo_pointer_address(&decision),
            present,
            mojo_mut_pointer_address(totals_output.as_mut_ptr()),
            count,
        )
    };
    assert_eq!(invoke(2, 11, 1), 4);
    assert_eq!(invoke(1, 10, 1), 1);
    assert_eq!(invoke(1, 11, 2), 1);
    assert_eq!(totals_output, [77; 11]);
}

#[test]
fn runtime_doctor_log_marker_token_matching_has_strict_compiled_abi() {
    for (message, marker, expected) in [
        (
            "[date] smart_context_autopilot request=3",
            "smart_context_autopilot",
            true,
        ),
        (
            "other\u{2003}('smart_context_autopilot'),",
            "smart_context_autopilot",
            true,
        ),
        (
            "smart_context_autopilot=3",
            "smart_context_autopilot",
            false,
        ),
        (
            "prefix_smart_context_autopilot",
            "smart_context_autopilot",
            false,
        ),
        ("...", "", true),
        ("", "", false),
    ] {
        assert_eq!(
            runtime_doctor_log_message_has_marker(message, marker).unwrap(),
            expected
        );
    }
    let message = view("smart_context_autopilot");
    let marker = view("smart_context_autopilot");
    let mut result = -1_i64;
    let invoke = |version, message_address, result_address| unsafe {
        prodex_mojo_runtime_doctor_log_message_has_marker_v1(
            version,
            message_address,
            mojo_pointer_address(&marker),
            result_address,
        )
    };
    let valid = mojo_pointer_address(&message);
    let output = mojo_mut_pointer_address(&mut result);
    assert_eq!(invoke(2, valid, output), 1);
    assert_eq!(invoke(1, 0, output), 1);
    assert_eq!(invoke(1, valid, 0), 1);
    assert_eq!(result, -1);
    assert_eq!(invoke(1, valid, output), 0);
    assert_eq!(result, 1);
}

#[test]
fn runtime_doctor_log_value_filter_is_mojo_owned() {
    assert!(runtime_doctor_log_value_is_ignored("").unwrap());
    assert!(runtime_doctor_log_value_is_ignored("-").unwrap());
    assert!(!runtime_doctor_log_value_is_ignored(" - ").unwrap());
    assert!(!runtime_doctor_log_value_is_ignored("value").unwrap());
}

#[test]
fn runtime_doctor_smart_context_fallback_reason_source_is_mojo_owned() {
    assert_eq!(
        runtime_doctor_smart_context_fallback_reason_source("self_check_passthrough", true, 1,)
            .unwrap(),
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
    assert_eq!(
        runtime_doctor_smart_context_fallback_reason_source("self_check_passthrough", false, 0,)
            .unwrap(),
        RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_DECISION,
    );
}

#[test]
fn runtime_doctor_smart_context_decision_fallback_classification_is_mojo_owned() {
    assert!(!runtime_doctor_smart_context_decision_is_fallback("rewritten").unwrap());
    assert!(!runtime_doctor_smart_context_decision_is_fallback("pass_through").unwrap());
    assert!(runtime_doctor_smart_context_decision_is_fallback("self_check_passthrough").unwrap());
    assert!(runtime_doctor_smart_context_decision_is_fallback("require_exact").unwrap());
    assert!(runtime_doctor_smart_context_decision_is_fallback("").unwrap());
}

#[test]
fn runtime_doctor_message_marker_selection_preserves_event_and_fallback_precedence() {
    let exact = "selection_pick profile=alpha";
    assert_eq!(
        runtime_doctor_parse_message_offsets(exact)
            .unwrap()
            .marker
            .map(|(start, end)| &exact[start..end]),
        Some("selection_pick")
    );

    let prefixed = "notice request_id=req-1 selection_pick profile=alpha";
    assert_eq!(
        runtime_doctor_parse_message_offsets(prefixed)
            .unwrap()
            .marker
            .map(|(start, end)| &prefixed[start..end]),
        Some("selection_pick")
    );

    let fields_first = "request=7 selection_pick profile=alpha";
    assert_eq!(
        runtime_doctor_parse_message_offsets(fields_first)
            .unwrap()
            .marker
            .map(|(start, end)| &fields_first[start..end]),
        Some("selection_pick")
    );

    assert_eq!(
        runtime_doctor_parse_message_offsets("notice unknown=value")
            .unwrap()
            .marker,
        None
    );
}

#[test]
fn runtime_doctor_marker_classifier_accepts_known_and_rejects_unknown() {
    assert!(runtime_doctor_marker_known("selection_pick").unwrap());
    assert!(runtime_doctor_marker_known("websocket_connect_overflow_rejected").unwrap());
    assert!(!runtime_doctor_marker_known("not_a_runtime_marker").unwrap());
}

#[test]
fn runtime_doctor_profile_provider_kind_is_mojo_owned() {
    assert_eq!(
        runtime_doctor_profile_provider_kind("openai").unwrap(),
        RUNTIME_DOCTOR_PROFILE_PROVIDER_OPENAI
    );
    assert_eq!(
        runtime_doctor_profile_provider_kind("gemini").unwrap(),
        RUNTIME_DOCTOR_PROFILE_PROVIDER_GEMINI
    );
    assert_eq!(
        runtime_doctor_profile_provider_kind("copilot").unwrap(),
        RUNTIME_DOCTOR_PROFILE_PROVIDER_COPILOT
    );
    assert_eq!(
        runtime_doctor_profile_provider_kind("anthropic").unwrap(),
        RUNTIME_DOCTOR_PROFILE_PROVIDER_EXTERNAL
    );
}

#[test]
fn runtime_doctor_marker_summary_counts_batch_and_chunk_marker_totals() {
    let long_marker = "x".repeat(257);
    let mut marker_counts = vec![
        ("selection_pick", 2),
        ("selection_keep_current", 3),
        ("selection_skip_current", 4),
        ("local_selection_blocked", 5),
        ("websocket_connect_overflow_rejected", 6),
        ("profile_auth_proactive_sync_failed", 7),
        ("compact_pressure_shed", 8),
        ("continuation_journal_queue_backpressure", 9),
        ("quota_critical_floor_before_send", 10),
        ("local_rewrite_gemini_live_sidecar_session_error", 11),
        (long_marker.as_str(), 12),
        ("unknown_marker", 0),
    ];
    marker_counts.extend((0..300).map(|_| ("selection_pick", 1)));

    let counts = runtime_doctor_marker_summary_counts(marker_counts).unwrap();

    assert_eq!(counts.selection_picked, 302);
    assert_eq!(counts.selection_kept, 3);
    assert_eq!(counts.selection_skipped, 4);
    assert_eq!(counts.selection_blocked, 15);
    assert_eq!(counts.failure_admission, 6);
    assert_eq!(counts.failure_auth, 7);
    assert_eq!(counts.failure_continuation, 8);
    assert_eq!(counts.failure_persistence, 9);
    assert_eq!(counts.failure_quota, 15);
    assert_eq!(counts.failure_transport, 11);
}

#[test]
fn runtime_doctor_marker_summary_counts_rejects_unrepresentable_and_overflowing_totals() {
    assert_eq!(
        runtime_doctor_marker_summary_counts([("selection_pick", usize::MAX)]),
        Err(MojoError::InvalidInput),
    );
    assert_eq!(
        runtime_doctor_marker_summary_counts([
            ("selection_pick", i64::MAX as usize),
            ("selection_pick", 1),
        ]),
        Err(MojoError::InvalidOutput),
    );
}
