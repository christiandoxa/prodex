use super::*;
use prodex_quota::{UsageWindow, WindowPair};

#[derive(Clone, Copy)]
struct VectorSelectionView<'a> {
    entries: &'a [VectorSelectionEntry],
}

#[derive(Debug)]
struct VectorSelectionEntry {
    name: &'static str,
    provider_priority: usize,
    last_run_selected_at: Option<i64>,
}

impl ProfileSelectionProvider for VectorSelectionEntry {
    fn runtime_pool_priority(&self) -> usize {
        self.provider_priority
    }
}

impl ProfileSelectionRead for VectorSelectionView<'_> {
    type Profile = VectorSelectionEntry;

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

fn test_usage(remaining: i64) -> UsageResponse {
    let now = Local::now().timestamp();
    let window = |remaining_percent| UsageWindow {
        used_percent: Some(100 - remaining_percent),
        reset_at: Some(now + 3_600),
        limit_window_seconds: Some(3_600),
    };
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(window(remaining)),
            secondary_window: Some(window(remaining)),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

#[test]
fn profile_rotation_order_supports_custom_selection_view() {
    let selection = VectorSelectionView {
        entries: &[
            VectorSelectionEntry {
                name: "bravo",
                provider_priority: 0,
                last_run_selected_at: None,
            },
            VectorSelectionEntry {
                name: "alpha",
                provider_priority: 1,
                last_run_selected_at: None,
            },
            VectorSelectionEntry {
                name: "charlie",
                provider_priority: 0,
                last_run_selected_at: None,
            },
        ],
    };

    assert_eq!(
        active_profile_selection_order_with_view(selection, "bravo"),
        vec![
            "bravo".to_string(),
            "charlie".to_string(),
            "alpha".to_string(),
        ]
    );
}

#[test]
fn schedule_ready_profile_candidates_supports_custom_selection_view() {
    let now = Local::now().timestamp();
    let selection = VectorSelectionView {
        entries: &[
            VectorSelectionEntry {
                name: "alpha",
                provider_priority: 0,
                last_run_selected_at: Some(now),
            },
            VectorSelectionEntry {
                name: "bravo",
                provider_priority: 0,
                last_run_selected_at: Some(now - RUN_SELECTION_COOLDOWN_SECONDS - 5),
            },
        ],
    };
    let candidates = vec![
        ReadyProfileCandidate {
            name: "alpha".to_string(),
            usage: test_usage(80),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "bravo".to_string(),
            usage: test_usage(80),
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let scheduled = schedule_ready_profile_candidates_with_view(candidates, selection, None);

    assert_eq!(
        scheduled
            .into_iter()
            .map(|candidate| candidate.name)
            .collect::<Vec<_>>(),
        vec!["bravo".to_string(), "alpha".to_string()]
    );
}
