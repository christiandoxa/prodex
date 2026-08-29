use super::*;
#[cfg(feature = "mojo")]
use prodex_mojo_core::runtime::{
    QuotaRouteScoreInput, QuotaScoreInput, quota_route_score_batch, quota_score_batch,
};
use prodex_quota::AuthSummary;
#[cfg(feature = "mojo")]
use prodex_quota::{scale_quota_pressure_for_plan, usage_plan_capacity_pressure_scale_bps};
use prodex_shared_types::{ReadyProfileCandidate, RunProfileProbeReport, RuntimeQuotaSource};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct SelectionView<'a> {
    entries: &'a [SelectionEntry],
}

struct SelectionEntry {
    name: &'static str,
    provider_priority: usize,
    last_run_selected_at: Option<i64>,
}

impl ProfileSelectionProvider for SelectionEntry {
    fn runtime_pool_priority(&self) -> usize {
        self.provider_priority
    }
}

impl ProfileSelectionRead for SelectionView<'_> {
    type Profile = SelectionEntry;

    fn profile_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.name.to_string())
            .collect()
    }

    fn profile_entry(&self, name: &str) -> Option<&Self::Profile> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    fn last_run_selected_at(&self, name: &str) -> Option<i64> {
        self.profile_entry(name)
            .and_then(|entry| entry.last_run_selected_at)
    }
}

fn selection_usage(now: i64, remaining: i64) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            allowed: None,
            limit_reached: None,
            extra: std::collections::BTreeMap::new(),
            primary_window: Some(UsageWindow {
                used_percent: Some(100 - remaining),
                reset_at: Some(now + 3_600),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(100 - remaining),
                reset_at: Some(now + 86_400),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    }
}

#[test]
fn selection_score_at_is_deterministic() {
    let now = 1_700_000_000;
    let usage = selection_usage(now, 80);

    let first = ready_profile_score_for_route_at(&usage, RuntimeRouteKind::Responses, now);
    let second = ready_profile_score_for_route_at(&usage, RuntimeRouteKind::Responses, now);

    assert_eq!(
        (
            first.total_pressure,
            first.weekly_pressure,
            first.five_hour_pressure,
            first.reserve_floor,
            first.weekly_remaining,
            first.five_hour_remaining,
            first.weekly_reset_at,
            first.five_hour_reset_at,
        ),
        (
            second.total_pressure,
            second.weekly_pressure,
            second.five_hour_pressure,
            second.reserve_floor,
            second.weekly_remaining,
            second.five_hour_remaining,
            second.weekly_reset_at,
            second.five_hour_reset_at,
        )
    );
    assert_eq!(first.five_hour_reset_at, now + 3_600);
    assert_eq!(first.weekly_reset_at, now + 86_400);
}

#[cfg(feature = "mojo")]
#[test]
fn route_score_contract_matches_shared_score_for_normalized_input() {
    let now = 1_700_000_000;
    let mut usage = selection_usage(now, 80);
    usage.plan_type = Some("pro".to_string());
    let weekly = required_main_window_snapshot_at(&usage, "weekly", now).unwrap();
    let five_hour = required_main_window_snapshot_at(&usage, "5h", now).unwrap();
    let scale_bps = usage_plan_capacity_pressure_scale_bps(&usage);
    let route_input = QuotaRouteScoreInput {
        weekly_pressure: weekly.pressure_score,
        five_hour_pressure: five_hour.pressure_score,
        scale_bps,
        weekly_remaining: weekly.remaining_percent,
        five_hour_remaining: five_hour.remaining_percent,
        weekly_has_value: true,
        five_hour_has_value: true,
        weekly_reset_at: weekly.reset_at,
        five_hour_reset_at: five_hour.reset_at,
    };
    let route_score = quota_route_score_batch(&[route_input], 0)
        .expect("route score contract should accept complete windows");
    let normalized_score = quota_score_batch(
        &[QuotaScoreInput {
            weekly_pressure: scale_quota_pressure_for_plan(weekly.pressure_score, scale_bps),
            five_hour_pressure: scale_quota_pressure_for_plan(five_hour.pressure_score, scale_bps),
            weekly_remaining: weekly.remaining_percent,
            five_hour_remaining: five_hour.remaining_percent,
            weekly_has_value: true,
            five_hour_has_value: true,
            weekly_reset_at: weekly.reset_at,
            five_hour_reset_at: five_hour.reset_at,
        }],
        0,
    )
    .expect("shared score contract should accept normalized windows");

    assert_eq!(route_score, normalized_score);
    let resolved = ready_profile_score_for_route_at(&usage, RuntimeRouteKind::Responses, now);
    assert_eq!(resolved.weekly_pressure, route_score[0].weekly_pressure);
    assert_eq!(
        resolved.five_hour_pressure,
        route_score[0].five_hour_pressure
    );
    assert_eq!(resolved.total_pressure, route_score[0].total_pressure);
    assert_eq!(resolved.reserve_floor, route_score[0].reserve_floor);
    assert_eq!(resolved.weekly_remaining, route_score[0].weekly_remaining);
    assert_eq!(
        resolved.five_hour_remaining,
        route_score[0].five_hour_remaining
    );
    assert_eq!(resolved.weekly_reset_at, route_score[0].weekly_reset_at);
    assert_eq!(
        resolved.five_hour_reset_at,
        route_score[0].five_hour_reset_at
    );
}

#[test]
fn route_score_contract_marks_incomplete_window_unknown() {
    let now = 1_700_000_000;
    let mut usage = selection_usage(now, 80);
    usage.rate_limit.as_mut().unwrap().primary_window = None;

    let score = ready_profile_score_for_route_at(&usage, RuntimeRouteKind::Responses, now);
    assert_eq!(score.total_pressure, i64::MAX);
    assert_eq!(score.five_hour_pressure, i64::MAX);

    #[cfg(feature = "mojo")]
    {
        let weekly = required_main_window_snapshot_at(&usage, "weekly", now).unwrap();
        let route_score = quota_route_score_batch(
            &[QuotaRouteScoreInput {
                weekly_pressure: weekly.pressure_score,
                five_hour_pressure: i64::MAX,
                scale_bps: 10_000,
                weekly_remaining: weekly.remaining_percent,
                five_hour_remaining: 0,
                weekly_has_value: true,
                five_hour_has_value: false,
                weekly_reset_at: weekly.reset_at,
                five_hour_reset_at: i64::MAX,
            }],
            0,
        )
        .expect("route score contract should accept an incomplete observation");
        assert_eq!(route_score[0].pressure_band, 4);

        let generic_score = quota_score_batch(
            &[QuotaScoreInput {
                weekly_pressure: weekly.pressure_score,
                five_hour_pressure: i64::MAX,
                weekly_remaining: weekly.remaining_percent,
                five_hour_remaining: 0,
                weekly_has_value: true,
                five_hour_has_value: false,
                weekly_reset_at: weekly.reset_at,
                five_hour_reset_at: i64::MAX,
            }],
            0,
        )
        .expect("generic score contract should accept an incomplete observation");
        assert_eq!(generic_score[0].pressure_band, 0);
    }
}

#[test]
fn scheduler_preserves_cooldown_and_provider_priority() {
    let now = Local::now().timestamp();
    let selection = SelectionView {
        entries: &[
            SelectionEntry {
                name: "recent",
                provider_priority: 0,
                last_run_selected_at: Some(now),
            },
            SelectionEntry {
                name: "ready",
                provider_priority: 0,
                last_run_selected_at: Some(now - RUN_SELECTION_COOLDOWN_SECONDS - 1),
            },
            SelectionEntry {
                name: "lower-provider",
                provider_priority: 1,
                last_run_selected_at: None,
            },
        ],
    };
    let candidate = |name: &str, provider_priority| ReadyProfileCandidate {
        name: name.to_string(),
        usage: selection_usage(now, 80),
        order_index: 0,
        preferred: false,
        provider_priority,
        quota_source: RuntimeQuotaSource::LiveProbe,
    };

    let scheduled = schedule_ready_profile_candidates_with_view(
        vec![
            candidate("recent", 0),
            candidate("lower-provider", 1),
            candidate("ready", 0),
        ],
        selection,
        None,
    );

    assert_eq!(
        scheduled
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        ["ready", "recent", "lower-provider"]
    );
}

#[test]
fn failed_probe_keeps_weekly_only_persisted_snapshot_ready() {
    let now = Local::now().timestamp();
    let selection = SelectionView {
        entries: &[SelectionEntry {
            name: "weekly-only",
            provider_priority: 0,
            last_run_selected_at: None,
        }],
    };
    let reports = [RunProfileProbeReport {
        name: "weekly-only".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Err("probe unavailable".to_string()),
    }];
    let snapshots = BTreeMap::from([(
        "weekly-only".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: now,
            plan_type: None,
            five_hour_status: RuntimeQuotaWindowStatus::Unknown,
            five_hour_remaining_percent: 0,
            five_hour_reset_at: i64::MAX,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 80,
            weekly_reset_at: now + 86_400,
        },
    )]);

    let candidates =
        ready_profile_candidates_with_view(&reports, false, None, selection, Some(&snapshots), 900);

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].name, "weekly-only");
    assert_eq!(
        candidates[0].quota_source,
        RuntimeQuotaSource::PersistedSnapshot
    );
    let windows = candidates[0].usage.rate_limit.as_ref().unwrap();
    assert!(windows.primary_window.is_none());
    assert_eq!(
        windows
            .secondary_window
            .as_ref()
            .and_then(|window| window.used_percent),
        Some(20)
    );
}

#[test]
fn rotation_order_keeps_current_then_provider_aware_rotation() {
    let selection = SelectionView {
        entries: &[
            SelectionEntry {
                name: "current",
                provider_priority: 0,
                last_run_selected_at: None,
            },
            SelectionEntry {
                name: "lower-provider",
                provider_priority: 1,
                last_run_selected_at: None,
            },
            SelectionEntry {
                name: "same-provider",
                provider_priority: 0,
                last_run_selected_at: None,
            },
        ],
    };

    assert_eq!(
        active_profile_selection_order_with_view(selection, "current"),
        ["current", "same-provider", "lower-provider"]
    );
    assert_eq!(
        profile_rotation_order_with_view(selection, "missing"),
        ["current", "same-provider", "lower-provider"]
    );
}
