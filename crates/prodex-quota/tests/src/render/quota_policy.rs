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

#[test]
fn quota_window_pair_readiness_matches_rust_oracle() {
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
        let rust = [first, second].into_iter().flatten().next().is_some()
            && [first, second]
                .into_iter()
                .flatten()
                .all(|used_percent| used_percent < 100);
        assert_eq!(window_pair_has_ready_limit(&pair), rust);
    }
}
