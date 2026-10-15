use super::super::*;
use super::attempt::{RuntimeProbeExecutionMode, RuntimeProbeRefreshAttempt};
use super::queue::schedule_runtime_probe_refresh;

pub(crate) fn runtime_profiles_needing_startup_probe_refresh(
    state: &AppState,
    current_profile: &str,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    profile_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
) -> Vec<String> {
    prodex_runtime_state::runtime_profiles_needing_startup_probe_refresh_from_snapshots(
        active_profile_selection_order(state, current_profile)
            .into_iter()
            .map(
                |profile_name| prodex_runtime_state::RuntimeStartupProbeRefreshInput {
                    probe_checked_at: profile_probe_cache
                        .get(&profile_name)
                        .map(|entry| entry.checked_at),
                    usage_snapshot: profile_usage_snapshots.get(&profile_name),
                    profile_name,
                },
            ),
        prodex_runtime_state::RuntimeStartupProbeRefreshPlan {
            now,
            probe_fresh_seconds: RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS,
            stale_grace_seconds: RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
            warm_limit: RUNTIME_STARTUP_PROBE_WARM_LIMIT,
        },
        |status| matches!(status, RuntimeQuotaWindowStatus::Exhausted),
    )
}

pub(crate) fn run_runtime_probe_jobs_inline(
    shared: &RuntimeRotationProxyShared,
    jobs: Vec<(String, PathBuf)>,
    context: &str,
) {
    if jobs.is_empty() {
        return;
    }
    let upstream_base_url = match shared.runtime.lock() {
        Ok(runtime) => runtime.upstream_base_url.clone(),
        Err(_) => return,
    };
    let upstream_no_proxy = shared.upstream_no_proxy;
    let probe_reports = map_parallel(jobs, |(profile_name, codex_home)| {
        (
            profile_name,
            RuntimeProbeRefreshAttempt::collect(
                &codex_home,
                upstream_base_url.as_str(),
                upstream_no_proxy,
            ),
        )
    });
    for (profile_name, attempt) in probe_reports {
        // Inline warm-up follows the same cache-generation arbitration as the
        // queued worker after a quota request refreshes an expired credential.
        let home = shared.runtime.lock().ok().and_then(|runtime| {
            runtime
                .state
                .profiles
                .get(&profile_name)
                .map(|p| p.codex_home.clone())
        });
        if let Some(home) = home {
            apply_runtime_profile_usage_auth_revalidation(
                shared,
                &profile_name,
                load_runtime_profile_usage_auth_cache_entry(&home),
            );
        }
        attempt.execute(
            RuntimeProbeExecutionMode::Inline { context },
            shared,
            &profile_name,
            Instant::now(),
        );
    }
}

pub(crate) fn schedule_runtime_startup_probe_warmup(shared: &RuntimeRotationProxyShared) {
    if runtime_proxy_pressure_mode_active(shared) {
        runtime_proxy_log(
            shared,
            "startup_probe_warmup deferred reason=local_pressure",
        );
        return;
    }
    let (state, current_profile, profile_probe_cache, profile_usage_snapshots, profile_health) =
        match shared.runtime.lock() {
            Ok(runtime) => (
                runtime.state.clone(),
                runtime.current_profile.clone(),
                runtime.profile_probe_cache.clone(),
                runtime.profile_usage_snapshots.clone(),
                runtime.profile_health.clone(),
            ),
            Err(_) => return,
        };
    let now = Local::now().timestamp();
    let ordinary_refreshes = runtime_profiles_needing_startup_probe_refresh(
        &state,
        &current_profile,
        &profile_probe_cache,
        &profile_usage_snapshots,
        now,
    );
    // Restarted followers can inherit a credential penalty alongside a fresh
    // quota snapshot. Queue a real authenticated revalidation for those owners
    // before ordinary warm-up jobs; this never makes a profile eligible itself.
    let mut refresh_profiles = active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .filter(|name| runtime_profile_auth_failure_active_from_map(&profile_health, name, now))
        .take(RUNTIME_STARTUP_PROBE_WARM_LIMIT)
        .collect::<Vec<_>>();
    for name in ordinary_refreshes {
        if refresh_profiles.len() >= RUNTIME_STARTUP_PROBE_WARM_LIMIT {
            break;
        }
        if !refresh_profiles.contains(&name) {
            refresh_profiles.push(name);
        }
    }
    if refresh_profiles.is_empty() {
        return;
    }

    let refresh_jobs = refresh_profiles
        .into_iter()
        .filter_map(|profile_name| {
            let profile = state.profiles.get(&profile_name)?;
            profile
                .provider
                .auth_summary(&profile.codex_home)
                .quota_compatible
                .then(|| (profile_name, profile.codex_home.clone()))
        })
        .collect::<Vec<_>>();
    if refresh_jobs.is_empty() {
        return;
    }

    let sync_limit = shared.runtime_config.startup_sync_probe_warm_limit;
    let sync_count = if cfg!(test) {
        refresh_jobs.len()
    } else {
        sync_limit.min(refresh_jobs.len())
    };
    if sync_count > 0 {
        let sync_jobs = refresh_jobs
            .iter()
            .take(sync_count)
            .cloned()
            .collect::<Vec<_>>();
        runtime_proxy_log(
            shared,
            format!(
                "startup_probe_warmup sync={} profiles={}",
                sync_jobs.len(),
                sync_jobs
                    .iter()
                    .map(|(profile_name, _)| profile_name.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );
        run_runtime_probe_jobs_inline(shared, sync_jobs, "startup_probe_warmup");
    }

    if cfg!(test) {
        return;
    }
    let async_jobs = refresh_jobs
        .into_iter()
        .skip(sync_count)
        .collect::<Vec<_>>();
    if async_jobs.is_empty() {
        return;
    }
    runtime_proxy_log(
        shared,
        format!(
            "startup_probe_warmup queued={} profiles={}",
            async_jobs.len(),
            async_jobs
                .iter()
                .map(|(profile_name, _)| profile_name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ),
    );
    for (profile_name, codex_home) in async_jobs {
        schedule_runtime_probe_refresh(shared, &profile_name, &codex_home);
    }
}
