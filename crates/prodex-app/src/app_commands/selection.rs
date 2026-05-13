use anyhow::{Context, Result};
use chrono::Local;
use std::collections::BTreeMap;
use std::thread;

use crate::ProfileProviderExt;
use crate::{
    AppState, MainWindowSnapshot, ProfileEntry, RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    ReadyProfileCandidate, RunProfileProbeJob, RunProfileProbeReport, RuntimeProfileUsageSnapshot,
    UsageResponse, fetch_usage_with_proxy_policy,
};

#[cfg(test)]
pub(crate) use prodex_app_reports::ready_profile_sort_key;
#[cfg(test)]
pub(crate) use prodex_app_reports::schedule_ready_profile_candidates_with_view;
pub(crate) use prodex_app_reports::{
    ProfileSelectionRead, ProfileSelectionView, RuntimeProfileSelectionCatalog,
    RuntimeProfileSelectionCatalogView, RuntimeRouteSelectionCatalog,
    RuntimeRouteSelectionCatalogView, RuntimeRouteSelectionEntry, RuntimeSelectionProfileEntry,
    active_profile_selection_order_with_view, merge_run_preflight_reports_with_current_first,
    profile_rotation_order_with_view, run_profile_probe_is_ready,
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
                .map_err(|err| err.to_string())
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
            .map_err(|err| err.to_string())
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
    prodex_app_reports::ready_profile_candidates_with_view(
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

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn required_main_window_snapshot(
    usage: &UsageResponse,
    label: &str,
) -> Option<MainWindowSnapshot> {
    prodex_app_reports::required_main_window_snapshot_at(usage, label, Local::now().timestamp())
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

    thread::scope(|scope| {
        let func = &func;
        let mut handles = Vec::with_capacity(inputs.len());
        for input in inputs {
            handles.push(scope.spawn(move || func(input)));
        }

        handles
            .into_iter()
            .map(|handle| match handle.join() {
                Ok(output) => output,
                Err(payload) => std::panic::resume_unwind(payload),
            })
            .collect()
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
