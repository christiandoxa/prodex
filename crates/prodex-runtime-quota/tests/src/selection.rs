use super::*;
use prodex_shared_types::{ReadyProfileCandidate, RuntimeQuotaSource};

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
