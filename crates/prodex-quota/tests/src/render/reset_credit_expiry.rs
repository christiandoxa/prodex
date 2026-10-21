use super::*;
use crate::RateLimitResetCreditExpiry;

#[test]
fn reset_credit_expiry_display_uses_local_time_and_explicit_zone() {
    let epoch = chrono::DateTime::parse_from_rfc3339("2026-10-23T01:23:45+07:00")
        .unwrap()
        .timestamp();
    let expected = Local
        .timestamp_opt(epoch, 0)
        .single()
        .unwrap()
        .format("%Y-%m-%d %H:%M:%S %:z")
        .to_string();
    let mut usage = main_windows(80, 1_700_001_800, 90, 1_700_259_200);
    usage.rate_limit_reset_credits = Some(RateLimitResetCreditsSummary {
        available_count: 3,
        next_expiry: RateLimitResetCreditExpiry::At(epoch),
    });
    let suffix = format!("3 available | next expiry: {expected}");
    assert_eq!(
        format_reset_credits(usage.rate_limit_reset_credits.as_ref().unwrap()),
        suffix
    );
    for width in [40, 80, 200] {
        for output in [
            render_profile_quota_with_width("main", &usage, width),
            render_quota_reports_with_layout(
                &[openai_report("main", usage.clone())],
                true,
                None,
                width,
            ),
        ] {
            assert!(
                output.lines().all(|line| text_width(line) <= width),
                "{output}"
            );
            let compact = output.split_whitespace().collect::<String>();
            assert!(
                compact.contains(&suffix.split_whitespace().collect::<String>()),
                "{output}"
            );
            assert_eq!(compact.matches("nextexpiry").count(), 1);
        }
    }
}

#[test]
fn reset_credit_expiry_display_distinguishes_never_unavailable_and_zero() {
    for (count, expiry, expected) in [
        (
            1,
            RateLimitResetCreditExpiry::Never,
            "1 available | next expiry: never",
        ),
        (
            3,
            RateLimitResetCreditExpiry::Unavailable,
            "3 available | next expiry: unavailable",
        ),
        (0, RateLimitResetCreditExpiry::At(42), "0 available"),
        (0, RateLimitResetCreditExpiry::Never, "0 available"),
        (-1, RateLimitResetCreditExpiry::Unavailable, "-1 available"),
        (
            1,
            RateLimitResetCreditExpiry::At(i64::MAX),
            "1 available | next expiry: unavailable",
        ),
    ] {
        assert_eq!(
            format_reset_credits(&RateLimitResetCreditsSummary {
                available_count: count,
                next_expiry: expiry,
            }),
            expected
        );
    }
    let usage = main_windows(80, 1_700_001_800, 90, 1_700_259_200);
    let output = render_profile_quota_with_width("main", &usage, 80);
    assert!(!output.contains("Reset credits"));
    assert!(!output.contains("next expiry"));
}
