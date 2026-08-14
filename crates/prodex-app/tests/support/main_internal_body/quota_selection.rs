use super::*;

#[test]
fn validates_profile_names() {
    assert!(validate_profile_name("alpha-1").is_ok());
    assert!(validate_profile_name("bad/name").is_err());
    assert!(validate_profile_name("bad space").is_err());
}

#[test]
fn recognizes_known_windows() {
    assert_eq!(window_label(Some(18_000)), "5h");
    assert_eq!(window_label(Some(604_800)), "weekly");
    assert_eq!(window_label(Some(2_592_000)), "monthly");
}

#[test]
fn blocks_when_main_window_is_exhausted() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(100),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };

    let blocked = collect_blocked_limits(&usage, false);
    assert_eq!(blocked.len(), 1);
    assert!(blocked[0].message.starts_with("5h exhausted until "));
}

#[test]
fn allows_available_main_window_when_weekly_window_is_missing() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };

    assert!(collect_blocked_limits(&usage, false).is_empty());
}

#[test]
fn allows_available_weekly_window_when_five_hour_window_is_missing() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: None,
            secondary_window: Some(UsageWindow {
                used_percent: Some(0),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };

    assert!(collect_blocked_limits(&usage, false).is_empty());
}

#[test]
fn compact_window_format_uses_scale_of_100() {
    let window = UsageWindow {
        used_percent: Some(37),
        reset_at: None,
        limit_window_seconds: Some(18_000),
    };

    assert_eq!(format_window_status_compact(&window), "5h 63%");
    assert!(format_window_status(&window).contains("63% left"));
    assert!(format_window_status(&window).contains("37% used"));
}

#[test]
fn main_reset_summary_lists_required_windows() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
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

    let summary = format_main_reset_summary(&usage);
    assert!(summary.starts_with("5h "));
    assert!(summary.contains(" | weekly "));
    assert!(summary.contains(&format_precise_reset_time(Some(1_700_000_000))));
}

#[test]
fn main_reset_summary_marks_missing_required_window() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };

    assert_eq!(
        format_main_reset_summary(&usage),
        format!(
            "5h {} | weekly unavailable",
            format_precise_reset_time(Some(1_700_000_000))
        )
    );
}

#[test]
fn map_parallel_runs_jobs_concurrently_and_preserves_order() {
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(Mutex::new(0usize));
    let started = Instant::now();

    let output = map_parallel(vec![1, 2, 3, 4], {
        let active = Arc::clone(&active);
        let max_active = Arc::clone(&max_active);
        move |value| {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            {
                let mut seen_max = max_active.lock().expect("max_active poisoned");
                *seen_max = (*seen_max).max(current);
            }

            thread::sleep(Duration::from_millis(50));
            active.fetch_sub(1, Ordering::SeqCst);
            value * 10
        }
    });

    assert_eq!(output, vec![10, 20, 30, 40]);
    assert!(
        *max_active.lock().expect("max_active poisoned") >= 2,
        "parallel worker count never exceeded one"
    );
    assert!(
        started.elapsed() < Duration::from_millis(150),
        "parallel execution took too long: {:?}",
        started.elapsed()
    );
}

#[test]
fn ready_profile_ranking_prefers_soon_recovering_weekly_capacity() {
    let candidates = vec![
        ReadyProfileCandidate {
            name: "slow".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "fast".to_string(),
            usage: usage_with_main_windows(80, 18_000, 80, 86_400),
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let mut ranked = candidates.clone();
    ranked.sort_by_key(ready_profile_sort_key);
    assert_eq!(ranked[0].name, "fast");
}

#[test]
fn runtime_probe_cache_freshness_distinguishes_fresh_stale_and_expired() {
    let now = Local::now().timestamp();
    let fresh = RuntimeProfileProbeCacheEntry {
        checked_at: now,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
    };
    let stale = RuntimeProfileProbeCacheEntry {
        checked_at: now - (RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS + 1),
        auth: fresh.auth.clone(),
        result: fresh.result.clone(),
    };
    let expired = RuntimeProfileProbeCacheEntry {
        checked_at: now - (RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS + 1),
        auth: fresh.auth.clone(),
        result: fresh.result.clone(),
    };

    assert_eq!(
        runtime_profile_probe_cache_freshness(&fresh, now),
        RuntimeProbeCacheFreshness::Fresh
    );
    assert_eq!(
        runtime_profile_probe_cache_freshness(&stale, now),
        RuntimeProbeCacheFreshness::StaleUsable
    );
    assert_eq!(
        runtime_profile_probe_cache_freshness(&expired, now),
        RuntimeProbeCacheFreshness::Expired
    );
}
