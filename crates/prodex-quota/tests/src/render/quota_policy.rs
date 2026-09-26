use super::*;

#[test]
fn quota_window_status_matches_expected_boundaries() {
    for (remaining, has_window, expected) in [
        (0, false, RuntimeQuotaWindowStatus::Unknown),
        (0, true, RuntimeQuotaWindowStatus::Exhausted),
        (1, true, RuntimeQuotaWindowStatus::Critical),
        (5, true, RuntimeQuotaWindowStatus::Critical),
        (6, true, RuntimeQuotaWindowStatus::Thin),
        (15, true, RuntimeQuotaWindowStatus::Thin),
        (16, true, RuntimeQuotaWindowStatus::Ready),
        (100, true, RuntimeQuotaWindowStatus::Ready),
    ] {
        assert_eq!(quota_window_status(remaining, has_window), expected);
    }
}

#[test]
fn quota_pressure_band_matches_expected_values() {
    let cases = [
        (
            RuntimeQuotaWindowStatus::Ready,
            RuntimeQuotaPressureBand::Healthy,
        ),
        (
            RuntimeQuotaWindowStatus::Thin,
            RuntimeQuotaPressureBand::Thin,
        ),
        (
            RuntimeQuotaWindowStatus::Critical,
            RuntimeQuotaPressureBand::Critical,
        ),
        (
            RuntimeQuotaWindowStatus::Exhausted,
            RuntimeQuotaPressureBand::Exhausted,
        ),
        (
            RuntimeQuotaWindowStatus::Unknown,
            RuntimeQuotaPressureBand::Unknown,
        ),
    ];
    for (status, expected) in cases {
        assert_eq!(quota_pressure_band_from_window_status(status), expected);
    }

    for (five_hour, weekly, expected) in [
        (
            RuntimeQuotaWindowStatus::Ready,
            RuntimeQuotaWindowStatus::Thin,
            RuntimeQuotaPressureBand::Thin,
        ),
        (
            RuntimeQuotaWindowStatus::Thin,
            RuntimeQuotaWindowStatus::Critical,
            RuntimeQuotaPressureBand::Critical,
        ),
        (
            RuntimeQuotaWindowStatus::Exhausted,
            RuntimeQuotaWindowStatus::Ready,
            RuntimeQuotaPressureBand::Exhausted,
        ),
        (
            RuntimeQuotaWindowStatus::Unknown,
            RuntimeQuotaWindowStatus::Ready,
            RuntimeQuotaPressureBand::Unknown,
        ),
    ] {
        assert_eq!(
            quota_pressure_band_from_windows(
                RuntimeQuotaWindowSummary {
                    status: five_hour,
                    remaining_percent: 0,
                    reset_at: 0
                },
                RuntimeQuotaWindowSummary {
                    status: weekly,
                    remaining_percent: 0,
                    reset_at: 0
                },
            ),
            expected
        );
    }
}

#[test]
fn quota_remaining_and_rounding_match_expected_boundaries() {
    for (used, expected) in [
        (None, 0),
        (Some(i64::MIN), 100),
        (Some(-1), 100),
        (Some(0), 100),
        (Some(42), 58),
        (Some(100), 0),
        (Some(101), 0),
        (Some(i64::MAX), 0),
    ] {
        assert_eq!(remaining_percent(used), expected);
    }
    for (value, expected) in [
        (-2.5, -3),
        (-0.5, -1),
        (0.5, 1),
        (2.5, 3),
        (f64::NAN, 0),
        (f64::INFINITY, i64::MAX),
        (f64::NEG_INFINITY, i64::MIN),
    ] {
        assert_eq!(round_quota_float(value), expected);
    }
}

#[cfg(feature = "mojo")]
#[test]
fn quota_window_pair_readiness_matches_expected_values() {
    for (first, second, expected) in [
        (None, None, false),
        (Some(0), None, true),
        (Some(99), Some(100), false),
        (Some(100), Some(0), false),
        (Some(-1), Some(99), true),
        (Some(101), Some(0), false),
    ] {
        let pair = WindowPair {
            allowed: None,
            limit_reached: None,
            extra: std::collections::BTreeMap::new(),
            primary_window: first.map(|used_percent| UsageWindow {
                used_percent: Some(used_percent),
                reset_at: None,
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: second.map(|used_percent| UsageWindow {
                used_percent: Some(used_percent),
                reset_at: None,
                limit_window_seconds: Some(604_800),
            }),
        };
        assert_eq!(window_pair_has_ready_limit(&pair), expected);
    }
}

#[cfg(feature = "mojo")]
#[test]
fn quota_admission_uses_expected_upstream_flags() {
    let pair = |allowed, limit_reached, fields: &[(&str, serde_json::Value)]| {
        let mut extra = std::collections::BTreeMap::new();
        for (key, value) in fields {
            extra.insert((*key).to_string(), value.clone());
        }
        WindowPair {
            allowed,
            limit_reached,
            extra,
            primary_window: Some(UsageWindow {
                used_percent: Some(10),
                reset_at: None,
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }
    };
    let cases = [
        (pair(None, None, &[]), false),
        (pair(Some(true), None, &[]), false),
        (pair(Some(false), None, &[]), true),
        (pair(None, Some(false), &[]), false),
        (pair(None, Some(true), &[]), true),
        (
            pair(
                None,
                None,
                &[("rate_limit_reached_type", serde_json::json!("reached"))],
            ),
            true,
        ),
        (
            pair(
                None,
                None,
                &[("rateLimitReachedType", serde_json::json!(null))],
            ),
            false,
        ),
        (
            pair(
                None,
                None,
                &[("rateLimitReachedType", serde_json::json!(false))],
            ),
            true,
        ),
        (
            pair(
                None,
                None,
                &[("spend_control_reached", serde_json::json!(true))],
            ),
            true,
        ),
        (
            pair(
                None,
                None,
                &[("spendControlReached", serde_json::json!(false))],
            ),
            false,
        ),
        (
            pair(
                None,
                None,
                &[("spendControlReached", serde_json::json!("true"))],
            ),
            false,
        ),
        (
            pair(
                None,
                None,
                &[("ordinaryUsageAllowed", serde_json::json!(false))],
            ),
            true,
        ),
        (
            pair(
                None,
                None,
                &[("ordinaryUsageAllowed", serde_json::json!(true))],
            ),
            false,
        ),
        (
            pair(
                None,
                None,
                &[("ordinaryUsageAllowed", serde_json::json!(null))],
            ),
            true,
        ),
    ];

    for (pair, expected) in cases {
        assert_eq!(
            crate::render::windows::window_pair_has_blocking_admission(&pair),
            expected
        );
        assert_eq!(window_pair_has_ready_limit(&pair), !expected);
    }

    let additional = |allowed, limit_reached| AdditionalRateLimit {
        limit_id: None,
        limit_name: None,
        metered_feature: None,
        rate_limit: pair(None, None, &[]),
        allowed,
        limit_reached,
        extra: std::collections::BTreeMap::new(),
    };
    assert!(additional_rate_limit_is_usable(&additional(None, None)));
    assert!(!additional_rate_limit_is_usable(&additional(
        Some(false),
        None
    )));
    assert!(!additional_rate_limit_is_usable(&additional(
        None,
        Some(true)
    )));
}

#[cfg(not(feature = "mojo"))]
#[test]
fn quota_windows_use_mojo_without_renderer_feature() {
    let pair = WindowPair {
        allowed: None,
        limit_reached: None,
        extra: std::collections::BTreeMap::new(),
        primary_window: Some(UsageWindow {
            used_percent: Some(10),
            reset_at: Some(1_700_003_600),
            limit_window_seconds: Some(18_000),
        }),
        secondary_window: None,
    };
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(pair.clone()),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };
    let additional = AdditionalRateLimit {
        limit_id: None,
        limit_name: None,
        metered_feature: None,
        rate_limit: pair.clone(),
        allowed: None,
        limit_reached: None,
        extra: std::collections::BTreeMap::new(),
    };

    assert!(window_pair_has_ready_limit(&pair));
    assert!(openai_quota_has_ready_limit(&usage));
    assert!(std::ptr::eq(
        openai_quota_runtime_window_pair(&usage).unwrap(),
        usage.rate_limit.as_ref().unwrap()
    ));
    let snapshot = required_main_window_snapshot_at(&usage, "5h", 1_700_000_000).unwrap();
    assert_eq!(snapshot.remaining_percent, 90);
    assert_eq!(snapshot.reset_at, 1_700_003_600);
    assert_eq!(snapshot.pressure_score, 40_000);
    assert!(openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-luna")
    ));
    assert!(openai_model_is_luna(Some("gpt-5.6-luna")));
    assert!(openai_model_is_retired_spark(Some("gpt-5.3-codex-spark")));
    assert!(std::ptr::eq(
        openai_quota_runtime_window_pair_for_model(&usage, Some("gpt-5.6-luna")).unwrap(),
        usage.rate_limit.as_ref().unwrap()
    ));

    let mut reserve = additional.clone();
    reserve.limit_name = Some("Luna Reserve".to_string());
    reserve.extra.insert(
        "normalModelSlug".to_string(),
        serde_json::json!("gpt-5.6-luna"),
    );
    assert!(additional_rate_limit_is_luna_reserve(&reserve));

    assert!(!openai_usage_has_unknown_luna_capacity(&usage));
    assert!(openai_usage_supports_model(
        &usage,
        false,
        Some("gpt-5.6-luna")
    ));

    let mut unknown_usage = usage.clone();
    unknown_usage
        .rate_limit
        .as_mut()
        .unwrap()
        .primary_window
        .as_mut()
        .unwrap()
        .used_percent = None;
    assert!(!openai_usage_has_unknown_luna_capacity(&unknown_usage));
    assert!(!openai_usage_supports_model(
        &unknown_usage,
        false,
        Some("gpt-5.6-luna")
    ));
    assert!(additional_rate_limit_is_usable(&additional));
}

#[test]
fn quota_summary_marks_exhausted_window() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            allowed: None,
            limit_reached: None,
            extra: std::collections::BTreeMap::new(),
            primary_window: Some(UsageWindow {
                used_percent: Some(100),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(30),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };

    let summary = quota_summary(&usage);
    assert_eq!(
        summary.five_hour.status,
        RuntimeQuotaWindowStatus::Exhausted
    );
    assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Exhausted);
}
