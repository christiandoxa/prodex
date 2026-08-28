use super::*;
use prodex_quota::AuthSummary;
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
}
