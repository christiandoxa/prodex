use anyhow::{Context, Result};
#[cfg(test)]
use chrono::Local;
use redaction::redaction_redact_secret_like_text;
use std::collections::BTreeMap;
use std::thread;

use crate::ProfileProviderExt;
use crate::{
    AppState, ProfileEntry, RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS, ReadyProfileCandidate,
    RunProfileProbeJob, RunProfileProbeReport, RuntimeProfileUsageSnapshot,
    fetch_usage_with_proxy_policy,
};
#[cfg(test)]
use crate::{MainWindowSnapshot, UsageResponse};

#[cfg(test)]
pub(crate) use prodex_runtime_quota::ready_profile_sort_key;
#[cfg(test)]
pub(crate) use prodex_runtime_quota::schedule_ready_profile_candidates_with_view;
pub(crate) use prodex_runtime_quota::{
    ProfileSelectionRead, ProfileSelectionView, RuntimeProfileSelectionCatalog,
    RuntimeRouteSelectionCatalog, RuntimeRouteSelectionCatalogView, RuntimeRouteSelectionEntry,
    RuntimeSelectionProfileEntry, active_profile_selection_order_with_view,
    merge_run_preflight_reports_with_current_first, profile_rotation_order_with_view,
    run_profile_probe_is_ready,
};

pub(crate) fn app_state_profile_selection_view(
    state: &AppState,
) -> ProfileSelectionView<'_, ProfileEntry> {
    ProfileSelectionView {
        profiles: &state.profiles,
        last_run_selected_at: &state.last_run_selected_at,
    }
}

pub(crate) fn collect_run_profile_reports(
    state: &AppState,
    profile_names: Vec<String>,
    base_url: Option<&str>,
    upstream_no_proxy: bool,
) -> Vec<RunProfileProbeReport> {
    let jobs = profile_names
        .into_iter()
        .enumerate()
        .filter_map(|(order_index, name)| {
            let profile = state.profiles.get(&name)?;
            Some(RunProfileProbeJob {
                name,
                order_index,
                provider: profile.provider.clone(),
                codex_home: profile.codex_home.clone(),
            })
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    map_parallel(jobs, |job| {
        let auth = job.provider.auth_summary(&job.codex_home);
        let result = if auth.quota_compatible {
            fetch_usage_with_proxy_policy(&job.codex_home, base_url.as_deref(), upstream_no_proxy)
                .map_err(|err| selection_probe_error(&err))
        } else {
            Err("auth mode is not quota-compatible".to_string())
        };

        RunProfileProbeReport {
            name: job.name,
            order_index: job.order_index,
            auth,
            result,
        }
    })
}

pub(crate) fn probe_run_profile(
    state: &AppState,
    profile_name: &str,
    order_index: usize,
    base_url: Option<&str>,
    upstream_no_proxy: bool,
) -> Result<RunProfileProbeReport> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let auth = profile.provider.auth_summary(&profile.codex_home);
    let result = if auth.quota_compatible {
        fetch_usage_with_proxy_policy(&profile.codex_home, base_url, upstream_no_proxy)
            .map_err(|err| selection_probe_error(&err))
    } else {
        Err("auth mode is not quota-compatible".to_string())
    };

    Ok(RunProfileProbeReport {
        name: profile_name.to_string(),
        order_index,
        auth,
        result,
    })
}

pub(crate) fn run_preflight_reports_with_current_first(
    state: &AppState,
    current_profile: &str,
    current_report: RunProfileProbeReport,
    base_url: Option<&str>,
    upstream_no_proxy: bool,
) -> Vec<RunProfileProbeReport> {
    merge_run_preflight_reports_with_current_first(
        current_report,
        collect_run_profile_reports(
            state,
            profile_rotation_order(state, current_profile),
            base_url,
            upstream_no_proxy,
        ),
    )
}

fn selection_probe_error(err: &anyhow::Error) -> String {
    redaction_redact_secret_like_text(&err.to_string())
}

pub(crate) fn ready_profile_candidates(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    preferred_profile: Option<&str>,
    state: &AppState,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
) -> Vec<ReadyProfileCandidate> {
    ready_profile_candidates_with_view(
        reports,
        include_code_review,
        preferred_profile,
        app_state_profile_selection_view(state),
        persisted_usage_snapshots,
    )
}

pub(crate) fn ready_profile_candidates_with_view<S: ProfileSelectionRead>(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    preferred_profile: Option<&str>,
    selection: S,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
) -> Vec<ReadyProfileCandidate> {
    prodex_runtime_quota::ready_profile_candidates_with_view(
        reports,
        include_code_review,
        preferred_profile,
        selection,
        persisted_usage_snapshots,
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    )
}

#[cfg(test)]
pub(crate) fn schedule_ready_profile_candidates(
    candidates: Vec<ReadyProfileCandidate>,
    state: &AppState,
    preferred_profile: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    schedule_ready_profile_candidates_with_view(
        candidates,
        app_state_profile_selection_view(state),
        preferred_profile,
    )
}

#[cfg(test)]
pub(crate) fn required_main_window_snapshot(
    usage: &UsageResponse,
    label: &str,
) -> Option<MainWindowSnapshot> {
    prodex_runtime_quota::required_main_window_snapshot_at(usage, label, Local::now().timestamp())
}

pub(crate) fn active_profile_selection_order(
    state: &AppState,
    current_profile: &str,
) -> Vec<String> {
    active_profile_selection_order_with_view(
        app_state_profile_selection_view(state),
        current_profile,
    )
}

pub(crate) fn map_parallel<I, O, F>(inputs: Vec<I>, func: F) -> Vec<O>
where
    I: Send,
    O: Send,
    F: Fn(I) -> O + Sync,
{
    if inputs.len() <= 1 {
        return inputs.into_iter().map(func).collect();
    }

    let input_count = inputs.len();
    let worker_count = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .clamp(2, 8)
        .min(inputs.len());
    let chunk_size = inputs.len().div_ceil(worker_count);
    let mut chunks = (0..worker_count)
        .map(|_| Vec::new())
        .collect::<Vec<Vec<I>>>();
    for (index, input) in inputs.into_iter().enumerate() {
        chunks[index / chunk_size].push(input);
    }
    chunks.retain(|chunk| !chunk.is_empty());

    thread::scope(|scope| {
        let func = &func;
        let handles = chunks
            .into_iter()
            .map(|chunk| scope.spawn(move || chunk.into_iter().map(func).collect::<Vec<_>>()))
            .collect::<Vec<_>>();

        let mut output = Vec::with_capacity(input_count);
        let mut panic = None;
        for handle in handles {
            match handle.join() {
                Ok(mut chunk) => output.append(&mut chunk),
                Err(payload) => {
                    panic.get_or_insert(payload);
                }
            }
        }
        if let Some(payload) = panic {
            std::panic::resume_unwind(payload);
        }
        output
    })
}

pub(crate) fn find_ready_profiles(
    state: &AppState,
    current_profile: &str,
    base_url: Option<&str>,
    include_code_review: bool,
    upstream_no_proxy: bool,
) -> Vec<String> {
    ready_profile_candidates(
        &collect_run_profile_reports(
            state,
            profile_rotation_order(state, current_profile),
            base_url,
            upstream_no_proxy,
        ),
        include_code_review,
        None,
        state,
        None,
    )
    .into_iter()
    .map(|candidate| candidate.name)
    .collect()
}

pub(crate) fn profile_rotation_order(state: &AppState, current_profile: &str) -> Vec<String> {
    profile_rotation_order_with_view(app_state_profile_selection_view(state), current_profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_probe_error_redacts_secret_like_material() {
        let err = anyhow::anyhow!(
            "failed: Authorization: Bearer fixture-token-123 url=https://example.test?api_key=sk-fixture-123"
        );

        let message = selection_probe_error(&err);

        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("fixture-token-123"));
        assert!(!message.contains("sk-fixture-123"));
    }

    #[test]
    fn map_parallel_preserves_order_and_propagates_worker_panics() {
        let output = map_parallel((0..32).collect(), |value| value * 2);
        assert_eq!(output, (0..32).map(|value| value * 2).collect::<Vec<_>>());

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            map_parallel(vec![0, 1, 2, 3], |value| {
                assert_ne!(value, 2);
                value
            });
        }));
        assert!(panic.is_err());
    }

    #[test]
    fn map_parallel_bounds_concurrency_for_large_inputs() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Barrier};

        let input_count = 128;
        let configured_workers = thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4)
            .clamp(2, 8)
            .min(input_count);
        let chunk_size = input_count.div_ceil(configured_workers);
        let actual_workers = input_count.div_ceil(chunk_size);
        let barrier = Arc::new(Barrier::new(actual_workers));
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);

        let output = map_parallel((0..input_count).collect(), |value| {
            if value % chunk_size == 0 {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(now, Ordering::SeqCst);
                barrier.wait();
                active.fetch_sub(1, Ordering::SeqCst);
            }
            value
        });

        assert_eq!(output, (0..input_count).collect::<Vec<_>>());
        assert_eq!(maximum.load(Ordering::SeqCst), actual_workers);
        assert!(maximum.load(Ordering::SeqCst) <= 8);
    }
}
