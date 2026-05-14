use super::*;
use prodex_quota::AuthSummary;
use prodex_quota::UsageResponse;
use prodex_quota::{UsageWindow, WindowPair};
use prodex_state::ProfileProvider;
use std::path::PathBuf;

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
    let window = |remaining_percent, seconds| UsageWindow {
        used_percent: Some(100 - remaining_percent),
        reset_at: Some(now + 3_600),
        limit_window_seconds: Some(seconds),
    };
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(window(remaining, 18_000)),
            secondary_window: Some(window(remaining, 604_800)),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

fn quota_auth() -> AuthSummary {
    AuthSummary {
        label: "chatgpt".to_string(),
        quota_compatible: true,
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

#[test]
fn explain_ready_profile_selection_reports_selected_and_rejected_reasons() {
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
                last_run_selected_at: None,
            },
            VectorSelectionEntry {
                name: "charlie",
                provider_priority: 1,
                last_run_selected_at: None,
            },
            VectorSelectionEntry {
                name: "delta",
                provider_priority: 0,
                last_run_selected_at: None,
            },
        ],
    };
    let reports = vec![
        RunProfileProbeReport {
            name: "alpha".to_string(),
            order_index: 0,
            auth: quota_auth(),
            result: Ok(test_usage(80)),
        },
        RunProfileProbeReport {
            name: "bravo".to_string(),
            order_index: 1,
            auth: quota_auth(),
            result: Ok(test_usage(80)),
        },
        RunProfileProbeReport {
            name: "charlie".to_string(),
            order_index: 2,
            auth: AuthSummary {
                label: "api-key".to_string(),
                quota_compatible: false,
            },
            result: Ok(test_usage(80)),
        },
    ];

    let explanation = explain_ready_profile_selection_with_view(
        &reports,
        false,
        Some("alpha"),
        Some("bravo"),
        selection,
        None,
        0,
    );

    assert_eq!(explanation.selected_profile.as_deref(), Some("bravo"));
    let bravo = explanation
        .profiles
        .iter()
        .find(|profile| profile.name == "bravo")
        .unwrap();
    assert_eq!(bravo.decision, ProfileSelectionDecision::Selected);
    assert_eq!(bravo.rank, Some(0));
    assert!(
        bravo
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::PreferredProfile)
    );
    assert!(
        bravo
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::QuotaReady)
    );

    let alpha = explanation
        .profiles
        .iter()
        .find(|profile| profile.name == "alpha")
        .unwrap();
    assert_eq!(alpha.decision, ProfileSelectionDecision::Ready);
    assert!(
        alpha
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::ActiveCurrentPreference)
    );
    assert!(
        alpha
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::RecentlySelected)
    );

    let charlie = explanation
        .profiles
        .iter()
        .find(|profile| profile.name == "charlie")
        .unwrap();
    assert_eq!(charlie.decision, ProfileSelectionDecision::Rejected);
    assert!(
        charlie
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::AuthIncompatible)
    );

    let delta = explanation
        .profiles
        .iter()
        .find(|profile| profile.name == "delta")
        .unwrap();
    assert_eq!(delta.decision, ProfileSelectionDecision::MissingReport);
    assert!(
        delta
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::MissingReport)
    );

    let rendered = render_profile_selection_explanation(&explanation);
    assert!(rendered.contains("Selected profile: bravo"));
    assert!(rendered.contains("PreferredProfile"));
}

#[test]
fn explain_ready_profile_selection_reports_blocked_and_probe_failure() {
    let selection = VectorSelectionView {
        entries: &[
            VectorSelectionEntry {
                name: "blocked",
                provider_priority: 0,
                last_run_selected_at: None,
            },
            VectorSelectionEntry {
                name: "failed",
                provider_priority: 0,
                last_run_selected_at: None,
            },
        ],
    };
    let reports = vec![
        RunProfileProbeReport {
            name: "blocked".to_string(),
            order_index: 0,
            auth: quota_auth(),
            result: Ok(test_usage(0)),
        },
        RunProfileProbeReport {
            name: "failed".to_string(),
            order_index: 1,
            auth: quota_auth(),
            result: Err("network unavailable".to_string()),
        },
    ];

    let explanation =
        explain_ready_profile_selection_with_view(&reports, false, None, None, selection, None, 0);

    assert_eq!(explanation.selected_profile, None);
    let blocked = explanation
        .profiles
        .iter()
        .find(|profile| profile.name == "blocked")
        .unwrap();
    assert_eq!(blocked.decision, ProfileSelectionDecision::Rejected);
    assert!(!blocked.blocked_limits.is_empty());
    assert!(
        blocked
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::Blocked)
    );

    let failed = explanation
        .profiles
        .iter()
        .find(|profile| profile.name == "failed")
        .unwrap();
    assert_eq!(failed.probe_error.as_deref(), Some("network unavailable"));
    assert!(
        failed
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::MissingPersistedSnapshot)
    );
}

#[test]
fn explain_runtime_route_catalog_adds_catalog_specific_reasons() {
    let catalog = RuntimeRouteSelectionCatalog {
        current_profile: "alpha".to_string(),
        include_code_review: false,
        upstream_base_url: "https://example.test".to_string(),
        entries: vec![RuntimeRouteSelectionEntry {
            profile: RuntimeSelectionProfileEntry {
                name: "alpha".to_string(),
                codex_home: PathBuf::from("/tmp/prodex-alpha"),
                provider: ProfileProvider::Openai,
                last_run_selected_at: None,
            },
            cached_auth_summary: Some(quota_auth()),
            cached_probe_entry: Some(RuntimeProfileProbeCacheEntry {
                checked_at: Local::now().timestamp(),
                auth: quota_auth(),
                result: Ok(test_usage(80)),
            }),
            cached_usage_snapshot: None,
            auth_failure_active: true,
            in_selection_backoff: true,
            backoff_sort_key: (0, 0, 0, 0),
            inflight_count: 0,
            health_sort_key: 0,
        }],
    };

    let explanation = explain_runtime_route_selection_catalog(&catalog, None, 0);
    let alpha = explanation
        .profiles
        .iter()
        .find(|profile| profile.name == "alpha")
        .unwrap();

    assert_eq!(alpha.decision, ProfileSelectionDecision::Rejected);
    assert!(
        alpha
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::AuthFailureActive)
    );
    assert!(
        alpha
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::SelectionBackoff)
    );
    assert!(
        alpha
            .reasons
            .iter()
            .any(|reason| reason.kind == ProfileSelectionReasonKind::SupportsCodexRuntime)
    );
}
