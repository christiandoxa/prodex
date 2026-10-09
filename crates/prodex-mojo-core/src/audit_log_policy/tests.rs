use super::*;

#[test]
fn audit_usage_policy_preserves_normalization_summary_and_limits() {
    assert_eq!(
        normalize_usage_token("  Project A β  ", "unknown", 64).unwrap(),
        "project-a"
    );
    assert_eq!(
        normalize_usage_token(" β ", "global", 64).unwrap(),
        "global"
    );
    assert_eq!(
        normalized_total_tokens(0, u64::MAX, 1, 7).unwrap(),
        u64::MAX
    );
    assert_eq!(normalized_total_tokens(9, 1, 2, 3).unwrap(), 9);
    assert_eq!(
        budget_window_plan(0, 3_800).unwrap(),
        AuditBudgetWindowPlan {
            calendar_month: false,
            fallback_start_epoch: 3_600,
        }
    );
    assert_eq!(
        budget_window_plan(0, -1).unwrap().fallback_start_epoch,
        -3_600
    );
    assert_eq!(
        budget_window_plan(0, i64::MIN)
            .unwrap()
            .fallback_start_epoch,
        i64::MIN
    );
    assert_eq!(
        budget_window_plan(3, 3_800).unwrap(),
        AuditBudgetWindowPlan {
            calendar_month: true,
            fallback_start_epoch: 0,
        }
    );

    let summary = summarize_usage(
        &[
            AuditUsageRowInput {
                recorded_at_epoch: 10,
                input_tokens: 2,
                output_tokens: 3,
                cached_input_tokens: 4,
                reasoning_tokens: 5,
                total_tokens: 10,
                cost_micros: 7,
            },
            AuditUsageRowInput {
                recorded_at_epoch: 20,
                input_tokens: u64::MAX,
                output_tokens: 1,
                cached_input_tokens: 1,
                reasoning_tokens: 1,
                total_tokens: u64::MAX,
                cost_micros: u64::MAX,
            },
        ],
        10,
        20,
    )
    .unwrap();
    assert_eq!(summary.requests, 2);
    assert_eq!(summary.input_tokens, u64::MAX);
    assert_eq!(summary.total_tokens, u64::MAX);
    assert_eq!(summary.cost_micros, u64::MAX);
    assert_eq!(
        budget_flags(summary, Some(2), Some(u64::MAX), None).unwrap(),
        AuditBudgetFlags {
            request_limit_reached: true,
            token_limit_reached: true,
            cost_limit_reached: false,
        }
    );
    assert_eq!(
        budget_evaluation(summary, Some(2), Some(u64::MAX), None).unwrap(),
        AuditBudgetEvaluationPlan {
            allowed: false,
            reasons: vec![
                "request limit reached (2/2)".to_string(),
                format!("token limit reached ({}/{})", u64::MAX, u64::MAX),
            ],
        }
    );
    assert_eq!(
        budget_evaluation(
            AuditUsageSummary {
                requests: 1,
                total_tokens: 2,
                cost_micros: 3,
                ..AuditUsageSummary::default()
            },
            Some(9),
            Some(9),
            Some(9),
        )
        .unwrap(),
        AuditBudgetEvaluationPlan {
            allowed: true,
            reasons: Vec::new(),
        }
    );
    assert!(query_has_filters(Some("profile"), None, None).unwrap());
    assert!(!query_has_filters(None, None, None).unwrap());
    assert!(
        query_matches(
            Some("profile"),
            None,
            Some("success"),
            "profile",
            "add",
            "success",
        )
        .unwrap()
    );
    assert!(!query_matches(Some("runtime"), None, None, "profile", "add", "success",).unwrap());
    assert_eq!(
        format_query(Some("profile"), None, Some("success")).unwrap(),
        "component=profile outcome=success"
    );
    assert_eq!(format_query(None, None, None).unwrap(), "none");
    assert_eq!(
        format_search_scope(512, 1024, 512, 512, true).unwrap(),
        "searched 512 of 1024 bytes (byte range 512..1024) limited to last 512 bytes"
    );
    assert_eq!(truncate_text("αβγδε", 3).unwrap(), "αβγ...");
    assert_eq!(truncate_text("αβγ", 3).unwrap(), "αβγ");
    assert_eq!(
        profile_name(Some("  Team β  ")).unwrap(),
        Some("Team β".to_string())
    );
    assert_eq!(profile_name(Some("   ")).unwrap(), None);
    assert_eq!(
        account_hint(Some(" demo-account-1234 ")).unwrap(),
        Some("...1234".to_string())
    );
    assert_eq!(
        account_hint(Some("🙂αβγδε")).unwrap(),
        Some("...βγδε".to_string())
    );
    assert_eq!(
        email_domain(Some(" Example@Sub.DOMAIN.TEST ")).unwrap(),
        Some("sub.domain.test".to_string())
    );
    assert_eq!(
        email_domain(Some("local@ignored@Example.COM")).unwrap(),
        Some("example.com".to_string())
    );
    assert_eq!(email_domain(Some("missing-at")).unwrap(), None);
}

#[test]
fn audit_line_window_drops_partial_prefix_and_keeps_requested_tail() {
    assert_eq!(
        line_window_start("partial\nfirst\r\nsecond\r\n", true, Some(1)).unwrap(),
        15
    );
    assert_eq!(
        line_window_start("first\nsecond\nthird", false, Some(2)).unwrap(),
        6
    );
    assert_eq!(line_window_start("first\n\n", false, Some(1)).unwrap(), 6);
    assert_eq!(line_window_start("partial", true, None).unwrap(), 7);
    assert_eq!(line_window_start("\ncomplete", true, None).unwrap(), 0);
    assert_eq!(tail_start_index(4, 2).unwrap(), 2);
    assert_eq!(tail_start_index(4, 0).unwrap(), 4);
    assert_eq!(tail_start_index(2, usize::MAX).unwrap(), 0);
    // Mojo returns a byte offset; multibyte prefixes must not be split.
    let unicode = "\u{1f510}\u{3b1}\nlast\n";
    assert_eq!(line_window_start(unicode, false, Some(1)).unwrap(), 7);
    assert_eq!(line_window_start(unicode, true, None).unwrap(), 7);
    assert_eq!(
        line_window_start("one\r\ntwo\r\n", false, Some(1)).unwrap(),
        5
    );
    assert_eq!(line_window_start("", false, Some(0)).unwrap(), 0);
    assert_eq!(tail_start_index(0, 0).unwrap(), 0);
    assert_eq!(tail_start_index(1, 1).unwrap(), 0);
    let oversized = "x".repeat(1_048_577);
    assert_eq!(
        line_window_start(&oversized, false, None),
        Err(MojoError::InvalidInput)
    );
}
