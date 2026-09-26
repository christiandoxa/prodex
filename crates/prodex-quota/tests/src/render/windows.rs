use super::*;
use std::collections::BTreeMap;

fn window(used: Option<i64>, reset_at: Option<i64>, seconds: Option<i64>) -> UsageWindow {
    UsageWindow {
        used_percent: used,
        reset_at,
        limit_window_seconds: seconds,
    }
}

fn pair(primary: Option<UsageWindow>, secondary: Option<UsageWindow>) -> WindowPair {
    WindowPair {
        allowed: None,
        limit_reached: None,
        extra: BTreeMap::new(),
        primary_window: primary,
        secondary_window: secondary,
    }
}

fn usage(
    rate_limit: Option<WindowPair>,
    additional_rate_limits: Vec<AdditionalRateLimit>,
) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: Some("plus".to_string()),
        rate_limit,
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits,
    }
}

#[test]
fn quota_windows_handle_missing_and_invalid_observations() {
    let missing = pair(None, None);
    let invalid = pair(Some(window(Some(10), Some(2_000), Some(60))), None);
    let unknown = pair(Some(window(None, Some(2_000), Some(18_000))), None);

    for pair in [&missing, &invalid, &unknown] {
        assert!(!window_pair_has_ready_limit(pair));
    }
    assert!(required_window_snapshot_at(&missing, "5h", 1_000).is_none());
    assert!(required_window_snapshot_at(&invalid, "5h", 1_000).is_none());
    assert!(required_window_snapshot_at(&unknown, "5h", 1_000).is_none());
}

#[test]
fn quota_windows_clamp_observations_and_preserve_reset_pressure() {
    let zero = pair(Some(window(Some(0), None, Some(18_000))), None);
    assert!(window_pair_has_ready_limit(&zero));
    let snapshot = required_window_snapshot_at(&zero, "5h", 1_000).unwrap();
    assert_eq!(snapshot.remaining_percent, 100);
    assert_eq!(snapshot.reset_at, i64::MAX);
    assert_eq!(snapshot.pressure_score, 92_233_720_368_547_758);

    let over = pair(Some(window(Some(101), Some(2_000), Some(18_000))), None);
    assert!(!window_pair_has_ready_limit(&over));
    let snapshot = required_window_snapshot_at(&over, "5h", 1_000).unwrap();
    assert_eq!(snapshot.remaining_percent, 0);
    assert_eq!(snapshot.reset_at, 2_000);
    assert_eq!(snapshot.pressure_score, 1_000_000);

    let negative = pair(Some(window(Some(-1), Some(2_000), Some(18_000))), None);
    assert!(window_pair_has_ready_limit(&negative));
    let snapshot = required_window_snapshot_at(&negative, "5h", 1_000).unwrap();
    assert_eq!(snapshot.remaining_percent, 100);
    assert_eq!(snapshot.pressure_score, 10_000);

    let expired = pair(Some(window(Some(80), Some(900), Some(18_000))), None);
    assert_eq!(
        required_window_snapshot_at(&expired, "5h", 1_000)
            .unwrap()
            .pressure_score,
        0
    );
}

#[test]
fn quota_window_hold_keeps_only_exhausted_observation() {
    for (used, expected) in [(10, None), (100, Some((0, 2_000, 1_000_000)))] {
        let mut pair = pair(Some(window(Some(used), Some(2_000), Some(18_000))), None);
        pair.allowed = Some(false);
        assert!(!window_pair_has_ready_limit(&pair));
        let actual = required_window_snapshot_at(&pair, "5h", 1_000).map(|snapshot| {
            (
                snapshot.remaining_percent,
                snapshot.reset_at,
                snapshot.pressure_score,
            )
        });
        assert_eq!(actual, expected);
    }

    let mut usage_hold = pair(Some(window(Some(10), Some(2_000), Some(18_000))), None);
    usage_hold
        .extra
        .insert("ordinaryUsageAllowed".to_string(), serde_json::json!(false));
    assert!(!window_pair_has_ready_limit(&usage_hold));
    assert!(required_window_snapshot_at(&usage_hold, "5h", 1_000).is_none());
}

#[test]
fn quota_window_selection_keeps_unknown_additional_lane_non_routable() {
    let main = pair(Some(window(Some(100), Some(2_000), Some(18_000))), None);
    let additional = AdditionalRateLimit {
        limit_id: None,
        limit_name: Some("extra".to_string()),
        metered_feature: None,
        rate_limit: pair(Some(window(Some(10), Some(2_000), Some(18_000))), None),
        allowed: None,
        limit_reached: None,
        extra: BTreeMap::new(),
    };
    let usage = usage(Some(main), vec![additional]);

    assert!(!openai_quota_has_ready_limit(&usage));
    assert!(std::ptr::eq(
        openai_quota_runtime_window_pair(&usage).unwrap(),
        usage.rate_limit.as_ref().unwrap()
    ));
}

#[test]
fn additional_capacity_preserves_unicode_label_rendering() {
    let additional = AdditionalRateLimit {
        limit_id: None,
        limit_name: Some("週次予算".to_string()),
        metered_feature: None,
        rate_limit: pair(Some(window(Some(0), Some(2_000), Some(18_000))), None),
        allowed: None,
        limit_reached: None,
        extra: BTreeMap::new(),
    };
    assert!(additional_rate_limit_is_usable(&additional));

    let mut blocked = additional.clone();
    blocked.allowed = Some(false);
    assert!(!additional_rate_limit_is_usable(&blocked));

    let rendered = render_profile_quota_with_width("main", &usage(None, vec![additional]), 100);
    assert!(rendered.contains("週次予算 5h"));
}

#[test]
fn quota_capacity_adapter_batches_past_256_rows() {
    let main = pair(Some(window(Some(10), Some(2_000), Some(18_000))), None);
    let extra = AdditionalRateLimit {
        limit_id: None,
        limit_name: None,
        metered_feature: None,
        rate_limit: main.clone(),
        allowed: None,
        limit_reached: None,
        extra: BTreeMap::new(),
    };
    for (additional_count, expected_count) in [(255, 256), (256, 257), (512, 513)] {
        let usage = usage(
            Some(main.clone()),
            (0..additional_count).map(|_| extra.clone()).collect(),
        );
        let candidates = quota_window_capacity_candidates_at(&usage, 1_000)
            .expect("rows are split into bounded Mojo batches");
        assert_eq!(candidates.len(), expected_count);
        assert!(std::ptr::eq(
            openai_quota_runtime_window_pair(&usage).unwrap(),
            usage.rate_limit.as_ref().unwrap()
        ));
    }
}
