use super::*;

#[test]
fn probe_plan_marks_pressure_skip_for_request_probe_jobs() {
    let now = Local::now().timestamp();
    let state = RuntimeRouteSelectionCatalog {
        current_profile: "main".to_string(),
        include_code_review: false,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        entries: vec![
            selection_entry(
                "main",
                SelectionEntryFixture {
                    cached_probe_entry: Some(RuntimeProfileProbeCacheEntry {
                        checked_at: now,
                        auth: quota_compatible_auth(),
                        result: Ok(test_usage_with_main_windows(80, 300, 80, 86_400)),
                    }),
                    auth_failure_active: true,
                    ..SelectionEntryFixture::default()
                },
            ),
            selection_entry("second", SelectionEntryFixture::default()),
        ],
    };

    let plan = build_runtime_response_probe_plan(
        &state,
        &BTreeSet::new(),
        RuntimeRouteKind::Responses,
        false,
        true,
        1,
        now,
    );

    assert_eq!(
        plan.sync_probe_jobs
            .iter()
            .map(|job| job.name.as_str())
            .collect::<Vec<_>>(),
        vec!["second"]
    );
    assert!(!plan.should_sync_probe_cold_start);
    assert_eq!(plan.sync_probe_skip_jobs_count, Some(1));
    assert_eq!(plan.sync_probe_skip_profiles_count, Some(1));
}

#[test]
fn candidate_plan_reuses_supplied_ready_candidates_without_rebuilding() {
    let state = RuntimeRouteSelectionCatalog {
        current_profile: "main".to_string(),
        include_code_review: false,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        entries: vec![
            selection_entry(
                "main",
                SelectionEntryFixture {
                    cached_probe_entry: Some(RuntimeProfileProbeCacheEntry {
                        checked_at: 0,
                        auth: quota_compatible_auth(),
                        result: Ok(test_usage_with_unbounded_main_windows(90, 95)),
                    }),
                    ..SelectionEntryFixture::default()
                },
            ),
            selection_entry("second", SelectionEntryFixture::default()),
        ],
    };
    // State-derived report planning would find `main`; execution planning must preserve this
    // already-prepared ready list instead.
    let ready_candidates = vec![ReadyProfileCandidate {
        name: "second".to_string(),
        usage: test_usage_with_unbounded_main_windows(74, 82),
        order_index: 41,
        preferred: false,
        provider_priority: 7,
        quota_source: RuntimeQuotaSource::PersistedSnapshot,
    }];

    let plan = build_runtime_response_candidate_execution_plan(
        &state,
        &BTreeSet::new(),
        RuntimeRouteKind::Responses,
        3,
        ready_candidates,
        runtime_response_candidate_execution_options(None, None, |_| 0),
    );

    assert_eq!(
        plan.ready_candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        vec!["second"]
    );
    assert_eq!(plan.ready_candidates[0].order_index, 0);
    assert_eq!(
        plan.ready_candidates[0].quota_source,
        RuntimeQuotaSource::PersistedSnapshot
    );
    assert_eq!(
        plan.ready_candidates[0]
            .quota_summary
            .five_hour
            .remaining_percent,
        74
    );
    assert_eq!(
        plan.fallback_candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        vec!["second"]
    );
}

#[test]
fn candidate_plan_excludes_failed_profiles_from_ready_and_fallback_candidates() {
    let state = RuntimeRouteSelectionCatalog {
        current_profile: "alpha".to_string(),
        include_code_review: false,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        entries: vec![
            selection_entry("alpha", SelectionEntryFixture::default()),
            selection_entry("beta", SelectionEntryFixture::default()),
        ],
    };
    let ready_candidates = vec![
        ReadyProfileCandidate {
            name: "alpha".to_string(),
            usage: test_usage_with_unbounded_main_windows(90, 95),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "beta".to_string(),
            usage: test_usage_with_unbounded_main_windows(88, 94),
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let plan = build_runtime_response_candidate_execution_plan(
        &state,
        &BTreeSet::from(["alpha".to_string()]),
        RuntimeRouteKind::Responses,
        3,
        ready_candidates,
        runtime_response_candidate_execution_options(None, None, |_| 0),
    );

    assert_eq!(
        plan.ready_candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        vec!["beta"]
    );
    assert_eq!(
        plan.fallback_candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        vec!["beta"]
    );
}

#[derive(Default)]
struct SelectionEntryFixture {
    cached_probe_entry: Option<RuntimeProfileProbeCacheEntry>,
    cached_usage_snapshot: Option<RuntimeProfileUsageSnapshot>,
    auth_failure_active: bool,
    in_selection_backoff: bool,
    backoff_sort_key: (usize, i64, i64, i64),
    inflight_count: usize,
    health_sort_key: u32,
}

fn selection_entry(name: &str, fixture: SelectionEntryFixture) -> RuntimeRouteSelectionEntry {
    RuntimeRouteSelectionEntry {
        profile: RuntimeSelectionProfileEntry {
            name: name.to_string(),
            codex_home: PathBuf::from(format!("/tmp/{name}")),
            provider: ProfileProvider::Openai,
            last_run_selected_at: None,
        },
        cached_auth_summary: Some(quota_compatible_auth()),
        cached_probe_entry: fixture.cached_probe_entry,
        cached_usage_snapshot: fixture.cached_usage_snapshot,
        auth_failure_active: fixture.auth_failure_active,
        in_selection_backoff: fixture.in_selection_backoff,
        backoff_sort_key: fixture.backoff_sort_key,
        inflight_count: fixture.inflight_count,
        health_sort_key: fixture.health_sort_key,
    }
}

fn quota_compatible_auth() -> AuthSummary {
    AuthSummary {
        label: "chatgpt".to_string(),
        quota_compatible: true,
    }
}

fn test_usage_with_main_windows(
    five_hour_remaining: i64,
    five_hour_reset_offset_seconds: i64,
    weekly_remaining: i64,
    weekly_reset_offset_seconds: i64,
) -> UsageResponse {
    let now = Local::now().timestamp();
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some((100 - five_hour_remaining).clamp(0, 100)),
                reset_at: Some(now + five_hour_reset_offset_seconds),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some((100 - weekly_remaining).clamp(0, 100)),
                reset_at: Some(now + weekly_reset_offset_seconds),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    }
}

fn test_usage_with_unbounded_main_windows(
    five_hour_remaining: i64,
    weekly_remaining: i64,
) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some((100 - five_hour_remaining).clamp(0, 100)),
                reset_at: Some(i64::MAX),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some((100 - weekly_remaining).clamp(0, 100)),
                reset_at: Some(i64::MAX),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    }
}
