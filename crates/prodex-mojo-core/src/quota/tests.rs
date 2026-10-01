use super::*;

#[cfg(all(test, feature = "mojo-quota"))]
#[test]
fn quota_model_policy_kernel_preserves_expected_contracts() {
    assert_eq!(plan_capacity_pressure_scale_bps(" Pro-20x "), Ok(2_000));
    assert_eq!(scale_quota_pressure_for_plan(-10, 5_000), Ok(-5));
    assert_eq!(quota_report_sort_next(5), Ok(0));
    assert_eq!(
        quota_report_compare(0, true, false, 1, 0, 0, 0, "", "", "", "", "", "", "", "",).unwrap(),
        -1
    );
    assert_eq!(
        quota_report_compare(
            1, false, false, 0, 0, 10, 20, "", "", "", "", "", "", "", "",
        )
        .unwrap(),
        -1
    );
    assert_eq!(
        quota_report_compare(
            2, false, false, 0, 0, 0, 0, " Beta ", "alpha", "", "", "", "", "", "",
        )
        .unwrap(),
        1
    );
    assert_eq!(
        quota_report_compare(
            3, false, false, 0, 0, 0, 0, "", "", " ZZZ ", "aaa", "", "", "", "",
        )
        .unwrap(),
        1
    );
    assert_eq!(
        quota_report_compare(
            4, false, false, 0, 0, 0, 0, "", "", "", "", " ALPHA ", "alpha", "", "",
        )
        .unwrap(),
        0
    );
    assert_eq!(quota_report_sort_label(0), Ok("current"));
    assert_eq!(quota_report_sort_label(5), Ok("plan"));
    assert_eq!(
        quota_blocked_status_label(QUOTA_BLOCKED_KIND_NONE),
        Ok("Unavailable")
    );
    assert_eq!(
        quota_blocked_status_label(QUOTA_BLOCKED_KIND_FIVE_HOUR),
        Ok("Blocked 5h")
    );
    assert_eq!(quota_usage_auth_sync_source_label(0), Ok("reloaded"));
    assert_eq!(quota_usage_auth_sync_source_label(1), Ok("refreshed"));
    assert_eq!(quota_window_label(None), Ok("usage".to_string()));
    assert_eq!(quota_window_label(Some(17_700)), Ok("5h".to_string()));
    assert_eq!(quota_window_label(Some(18_300)), Ok("5h".to_string()));
    assert_eq!(quota_window_label(Some(601_200)), Ok("weekly".to_string()));
    assert_eq!(
        quota_window_label(Some(2_678_400)),
        Ok("monthly".to_string())
    );
    assert_eq!(quota_window_label(Some(42)), Ok("42s".to_string()));
    assert_eq!(
        quota_workspace_label(Some("  Personal  "), Some("ignored")).unwrap(),
        Some("Personal".to_string())
    );
    assert_eq!(
        quota_workspace_label(None, Some("abcdefghijklmnopqrstuvwxy123456")).unwrap(),
        Some("abcdefghijkl...123456".to_string())
    );
    assert_eq!(
        quota_workspace_label(None, Some("αβγδεζηθικλμνξοπρστυφχψω123456")).unwrap(),
        Some("αβγδεζηθικλμ...123456".to_string())
    );
    assert_eq!(
        quota_workspace_label(Some("   "), Some(" short-id ")).unwrap(),
        Some("short-id".to_string())
    );
    assert_eq!(quota_copilot_feature_key(0), Ok("chat"));
    assert_eq!(quota_copilot_feature_key(1), Ok("completions"));
    assert_eq!(
        quota_copilot_display(Some(450), Some(500), Some(4_000), Some(4_000)).unwrap(),
        CopilotQuotaDisplay {
            ready: true,
            status: "Ready".to_string(),
            main: "chat 450/500 | comp 4000/4000".to_string(),
        }
    );
    assert_eq!(
        quota_copilot_display(Some(0), Some(500), None, None).unwrap(),
        CopilotQuotaDisplay {
            ready: false,
            status: "Blocked".to_string(),
            main: "chat 0/500".to_string(),
        }
    );
    assert_eq!(
        quota_copilot_display(None, None, None, None).unwrap(),
        CopilotQuotaDisplay {
            ready: true,
            status: "Ready".to_string(),
            main: "-".to_string(),
        }
    );
    assert_eq!(
        quota_gemini_bucket_label(Some(" models/gemini-3.5-flash "), Some("IGNORED")).unwrap(),
        "gemini-3.5-flash"
    );
    assert_eq!(
        quota_gemini_bucket_label(None, Some(" TEXT ")).unwrap(),
        "text"
    );
    assert_eq!(quota_gemini_bucket_label(None, None).unwrap(), "gemini");
    let gemini_numeric = [
        GeminiBucketNumericOutput {
            remaining: Some(50),
            total: Some(100),
            remaining_percent: Some(50),
            exhausted: false,
        },
        GeminiBucketNumericOutput {
            remaining: Some(25),
            total: None,
            remaining_percent: Some(25),
            exhausted: false,
        },
    ];
    assert_eq!(
        quota_gemini_bucket_summary("gemini-a", gemini_numeric[0]).unwrap(),
        "gemini-a 50/100"
    );
    assert_eq!(
        quota_gemini_display(&gemini_numeric).unwrap(),
        GeminiQuotaDisplay {
            ready: true,
            status: "Ready".to_string(),
            main: "gemini 25% (2 buckets)".to_string(),
            remaining_percent: Some(25),
        }
    );
    assert_eq!(
        quota_gemini_display(&[]).unwrap(),
        GeminiQuotaDisplay {
            ready: false,
            status: "Unknown".to_string(),
            main: "-".to_string(),
            remaining_percent: None,
        }
    );
    assert_eq!(
        quota_auth_summary_kind(Some(" Bedrock_API-Key "), false, false, false).unwrap(),
        QuotaAuthSummaryKind::BedrockApiKey
    );
    assert_eq!(
        quota_auth_summary_kind(Some("api-key"), true, true, false).unwrap(),
        QuotaAuthSummaryKind::Chatgpt
    );
    assert_eq!(
        quota_usage_auth_kind(Some("chatgpt"), true, false).unwrap(),
        QuotaUsageAuthKind::ApiKey
    );
    assert_eq!(
        quota_usage_auth_kind(Some("api-key"), false, true).unwrap(),
        QuotaUsageAuthKind::BedrockApiKey
    );
    assert!(quota_auth_needs_proactive_refresh(Some(110), None, 100, 10, 8).unwrap());
    assert!(quota_auth_needs_proactive_refresh(None, Some(0), 8 * 86_400, 10, 8).unwrap());
    let filter = quota_auth_filter_parse(" CHATGPT ").expect("filter parse");
    assert_eq!(filter, QuotaAuthFilterPlan::Label("chatgpt".to_string()));
    assert_eq!(
        quota_auth_filter_matches(1, "chatgpt", "ChatGPT", true),
        Ok(true)
    );
}

#[cfg(all(test, feature = "mojo-quota"))]
#[test]
fn round_f64_matches_rust_float_to_int_semantics() {
    for value in [
        0.0,
        -0.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -2.5,
        -2.499_999_999,
        -0.500_000_001,
        -0.5,
        -0.499_999_999,
        0.000_000_001,
        0.499_999_999,
        0.5,
        0.500_000_001,
        1.5,
        2.5,
        (i64::MAX as f64) * 0.5,
        i64::MAX as f64,
        i64::MIN as f64,
    ] {
        assert_eq!(round_f64(value), value.round() as i64, "value={value:?}");
    }
}

#[cfg(all(test, feature = "mojo-quota"))]
#[test]
fn gemini_bucket_batch_preserves_normalized_presence_states() {
    let outputs = gemini_bucket_numeric_batch(&[
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Parsed(50),
            remaining_fraction: Some(0.5),
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Absent,
            remaining_fraction: Some(0.5),
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Parsed(50),
            remaining_fraction: None,
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Invalid,
            remaining_fraction: Some(0.5),
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Parsed(0),
            remaining_fraction: Some(0.0),
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Absent,
            remaining_fraction: Some(f64::NAN),
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Parsed(50),
            remaining_fraction: Some(2.0),
        },
    ])
    .expect("valid normalized Gemini input");
    assert_eq!(
        outputs,
        [
            GeminiBucketNumericOutput {
                remaining: Some(50),
                total: Some(100),
                remaining_percent: Some(50),
                exhausted: false,
            },
            GeminiBucketNumericOutput {
                remaining: Some(50),
                total: Some(100),
                remaining_percent: Some(50),
                exhausted: false,
            },
            GeminiBucketNumericOutput {
                remaining: Some(50),
                total: None,
                remaining_percent: None,
                exhausted: false,
            },
            GeminiBucketNumericOutput {
                remaining: None,
                total: None,
                remaining_percent: Some(50),
                exhausted: false,
            },
            GeminiBucketNumericOutput {
                remaining: Some(0),
                total: None,
                remaining_percent: Some(0),
                exhausted: true,
            },
            GeminiBucketNumericOutput {
                remaining: Some(0),
                total: Some(100),
                remaining_percent: Some(0),
                exhausted: true,
            },
            GeminiBucketNumericOutput {
                remaining: Some(50),
                total: None,
                remaining_percent: Some(200),
                exhausted: false,
            },
        ]
    );
}
