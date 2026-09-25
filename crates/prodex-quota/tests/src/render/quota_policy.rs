use super::*;

#[test]
fn quota_window_status_matches_rust_oracle() {
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
        let rust = if !has_window {
            RuntimeQuotaWindowStatus::Unknown
        } else if remaining == 0 {
            RuntimeQuotaWindowStatus::Exhausted
        } else if remaining <= 5 {
            RuntimeQuotaWindowStatus::Critical
        } else if remaining <= 15 {
            RuntimeQuotaWindowStatus::Thin
        } else {
            RuntimeQuotaWindowStatus::Ready
        };
        assert_eq!(quota_window_status(remaining, has_window), rust);
    }
}

#[test]
fn quota_pressure_band_matches_rust_oracle() {
    let statuses = [
        RuntimeQuotaWindowStatus::Ready,
        RuntimeQuotaWindowStatus::Thin,
        RuntimeQuotaWindowStatus::Critical,
        RuntimeQuotaWindowStatus::Exhausted,
        RuntimeQuotaWindowStatus::Unknown,
    ];
    for status in statuses {
        let expected = rust_pressure_band(status);
        assert_eq!(quota_pressure_band_from_window_status(status), expected);
    }

    for five_hour in statuses {
        for weekly in statuses {
            let expected = [rust_pressure_band(five_hour), rust_pressure_band(weekly)]
                .into_iter()
                .max()
                .unwrap_or(RuntimeQuotaPressureBand::Unknown);
            assert_eq!(
                quota_pressure_band_from_windows(
                    RuntimeQuotaWindowSummary {
                        status: five_hour,
                        remaining_percent: 0,
                        reset_at: 0,
                    },
                    RuntimeQuotaWindowSummary {
                        status: weekly,
                        remaining_percent: 0,
                        reset_at: 0,
                    },
                ),
                expected
            );
        }
    }
}

fn rust_pressure_band(status: RuntimeQuotaWindowStatus) -> RuntimeQuotaPressureBand {
    match status {
        RuntimeQuotaWindowStatus::Ready => RuntimeQuotaPressureBand::Healthy,
        RuntimeQuotaWindowStatus::Thin => RuntimeQuotaPressureBand::Thin,
        RuntimeQuotaWindowStatus::Critical => RuntimeQuotaPressureBand::Critical,
        RuntimeQuotaWindowStatus::Exhausted => RuntimeQuotaPressureBand::Exhausted,
        RuntimeQuotaWindowStatus::Unknown => RuntimeQuotaPressureBand::Unknown,
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
fn quota_capacity_fails_closed_without_mojo() {
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

    assert!(!window_pair_has_ready_limit(&pair));
    assert!(!openai_quota_has_ready_limit(&usage));
    assert!(openai_quota_runtime_window_pair(&usage).is_none());
    assert!(required_main_window_snapshot_at(&usage, "5h", 1_700_000_000).is_none());
    assert!(!openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-luna")
    ));
    assert!(!openai_usage_has_unknown_luna_capacity(&usage));
    assert!(!openai_usage_supports_model(
        &usage,
        false,
        Some("gpt-5.6-luna")
    ));
    assert!(!additional_rate_limit_is_usable(&additional));
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
    #[cfg(feature = "mojo")]
    assert_eq!(
        summary.five_hour.status,
        RuntimeQuotaWindowStatus::Exhausted
    );
    #[cfg(not(feature = "mojo"))]
    assert_eq!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown);
    #[cfg(feature = "mojo")]
    assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Exhausted);
    #[cfg(not(feature = "mojo"))]
    assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Unknown);
}
