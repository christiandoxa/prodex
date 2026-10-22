use super::*;

#[test]
fn compiled_operational_text_span_counts_scalars_and_preserves_full_width_limits() {
    for (value, limit, end, truncated) in [
        ("", 0, 0, false),
        ("text", 0, 0, true),
        ("é🦀e\u{301}", 3, 7, true),
        ("é🦀e\u{301}", 4, 9, false),
        ("é🦀e\u{301}", usize::MAX, 9, false),
        ("line\nnext", 5, 5, true),
        ("a\0b", 3, 3, false),
    ] {
        assert_eq!(
            operational_log_text_span(value, limit),
            Ok(OperationalTextSpan { end, truncated })
        );
    }
}

#[test]
fn compiled_operational_text_span_abi_rejects_invalid_utf8_and_shapes_before_writes() {
    let invalid = [0xff_u8];
    let mut output = [73_i64, 83];
    let mut call = |version, address, length, target| unsafe {
        prodex_mojo_operational_log_text_span_v1(
            version,
            address,
            length,
            0,
            if target {
                output.as_mut_ptr() as u64
            } else {
                0
            },
        )
    };
    assert_eq!(call(2, 0, 0, true), 4);
    assert_eq!(call(1, 0, 1, true), 1);
    assert_eq!(call(1, 0, -1, true), 1);
    assert_eq!(call(1, invalid.as_ptr() as u64, 1, true), 1);
    assert_eq!(call(1, 0, 0, false), 1);
    assert_eq!(output, [73, 83]);
}

#[test]
fn compiled_stream_policy_keeps_payload_exclusion_and_exact_switch_values() {
    const { assert!(crate::MOJO_ACTIVE) }
    for event in [
        "stream_payload",
        "upstream_payload",
        "token_usage",
        "token_usage_progress",
    ] {
        assert_eq!(operational_stream_plan(event, &[]), Ok(None));
    }
    for fields in [
        vec![],
        vec![("switched", "false")],
        vec![("switched", "TRUE")],
    ] {
        assert_eq!(operational_stream_plan("profile_commit", &fields), Ok(None));
    }
    assert_eq!(
        operational_stream_plan("profile_commit", &[("switched", "true")]),
        Ok(Some(OperationalStreamPlan {
            source: "route",
            load_fields: None,
        }))
    );
    assert_eq!(
        operational_stream_plan(
            "profile_commit",
            &[("switched", "false"), ("switched", "true")]
        ),
        Ok(None)
    );
}

#[test]
fn compiled_stream_policy_reuses_dynamic_source_interest_and_load_classification() {
    for (event, fields, source, load) in [
        ("request_captured", vec![], Some("request"), false),
        (
            "runtime_proxy_queue_overloaded",
            vec![],
            Some("error"),
            false,
        ),
        (
            "websocket_connect_overflow_rejected",
            vec![],
            Some("error"),
            false,
        ),
        ("profile_inflight_saturated", vec![], Some("load"), true),
        (
            "runtime_proxy_active_limit_reached",
            vec![],
            Some("load"),
            true,
        ),
        (
            "runtime_proxy_lane_limit_reached",
            vec![],
            Some("load"),
            true,
        ),
        (
            "compat_request_surface",
            vec![("tool_surface", "mcp,functions")],
            Some("mcp"),
            false,
        ),
        (
            "compat_request_surface",
            vec![("tool_surface", "sub_agent")],
            Some("agent"),
            false,
        ),
        (
            "compat_request_surface",
            vec![("tool_surface", "none"), ("family", "codex")],
            None,
            false,
        ),
        (
            "smart_context_prepare_fallback",
            vec![("decision", "pass_through")],
            None,
            false,
        ),
        ("new_fixture_event", vec![], Some("event"), false),
    ] {
        assert_eq!(
            operational_stream_plan(event, &fields),
            Ok(source.map(|source| OperationalStreamPlan {
                source,
                load_fields: load.then(Vec::new),
            })),
            "{event} {fields:?}"
        );
    }
}

#[test]
fn compiled_stream_policy_selects_exact_load_fields_and_endpoint_presentation_in_input_order() {
    use OperationalLoadField::{Endpoint, Plain};
    let fields = [
        ("reason", ""),
        ("path", "https://example.com/responses?fake=1"),
        ("request", "17"),
        ("profile", "synthetic"),
        ("route", "responses"),
        ("lane", "responses"),
        ("transport", "http"),
        ("context", "fresh"),
        ("provider", "openai"),
        ("model", "fixture"),
        ("limit", "8"),
        ("hard_limit", "16"),
        ("authorization", "fake"),
        ("Path", "/ignored"),
    ];
    assert_eq!(
        operational_stream_plan("runtime_proxy_active_limit_reached", &fields),
        Ok(Some(OperationalStreamPlan {
            source: "load",
            load_fields: Some(vec![
                Plain(0),
                Endpoint(1),
                Plain(3),
                Plain(4),
                Plain(5),
                Plain(6),
                Plain(7),
                Plain(8),
                Plain(9),
                Plain(10),
                Plain(11)
            ]),
        }))
    );
    assert_eq!(
        operational_stream_plan("runtime_proxy_queue_overloaded", &fields),
        Ok(Some(OperationalStreamPlan {
            source: "error",
            load_fields: None
        }))
    );
}

#[test]
fn compiled_stream_abi_rejects_invalid_views_versions_and_capacity_without_writes() {
    let event = "profile_commit";
    let invalid = [0xff_u8];
    let mut views = [TextView::from("switched"), TextView::from("true")];
    let mut output = [73_i64; 2];
    let mut metadata = [83_i64; 2];
    let mut call = |version, address, count, capacity| unsafe {
        prodex_mojo_operational_log_stream_v1(
            version,
            event.as_ptr() as u64,
            event.len() as i64,
            address,
            count,
            output.as_mut_ptr() as u64,
            capacity,
            metadata.as_mut_ptr() as u64,
        )
    };
    assert_eq!(call(2, views.as_ptr() as u64, 1, 2), 4);
    assert_eq!(call(1, 0, 1, 2), 1);
    assert_eq!(call(1, views.as_ptr() as u64, 1_048_577, 2), 1);
    assert_eq!(call(1, views.as_ptr() as u64, 1, 0), 2);
    views[1] = TextView {
        address: invalid.as_ptr() as u64,
        length: 1,
    };
    assert_eq!(call(1, views.as_ptr() as u64, 1, 2), 1);
    views[1] = TextView {
        address: 0,
        length: 1,
    };
    assert_eq!(call(1, views.as_ptr() as u64, 1, 2), 1);
    assert_eq!(output, [73; 2]);
    assert_eq!(metadata, [83; 2]);
}
