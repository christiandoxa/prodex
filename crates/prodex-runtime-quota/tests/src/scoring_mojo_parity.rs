use super::*;
use prodex_quota::UsageWindow;

const NOW: i64 = 1_700_000_000;
const REMAINING_BOUNDARIES: [i64; 16] = [0, 1, 3, 4, 5, 6, 9, 10, 11, 19, 20, 21, 50, 90, 99, 100];

#[derive(Clone)]
struct SelectionEntry {
    name: String,
    provider_priority: usize,
    last_selected_at: Option<i64>,
}

impl ProfileSelectionProvider for SelectionEntry {
    fn runtime_pool_priority(&self) -> usize {
        self.provider_priority
    }
}

#[derive(Clone, Copy)]
struct SelectionView<'a> {
    entries: &'a [SelectionEntry],
}

impl ProfileSelectionRead for SelectionView<'_> {
    type Profile = SelectionEntry;

    fn profile_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.name.clone())
            .collect()
    }

    fn profile_entry(&self, name: &str) -> Option<&Self::Profile> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    fn last_run_selected_at(&self, name: &str) -> Option<i64> {
        self.profile_entry(name)
            .and_then(|entry| entry.last_selected_at)
    }
}

fn next(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    *seed
}

fn usage(
    weekly_remaining: i64,
    five_hour_remaining: i64,
    plan_type: Option<&str>,
    missing_window: u64,
) -> UsageResponse {
    let window = |remaining, reset_at, limit_window_seconds| UsageWindow {
        used_percent: Some(100 - remaining),
        reset_at: Some(reset_at),
        limit_window_seconds: Some(limit_window_seconds),
    };
    UsageResponse {
        email: None,
        plan_type: plan_type.map(str::to_string),
        rate_limit: Some(prodex_quota::WindowPair {
            primary_window: (missing_window & 1 == 0)
                .then(|| window(five_hour_remaining, NOW + 3_600, 18_000)),
            secondary_window: (missing_window & 2 == 0)
                .then(|| window(weekly_remaining, NOW + 86_400, 604_800)),
            ..Default::default()
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    }
}

fn ordered_output(
    candidates: &[ReadyProfileCandidate],
) -> Vec<(
    String,
    serde_json::Value,
    usize,
    bool,
    usize,
    RuntimeQuotaSource,
)> {
    candidates
        .iter()
        .map(|candidate| {
            (
                candidate.name.clone(),
                serde_json::to_value(&candidate.usage).unwrap(),
                candidate.order_index,
                candidate.preferred,
                candidate.provider_priority,
                candidate.quota_source,
            )
        })
        .collect()
}

#[test]
fn schedule_matches_pre_migration_rust_oracle_over_seeded_candidates() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    let mut seed = 0x510e_527f_ade6_82d1_u64;
    for case_index in 0..1_024 {
        let count = 2 + (next(&mut seed) % 9) as usize;
        let mut entries = Vec::with_capacity(count);
        let mut candidates = Vec::with_capacity(count);
        for order_index in 0..count {
            let name = format!("profile-{case_index}-{order_index}");
            let provider_priority = (next(&mut seed) % 3) as usize;
            let selected = match next(&mut seed) % 6 {
                0 => None,
                1 => Some(NOW.saturating_sub(RUN_SELECTION_COOLDOWN_SECONDS - 1)),
                2 => Some(NOW.saturating_sub(RUN_SELECTION_COOLDOWN_SECONDS)),
                3 => Some(NOW.saturating_add(1)),
                4 => Some(i64::MIN),
                _ => Some(i64::MAX),
            };
            entries.push(SelectionEntry {
                name: name.clone(),
                provider_priority,
                last_selected_at: selected,
            });
            let weekly_remaining =
                REMAINING_BOUNDARIES[(next(&mut seed) as usize) % REMAINING_BOUNDARIES.len()];
            let five_hour_remaining =
                REMAINING_BOUNDARIES[(next(&mut seed) as usize) % REMAINING_BOUNDARIES.len()];
            let plan_type = match next(&mut seed) % 5 {
                0 => None,
                1 => Some("free"),
                2 => Some("plus"),
                3 => Some("pro"),
                _ => Some("unknown-plan"),
            };
            candidates.push(ReadyProfileCandidate {
                name,
                usage: usage(
                    weekly_remaining,
                    five_hour_remaining,
                    plan_type,
                    u64::from(next(&mut seed).is_multiple_of(13)),
                ),
                order_index,
                preferred: next(&mut seed).is_multiple_of(4),
                provider_priority,
                quota_source: if next(&mut seed).is_multiple_of(2) {
                    RuntimeQuotaSource::LiveProbe
                } else {
                    RuntimeQuotaSource::PersistedSnapshot
                },
            });
        }
        let selection = SelectionView { entries: &entries };
        let preferred_index = (next(&mut seed) % (count + 1) as u64) as usize;
        let preferred = (preferred_index < count).then(|| candidates[preferred_index].name.clone());
        let expected = schedule_ready_profile_candidates_rust(
            candidates.clone(),
            selection,
            preferred.as_deref(),
            None,
            NOW,
        );
        let actual = schedule_ready_profile_candidates_with_view_for_model_at(
            candidates,
            selection,
            preferred.as_deref(),
            None,
            NOW,
        );
        assert_eq!(
            ordered_output(&actual),
            ordered_output(&expected),
            "seeded scheduler mismatch: seed=0x510e527fade682d1 case={case_index}"
        );
    }
}

#[test]
fn quota_score_and_band_match_pre_migration_rust_oracles() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    let mut seed = 0x9b05_688c_2b3e_6c1f_u64;
    for case_index in 0..2_048 {
        let weekly_remaining =
            REMAINING_BOUNDARIES[(next(&mut seed) as usize) % REMAINING_BOUNDARIES.len()];
        let five_hour_remaining =
            REMAINING_BOUNDARIES[(next(&mut seed) as usize) % REMAINING_BOUNDARIES.len()];
        let missing_window = if case_index % 17 == 0 {
            1 + (next(&mut seed) % 3)
        } else {
            0
        };
        let plan_type = match next(&mut seed) % 5 {
            0 => None,
            1 => Some("free"),
            2 => Some("plus"),
            3 => Some("pro"),
            _ => Some("unknown-plan"),
        };
        let usage = usage(
            weekly_remaining,
            five_hour_remaining,
            plan_type,
            missing_window,
        );
        for route_kind in [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Compact,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Standard,
        ] {
            let rust_score = ready_profile_score_for_route_at_rust(&usage, route_kind, NOW);
            let mojo_score = ready_profile_score_for_route_at(&usage, route_kind, NOW);
            assert_eq!(
                (
                    mojo_score.total_pressure,
                    mojo_score.weekly_pressure,
                    mojo_score.five_hour_pressure,
                    mojo_score.reserve_floor,
                    mojo_score.weekly_remaining,
                    mojo_score.five_hour_remaining,
                    mojo_score.weekly_reset_at,
                    mojo_score.five_hour_reset_at,
                ),
                (
                    rust_score.total_pressure,
                    rust_score.weekly_pressure,
                    rust_score.five_hour_pressure,
                    rust_score.reserve_floor,
                    rust_score.weekly_remaining,
                    rust_score.five_hour_remaining,
                    rust_score.weekly_reset_at,
                    rust_score.five_hour_reset_at,
                ),
                "score mismatch: seed=0x9b05688c2b3e6c1f case={case_index} route={route_kind:?}"
            );
            assert_eq!(
                runtime_quota_pressure_band_for_route_at(&usage, route_kind, NOW),
                runtime_quota_pressure_band_for_route_at_rust(&usage, route_kind, NOW),
                "band mismatch: seed=0x9b05688c2b3e6c1f case={case_index} route={route_kind:?}"
            );
        }
    }
}
