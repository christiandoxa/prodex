use super::{
    QuotaTuiLine, QuotaTuiTone, prodex_quota_tui_span_plan_v1, quota_tui_line_plan,
    quota_tui_status_tone,
};

#[test]
fn quota_tui_bridge_preserves_precedence_and_rejects_invalid_abi_without_writes() {
    for (text, line, cell) in [
        ("", QuotaTuiTone::Muted, QuotaTuiTone::Raw),
        (
            "Quota Error Ready",
            QuotaTuiTone::Title,
            QuotaTuiTone::Error,
        ),
        (
            "PROFILE Ready critical",
            QuotaTuiTone::Success,
            QuotaTuiTone::Success,
        ),
        ("PROFILE names", QuotaTuiTone::Bold, QuotaTuiTone::Raw),
        ("thin", QuotaTuiTone::Error, QuotaTuiTone::Raw),
        (
            "  workspace: value",
            QuotaTuiTone::Detail,
            QuotaTuiTone::Raw,
        ),
        (
            "\u{001c}workspace: value",
            QuotaTuiTone::Raw,
            QuotaTuiTone::Raw,
        ),
        ("unhealthy", QuotaTuiTone::Success, QuotaTuiTone::Success),
    ] {
        assert_eq!(
            quota_tui_line_plan(text).unwrap(),
            QuotaTuiLine::Whole(line)
        );
        assert_eq!(quota_tui_status_tone(text).unwrap(), cell);
    }
    let mut output = [77_i64; 3];
    let output_address = output.as_mut_ptr() as u64;
    // SAFETY: all calls either borrow this array or are rejected before dereferencing.
    unsafe {
        assert_eq!(
            prodex_quota_tui_span_plan_v1(2, 0, 0, 0, output_address, 3),
            4
        );
        assert_eq!(
            prodex_quota_tui_span_plan_v1(1, 2, 0, 0, output_address, 3),
            1
        );
        assert_eq!(
            prodex_quota_tui_span_plan_v1(1, 0, 0, 1, output_address, 3),
            1
        );
        assert_eq!(
            prodex_quota_tui_span_plan_v1(1, 0, 0, 0, output_address + 1, 3),
            1
        );
        assert_eq!(
            prodex_quota_tui_span_plan_v1(1, 0, 0, 0, output_address, 2),
            3
        );
    }
    assert_eq!(output, [77; 3]);
}

#[test]
fn quota_tui_bridge_preserves_borrowed_unicode_fields_controls_and_large_text() {
    let text = "\u{2003}Account:\t 雪\0:more ";
    assert_eq!(
        quota_tui_line_plan(text).unwrap(),
        QuotaTuiLine::Field {
            label: "\u{2003}Account:",
            value: "雪\0:more "
        }
    );
    let text = format!("Status: {} ", "x".repeat(64 * 1024 * 1024 + 1));
    let QuotaTuiLine::Field { label, value } = quota_tui_line_plan(&text).unwrap() else {
        panic!("large status field must remain a field");
    };
    assert_eq!(label, "Status:");
    assert_eq!(value.len(), 64 * 1024 * 1024 + 2);
    assert_eq!(value.as_ptr(), text[8..].as_ptr());
    assert!(value.ends_with(' '));
}
