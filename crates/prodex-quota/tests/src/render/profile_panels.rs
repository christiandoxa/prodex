use super::*;
use std::collections::BTreeMap;

fn assert_widths(fields: &[(String, String)], render: impl Fn(usize) -> String) {
    for width in [1, 24, 80] {
        assert_eq!(
            render(width),
            render_panel_with_width("Quota main", fields, width)
        );
    }
}

#[test]
fn profile_panels_preserve_openai_rows_and_empty_additional_name() {
    let window = UsageWindow {
        used_percent: Some(10),
        reset_at: None,
        limit_window_seconds: Some(60),
    };
    let pair = WindowPair {
        primary_window: Some(window.clone()),
        ..Default::default()
    };
    let usage = UsageResponse {
        email: Some("".into()),
        plan_type: Some(" \u{2003} ".into()),
        rate_limit: Some(pair.clone()),
        code_review_rate_limit: Some(pair.clone()),
        rate_limit_reset_credits: Some(crate::RateLimitResetCreditsSummary {
            available_count: 2,
            ..Default::default()
        }),
        additional_rate_limits: vec![AdditionalRateLimit {
            limit_name: Some("".into()),
            metered_feature: Some("ignored".into()),
            limit_id: None,
            rate_limit: pair.clone(),
            allowed: None,
            limit_reached: None,
            extra: BTreeMap::new(),
        }],
    };
    let mut fields = vec![
        ("Profile".into(), "main".into()),
        ("Account".into(), "".into()),
        ("Plan".into(), " \u{2003} ".into()),
        ("Status".into(), format_openai_quota_status(&usage)),
        ("Main".into(), format_main_windows(&usage)),
        ("Code review".into(), format_window_pair(&pair)),
        (
            "Reset credits".into(),
            format_reset_credits(usage.rate_limit_reset_credits.as_ref().unwrap()),
        ),
        (" 60s".into(), format_window_details(&window)),
    ];
    assert_widths(&fields, |width| {
        render_profile_quota_with_width("main", &usage, width)
    });
    fields.insert(5, ("Reset".into(), format_main_reset_summary(&usage)));
    assert_eq!(
        render_profile_quota_with_detail("main", &usage, true),
        render_panel_with_width("Quota main", &fields, current_cli_width())
    );
}

#[test]
fn profile_panels_preserve_copilot_access_rules_and_reset_presence() {
    for (plan, expected_plan, access_row) in [
        (None, "sku", true),
        (Some("sku"), "sku", false),
        (Some(""), "", true),
        (Some(" \u{2003} "), " \u{2003} ", true),
    ] {
        let info = CopilotQuotaInfo {
            login: None,
            copilot_plan: plan.map(str::to_owned),
            access_type_sku: Some("sku".into()),
            limited_user_quotas: BTreeMap::new(),
            monthly_quotas: BTreeMap::new(),
            limited_user_reset_date: Some("".into()),
        };
        let mut fields = vec![
            ("Profile".into(), "main".into()),
            ("Account".into(), "-".into()),
            ("Plan".into(), expected_plan.into()),
        ];
        if access_row {
            fields.push(("Access".into(), "sku".into()));
        }
        fields.extend([
            ("Status".into(), format_copilot_quota_status(&info)),
            ("Main".into(), format_copilot_main_quota(&info)),
            ("Reset".into(), "monthly ".into()),
        ]);
        assert_widths(&fields, |width| {
            render_profile_copilot_quota_with_width("main", &info, width)
        });
    }
}

#[test]
fn profile_panels_preserve_gemini_bucket_numbering_and_project_row() {
    let bucket = GeminiQuotaBucket {
        remaining_amount: None,
        remaining_fraction: Some(0.5),
        reset_time: Some("soon".into()),
        token_type: None,
        model_id: Some("models/test-model".into()),
    };
    let info = GeminiQuotaInfo {
        email: None,
        plan: Some("".into()),
        project_id: Some("proj-1".into()),
        buckets: vec![bucket.clone(), bucket],
    };
    let summaries = format_gemini_bucket_summaries(&info);
    assert_eq!(summaries.len(), 2);
    let fields = vec![
        ("Profile".into(), "main".into()),
        ("Account".into(), "-".into()),
        ("Plan".into(), "".into()),
        ("Project".into(), "proj-1".into()),
        ("Status".into(), format_gemini_quota_status(&info)),
        ("Main".into(), format_gemini_main_quota(&info)),
        ("Reset".into(), format_gemini_reset_summary(&info).unwrap()),
        ("Bucket 1".into(), summaries[0].clone()),
        ("Bucket 2".into(), summaries[1].clone()),
    ];
    assert_widths(&fields, |width| {
        render_profile_gemini_quota_with_width("main", &info, width)
    });
}

#[test]
fn profile_panels_preserve_external_empty_reset_and_verbatim_details() {
    let info = ExternalQuotaInfo {
        provider: "Example".into(),
        account: Some("".into()),
        plan: None,
        status: "Ready".into(),
        main: "USD 1.00".into(),
        reset: Some("".into()),
        available: None,
        details: vec![crate::ExternalQuotaDetail {
            label: "週\n\"\\\0".into(),
            value: "value 🧭".into(),
        }],
    };
    let fields = vec![
        ("Profile".into(), "main".into()),
        ("Provider".into(), "Example".into()),
        ("Account".into(), "".into()),
        ("Plan".into(), "-".into()),
        ("Status".into(), "Ready".into()),
        ("Main".into(), "USD 1.00".into()),
        ("Reset".into(), "".into()),
        ("週\n\"\\\0".into(), "value 🧭".into()),
    ];
    assert_widths(&fields, |width| {
        render_profile_external_quota_with_width("main", &info, width)
    });
}
