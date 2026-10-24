#![cfg(feature = "mojo-quota")]

use prodex_mojo_core::quota::{
    QuotaBlockedWindowInput, QuotaBlockedWindowKind, quota_blocked_windows_json,
};

#[test]
fn required_mojo_renders_main_additional_and_code_review_blocked_rows() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };
    let inputs = [
        QuotaBlockedWindowInput {
            kind: QuotaBlockedWindowKind::Main,
            used_percent: None,
            seconds: Some(18_000),
            names: [None; 3],
            reset: "-",
        },
        QuotaBlockedWindowInput {
            kind: QuotaBlockedWindowKind::Main,
            used_percent: Some(100),
            seconds: Some(604_800),
            names: [None; 3],
            reset: "tomorrow",
        },
        QuotaBlockedWindowInput {
            kind: QuotaBlockedWindowKind::Additional,
            used_percent: Some(100),
            seconds: Some(60),
            names: [Some("burst"), Some("ignored"), None],
            reset: "soon",
        },
        QuotaBlockedWindowInput {
            kind: QuotaBlockedWindowKind::CodeReview,
            used_percent: Some(101),
            seconds: None,
            names: [None; 3],
            reset: "-",
        },
    ];
    assert_eq!(
        quota_blocked_windows_json(false, true, true, &inputs).unwrap(),
        r#"["5h quota unknown","weekly exhausted until tomorrow","burst 60s exhausted until soon","code-review usage exhausted until -"]"#
    );
    assert_eq!(
        quota_blocked_windows_json(true, true, true, &inputs).unwrap(),
        r#"["code-review usage exhausted until -"]"#
    );
    assert_eq!(
        quota_blocked_windows_json(false, true, false, &inputs).unwrap(),
        r#"["5h quota unknown","weekly exhausted until tomorrow","burst 60s exhausted until soon"]"#
    );
    assert_eq!(
        quota_blocked_windows_json(true, true, false, &inputs).unwrap(),
        "[]"
    );
}

#[test]
fn required_mojo_distinguishes_missing_and_empty_main_quota() {
    assert_eq!(
        quota_blocked_windows_json(false, false, false, &[]).unwrap(),
        r#"["5h quota unavailable","weekly quota unavailable"]"#
    );
    assert_eq!(
        quota_blocked_windows_json(false, true, false, &[]).unwrap(),
        r#"["quota unavailable"]"#
    );
    let input = QuotaBlockedWindowInput {
        kind: QuotaBlockedWindowKind::Additional,
        used_percent: Some(99),
        seconds: Some(60),
        names: [None; 3],
        reset: "-",
    };
    for used_percent in [None, Some(i64::MIN), Some(0), Some(99)] {
        assert_eq!(
            quota_blocked_windows_json(
                false,
                true,
                false,
                &[QuotaBlockedWindowInput {
                    used_percent,
                    ..input
                }]
            )
            .unwrap(),
            r#"["quota unavailable"]"#
        );
    }
}

#[test]
fn required_mojo_preserves_empty_name_precedence_and_escapes_borrowed_text() {
    let input = QuotaBlockedWindowInput {
        kind: QuotaBlockedWindowKind::Additional,
        used_percent: Some(i64::MAX),
        seconds: Some(60),
        names: [None; 3],
        reset: "週\n\"\\\0",
    };
    for (names, prefix) in [
        ([Some(""), Some("feature"), Some("id")], ""),
        ([None, Some("feature"), Some("id")], "feature "),
        ([None, Some(""), Some("id")], ""),
        ([None, None, Some("id")], "id "),
        ([None; 3], ""),
    ] {
        assert_eq!(
            quota_blocked_windows_json(
                false,
                true,
                false,
                &[QuotaBlockedWindowInput { names, ..input }]
            )
            .unwrap(),
            format!(r#"["{prefix}60s exhausted until 週\n\"\\\u0000"]"#)
        );
    }
}

#[test]
fn required_mojo_blocked_windows_preserve_order_past_former_batch_boundary() {
    let names = (0..2_049)
        .map(|index| format!("lane-{index}"))
        .collect::<Vec<_>>();
    let inputs = names
        .iter()
        .enumerate()
        .map(|(index, name)| QuotaBlockedWindowInput {
            kind: QuotaBlockedWindowKind::Additional,
            used_percent: Some(if index % 2 == 0 { 100 } else { 0 }),
            seconds: Some(60),
            names: [Some(name), None, None],
            reset: "-",
        })
        .collect::<Vec<_>>();
    let expected = names
        .iter()
        .step_by(2)
        .map(|name| format!("\"{name} 60s exhausted until -\""))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        quota_blocked_windows_json(false, true, false, &inputs).unwrap(),
        format!("[{expected}]")
    );
}

#[test]
fn required_mojo_blocked_messages_share_window_labels_and_signed_duration_edges() {
    for (seconds, label) in [
        (None, "usage"),
        (Some(i64::MIN), "-9223372036854775808s"),
        (Some(0), "0s"),
        (Some(17_700), "5h"),
        (Some(18_300), "5h"),
        (Some(601_200), "weekly"),
        (Some(608_400), "weekly"),
        (Some(2_505_600), "monthly"),
        (Some(2_678_400), "monthly"),
        (Some(i64::MAX), "9223372036854775807s"),
    ] {
        let input = QuotaBlockedWindowInput {
            kind: QuotaBlockedWindowKind::Additional,
            used_percent: Some(100),
            seconds,
            names: [None; 3],
            reset: "-",
        };
        assert_eq!(
            quota_blocked_windows_json(false, true, false, &[input]).unwrap(),
            format!(r#"["{label} exhausted until -"]"#)
        );
    }
}
