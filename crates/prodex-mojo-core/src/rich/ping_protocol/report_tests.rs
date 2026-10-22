use super::{
    PingReportInput, PingReportKind, PingReportRow, RowFfi, prodex_mojo_ping_report_v1,
    render_ping_report,
};
use crate::MojoError;
use crate::rich::{PING_STATUS_AUTH_FAILED, PING_STATUS_QUOTA_EXHAUSTED, RichStringView, view};

fn sample() -> PingReportRow<'static> {
    PingReportRow {
        profile: "profile-雪",
        status: PING_STATUS_AUTH_FAILED,
        model: Some("model-\"雪"),
        effort: Some(""),
        effective_model: Some("effective\nmodel"),
        detail: "synthetic reason\n雪\tquoted\"",
        first_response_latency_ms: Some(0),
        latency_ms: Some(u64::MAX as u128),
    }
}

#[test]
fn native_report_bridge_keeps_complete_optional_fields_and_u128_endpoints() {
    let mut row = sample();
    row.first_response_latency_ms = Some(u128::MAX);
    row.latency_ms = Some(u128::MAX);
    let rows = [row];
    let input = PingReportInput {
        rows: &rows,
        kind: PingReportKind::Profile,
        elapsed_ms: 0,
    };
    assert_eq!(
        render_ping_report(input).unwrap(),
        "profile-雪  AUTH_FAILED          first=340282366920938463463374607431768211455ms completion=340282366920938463463374607431768211455ms  requested=model-\"雪 effort= effective=effective\nmodel\n  reason: synthetic reason\n雪\tquoted\""
    );
    assert!(matches!(
        render_ping_report(PingReportInput {
            rows: &rows,
            kind: PingReportKind::SummaryJson,
            elapsed_ms: 0
        }),
        Err(MojoError::InvalidInput)
    ));
}

#[test]
fn native_report_bridge_keeps_complete_empty_and_multirow_json() {
    let empty = render_ping_report(PingReportInput {
        rows: &[],
        kind: PingReportKind::SummaryJson,
        elapsed_ms: 0,
    })
    .unwrap();
    assert_eq!(
        empty,
        "{\"detail\":\"0/0 profiles healthy\",\"effective_model\":null,\"effort\":null,\"latency_ms\":0,\"model\":null,\"profiles\":[],\"provider\":\"openai\",\"requested_effort\":null,\"requested_model\":null,\"status\":\"failed\",\"summary\":{\"auth_failures\":0,\"duration_ms\":0,\"exhausted\":0,\"healthy\":0,\"other_failures\":0,\"pool_usable\":false,\"profiles_discovered\":0,\"profiles_tested\":0,\"temporary_failures\":0}}"
    );
    let rows = [
        sample(),
        PingReportRow {
            profile: "synthetic-quota",
            status: PING_STATUS_QUOTA_EXHAUSTED,
            model: None,
            effort: None,
            effective_model: None,
            detail: "synthetic detail",
            first_response_latency_ms: None,
            latency_ms: None,
        },
    ];
    let json = render_ping_report(PingReportInput {
        rows: &rows,
        kind: PingReportKind::SummaryJson,
        elapsed_ms: u64::MAX as u128,
    })
    .unwrap();
    assert_eq!(
        json,
        "{\"detail\":\"0/2 profiles healthy\",\"effective_model\":\"effective\\nmodel\",\"effort\":\"\",\"latency_ms\":18446744073709551615,\"model\":\"model-\\\"雪\",\"profiles\":[{\"completion_latency_ms\":18446744073709551615,\"credential_validation\":\"failed\",\"detail\":\"synthetic reason\\n雪\\tquoted\\\"\",\"effective_model\":\"effective\\nmodel\",\"effort\":\"\",\"first_response_latency_ms\":0,\"latency_ms\":18446744073709551615,\"model\":\"model-\\\"雪\",\"profile\":\"profile-雪\",\"requested_effort\":\"\",\"requested_model\":\"model-\\\"雪\",\"status\":\"auth_failed\"},{\"completion_latency_ms\":null,\"credential_validation\":\"unknown\",\"detail\":\"synthetic detail\",\"effective_model\":null,\"effort\":null,\"first_response_latency_ms\":null,\"latency_ms\":null,\"model\":null,\"profile\":\"synthetic-quota\",\"requested_effort\":null,\"requested_model\":null,\"status\":\"exhausted\"}],\"provider\":\"openai\",\"requested_effort\":\"\",\"requested_model\":\"model-\\\"雪\",\"status\":\"failed\",\"summary\":{\"auth_failures\":1,\"duration_ms\":18446744073709551615,\"exhausted\":1,\"healthy\":0,\"other_failures\":0,\"pool_usable\":false,\"profiles_discovered\":2,\"profiles_tested\":2,\"temporary_failures\":0}}"
    );
    let human = render_ping_report(PingReportInput {
        rows: &rows,
        kind: PingReportKind::SummaryHuman,
        elapsed_ms: std::time::Duration::MAX.as_millis(),
    })
    .unwrap();
    assert_eq!(
        human,
        "Profiles discovered: 2\nProfiles tested: 2\nHealthy: 0\nExhausted: 1\nAuth failures: 1\nTemporary failures: 0\nOther failures: 0\nTotal duration: 18446744073709551615999ms\nPool usable: no"
    );
}

#[test]
fn native_report_abi_rejects_bad_rows_flags_and_capacity_without_writes() {
    let empty = view("");
    let mut rows = [RowFfi {
        status: 0,
        presence: 0,
        profile: view("synthetic-missing"),
        model: empty,
        effort: empty,
        effective_model: empty,
        detail: empty,
        first_low: 0,
        first_high: 0,
        completion_low: 0,
        completion_high: 0,
    }];
    let mut output = [93_u8; 2048];
    let mut metadata = [77_i64, 88];
    let input = rows.as_mut_ptr() as u64;
    let target = output.as_mut_ptr() as u64;
    let meta = metadata.as_mut_ptr() as u64;
    for (version, kind, address, count, high, phase, capacity, result, expected) in [
        (2, 0, input, 1, 0, 0, 2048, meta, 4),
        (1, 3, input, 1, 0, 0, 2048, meta, 1),
        (1, 0, input, 0, 0, 0, 2048, meta, 1),
        (1, 1, input, 65537, 0, 0, 2048, meta, 1),
        (1, 0, input + 1, 1, 0, 0, 2048, meta, 1),
        (1, 0, input, 1, 0, 2, 2048, meta, 1),
        (1, 0, input, 1, 0, 1, 2048, meta, 1),
        (1, 0, input, 1, 0, 0, 1, meta, 3),
        (1, 0, input, 1, 0, 0, 2048, meta + 1, 1),
        (1, 2, input, 1, 1, 0, 2048, meta, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_mojo_ping_report_v1(
                    version, kind, address, count, 0, high, phase, target, capacity, result,
                )
            },
            expected
        );
        assert_eq!(output, [93; 2048]);
        assert_eq!(metadata, [77, 88]);
    }
    for (status, presence, first_high) in [(15, 0, 0), (0, 32, 0), (0, 8, 1)] {
        rows[0].status = status;
        rows[0].presence = presence;
        rows[0].first_high = first_high;
        assert_eq!(
            unsafe { prodex_mojo_ping_report_v1(1, 2, input, 1, 0, 0, 0, target, 2048, meta) },
            1
        );
        assert_eq!(output, [93; 2048]);
        assert_eq!(metadata, [77, 88]);
    }
    let bad_utf8 = [255_u8];
    rows[0].status = 0;
    rows[0].presence = 0;
    rows[0].profile = RichStringView {
        ptr: bad_utf8.as_ptr() as u64,
        len: 1,
    };
    assert_eq!(
        unsafe {
            prodex_mojo_ping_report_v1(1, 0, rows.as_ptr() as u64, 1, 0, 0, 0, target, 2048, meta)
        },
        1
    );
    assert_eq!(output, [93; 2048]);
    assert_eq!(metadata, [77, 88]);
}
