use super::*;
use crate::command_dispatch::command_exit_error;
use chrono::Local;
use prodex_quota::{BlockedLimit, UsageWindow, WindowPair};
use redaction::redaction_redact_secret_like_text;
use std::collections::BTreeMap;

pub(super) fn select_runtime_launch_profile(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<RuntimeLaunchSelection> {
    let mut selection = RuntimeLaunchSelection::resolve(
        paths,
        state,
        request.profile,
        request.model_provider_override,
        request.profile_v2_name,
        request.external_provider,
        request.external_provider_api_key,
    )?;
    validate_runtime_launch_upstream_base_url(&selection, request)?;
    if selection.non_openai_model_provider.is_some() {
        return Ok(selection);
    }
    if request.skip_quota_check {
        if let Some(profile_name) =
            persist_implicit_runtime_launch_profile_selection(state, &selection)
        {
            print_runtime_launch_auto_selected_profile(&profile_name)?;
        }
        return Ok(selection);
    }

    if request.allow_auto_rotate
        && !selection.explicit_profile_requested
        && state.profiles.len() > 1
    {
        run_auto_runtime_launch_preflight(paths, state, request, &mut selection)?;
    } else {
        run_selected_runtime_launch_preflight(paths, state, request, &mut selection)?;
    }

    if let Some(profile_name) = persist_implicit_runtime_launch_profile_selection(state, &selection)
    {
        print_runtime_launch_auto_selected_profile(&profile_name)?;
    }
    Ok(selection)
}

fn persist_implicit_runtime_launch_profile_selection(
    state: &mut AppState,
    selection: &RuntimeLaunchSelection,
) -> Option<String> {
    if !selection.profileless_local_home
        && !selection.explicit_profile_requested
        && state.active_profile.as_deref() != Some(selection.selected_profile_name.as_str())
    {
        state.active_profile = Some(selection.selected_profile_name.clone());
        return Some(selection.selected_profile_name.clone());
    }
    None
}

fn print_runtime_launch_auto_selected_profile(profile_name: &str) -> Result<()> {
    print_runtime_launch_notice(
        "Profile Selection",
        vec![format!(
            "Auto-selected profile '{profile_name}' because no active profile was available."
        )],
    )
}

fn run_auto_runtime_launch_preflight(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
) -> Result<()> {
    if try_runtime_launch_snapshot_preflight(paths, state, request, selection)? {
        return Ok(());
    }

    let current_report = probe_run_profile(
        state,
        &selection.initial_profile_name,
        0,
        request.base_url,
        request.upstream_no_proxy,
    )?;
    if run_profile_probe_is_ready(&current_report, request.include_code_review) {
        return Ok(());
    }

    let persisted_usage_snapshots =
        load_runtime_usage_snapshots(paths, &state.profiles).unwrap_or_default();
    let reports = run_preflight_reports_with_current_first(
        state,
        &selection.initial_profile_name,
        current_report,
        request.base_url,
        request.upstream_no_proxy,
    );
    let ready_candidates = ready_profile_candidates(
        &reports,
        request.include_code_review,
        Some(&selection.initial_profile_name),
        state,
        Some(&persisted_usage_snapshots),
    );
    let selected_report = reports
        .iter()
        .find(|report| report.name == selection.initial_profile_name);

    if let Some(best_candidate) = ready_candidates.first() {
        if best_candidate.name != selection.initial_profile_name {
            rotate_to_scored_runtime_candidate(
                paths,
                state,
                request,
                selection,
                best_candidate,
                selected_report,
                request.include_code_review,
            )?;
        }
        return Ok(());
    }

    if let Some(report) = selected_report {
        handle_no_ready_runtime_profiles(report, &selection.initial_profile_name, request)?;
    }
    Ok(())
}

fn rotate_to_scored_runtime_candidate(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
    best_candidate: &ReadyProfileCandidate,
    selected_report: Option<&RunProfileProbeReport>,
    include_code_review: bool,
) -> Result<()> {
    let selection_message = scored_runtime_candidate_message(
        &selection.initial_profile_name,
        best_candidate,
        selected_report,
        include_code_review,
    );

    activate_runtime_launch_profile(paths, state, request, selection, &best_candidate.name)?;
    let mut messages = Vec::new();
    if let Some(warning) = selection_message.warning.as_deref() {
        messages.push(warning.to_string());
    }
    messages.push(selection_message.selection);
    print_runtime_launch_notice("Quota Preflight", messages)?;
    Ok(())
}

fn scored_runtime_candidate_message(
    initial_profile_name: &str,
    best_candidate: &ReadyProfileCandidate,
    selected_report: Option<&RunProfileProbeReport>,
    include_code_review: bool,
) -> RuntimeLaunchScoredCandidateOutput {
    prodex_runtime_launch::scored_runtime_candidate_message(
        initial_profile_name,
        best_candidate,
        selected_report,
        include_code_review,
    )
}

pub(super) fn handle_no_ready_runtime_profiles(
    report: &RunProfileProbeReport,
    profile_name: &str,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<()> {
    match prodex_runtime_launch::no_ready_runtime_profiles_plan(
        report,
        profile_name,
        request.include_code_review,
    ) {
        prodex_runtime_launch::RuntimeLaunchNoReadyProfilesPlan::Blocked {
            blocked_message,
            no_ready_message,
            inspect_hint,
            error_message,
        } => {
            print_runtime_launch_notice(
                "Quota Preflight",
                vec![blocked_message, no_ready_message, inspect_hint],
            )?;
            return Err(command_exit_error(2, error_message));
        }
        prodex_runtime_launch::RuntimeLaunchNoReadyProfilesPlan::ProbeFailed {
            warning_message,
            continue_message,
        } => {
            print_runtime_launch_notice(
                "Quota Preflight",
                vec![warning_message, continue_message],
            )?;
        }
    }
    Ok(())
}

fn run_selected_runtime_launch_preflight(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
) -> Result<()> {
    if try_runtime_launch_snapshot_preflight(paths, state, request, selection)? {
        return Ok(());
    }

    match fetch_usage_with_proxy_policy(
        &selection.codex_home,
        request.base_url,
        request.upstream_no_proxy,
    ) {
        Ok(usage) => {
            let blocked = collect_blocked_limits(&usage, request.include_code_review);
            if !blocked.is_empty() {
                handle_blocked_selected_runtime_profile(
                    paths, state, request, selection, &blocked,
                )?;
            }
        }
        Err(err) => {
            print_runtime_launch_notice(
                "Quota Preflight",
                vec![
                    format!(
                        "Warning: quota preflight failed for '{}': {}",
                        selection.initial_profile_name,
                        runtime_launch_preflight_error_message(&err)
                    ),
                    "Continuing without quota gate.".to_string(),
                ],
            )?;
        }
    }

    Ok(())
}

pub(super) fn runtime_launch_preflight_error_message(err: &anyhow::Error) -> String {
    redaction_redact_secret_like_text(&format!("{err:#}"))
}

fn try_runtime_launch_snapshot_preflight(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
) -> Result<bool> {
    let snapshots = load_runtime_usage_snapshots(paths, &state.profiles).unwrap_or_default();
    if request.allow_auto_rotate
        && !selection.explicit_profile_requested
        && state.profiles.len() > 1
        && try_runtime_launch_auto_snapshot_preflight(paths, state, request, selection, &snapshots)?
    {
        return Ok(true);
    }

    let Some(current_snapshot) =
        runtime_launch_usable_usage_snapshot(snapshots.get(&selection.initial_profile_name))
    else {
        return Ok(false);
    };

    let current_usage = runtime_launch_usage_from_snapshot(current_snapshot);
    let current_blocked = collect_blocked_limits(&current_usage, request.include_code_review);
    if current_blocked.is_empty() {
        return Ok(true);
    }

    Ok(false)
}

fn try_runtime_launch_auto_snapshot_preflight(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
    snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
) -> Result<bool> {
    let snapshot_reports = runtime_launch_snapshot_reports(
        state,
        &selection.initial_profile_name,
        snapshots,
        request.include_code_review,
    );
    let ready_candidates = ready_profile_candidates(
        &snapshot_reports,
        request.include_code_review,
        Some(&selection.initial_profile_name),
        state,
        Some(snapshots),
    );
    let Some(best_candidate) = ready_candidates.first() else {
        return Ok(false);
    };
    if best_candidate.name == selection.initial_profile_name {
        return Ok(true);
    }

    let selected_report = snapshot_reports
        .iter()
        .find(|report| report.name == selection.initial_profile_name);
    rotate_to_scored_runtime_candidate(
        paths,
        state,
        request,
        selection,
        best_candidate,
        selected_report,
        request.include_code_review,
    )?;
    Ok(true)
}

fn runtime_launch_usable_usage_snapshot(
    snapshot: Option<&RuntimeProfileUsageSnapshot>,
) -> Option<&RuntimeProfileUsageSnapshot> {
    let snapshot = snapshot?;
    if prodex_runtime_quota::runtime_usage_snapshot_is_usable(
        snapshot,
        Local::now().timestamp(),
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    ) {
        Some(snapshot)
    } else {
        None
    }
}

fn runtime_launch_snapshot_reports(
    state: &AppState,
    current_profile: &str,
    snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    include_code_review: bool,
) -> Vec<RunProfileProbeReport> {
    profile_rotation_order(state, current_profile)
        .into_iter()
        .enumerate()
        .filter_map(|(order_index, name)| {
            let profile = state.profiles.get(&name)?;
            let snapshot = runtime_launch_usable_usage_snapshot(snapshots.get(&name))?;
            let usage = runtime_launch_usage_from_snapshot(snapshot);
            let blocked = collect_blocked_limits(&usage, include_code_review);
            let result = if blocked.is_empty() {
                Ok(usage)
            } else {
                Err(runtime_launch_blocked_snapshot_message(&blocked))
            };
            Some(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth: profile.provider.auth_summary(&profile.codex_home),
                result,
            })
        })
        .collect()
}

pub(super) fn runtime_launch_usage_from_snapshot(
    snapshot: &RuntimeProfileUsageSnapshot,
) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: runtime_launch_window_from_snapshot(
                snapshot.five_hour_status,
                snapshot.five_hour_remaining_percent,
                snapshot.five_hour_reset_at,
                18_000,
            ),
            secondary_window: runtime_launch_window_from_snapshot(
                snapshot.weekly_status,
                snapshot.weekly_remaining_percent,
                snapshot.weekly_reset_at,
                604_800,
            ),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    }
}

fn runtime_launch_window_from_snapshot(
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    limit_window_seconds: i64,
) -> Option<UsageWindow> {
    let now = Local::now().timestamp();
    let effective_status = if reset_at != i64::MAX && reset_at <= now {
        RuntimeQuotaWindowStatus::Ready
    } else {
        status
    };
    let used_percent = match effective_status {
        RuntimeQuotaWindowStatus::Ready
        | RuntimeQuotaWindowStatus::Thin
        | RuntimeQuotaWindowStatus::Critical => Some((100 - remaining_percent).clamp(0, 99)),
        RuntimeQuotaWindowStatus::Exhausted => Some(100),
        RuntimeQuotaWindowStatus::Unknown => return None,
    };
    Some(UsageWindow {
        used_percent,
        reset_at: if reset_at == i64::MAX {
            None
        } else {
            Some(reset_at)
        },
        limit_window_seconds: Some(limit_window_seconds),
    })
}

fn runtime_launch_blocked_snapshot_message(blocked: &[BlockedLimit]) -> String {
    blocked
        .iter()
        .map(|limit| limit.message.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn handle_blocked_selected_runtime_profile(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
    blocked: &[BlockedLimit],
) -> Result<()> {
    let alternatives = find_ready_profiles(
        state,
        &selection.initial_profile_name,
        request.base_url,
        request.include_code_review,
        request.upstream_no_proxy,
    );
    let plan = prodex_runtime_launch::blocked_selected_runtime_profile_plan(
        &selection.initial_profile_name,
        blocked,
        request.allow_auto_rotate,
        &alternatives,
    );

    match plan {
        prodex_runtime_launch::RuntimeLaunchBlockedSelectedProfilePlan::Rotate {
            blocked_message,
            next_profile,
            rotate_message,
        } => {
            activate_runtime_launch_profile(paths, state, request, selection, &next_profile)?;
            print_runtime_launch_notice("Quota Preflight", vec![blocked_message, rotate_message])?;
        }
        prodex_runtime_launch::RuntimeLaunchBlockedSelectedProfilePlan::Stop {
            blocked_message,
            messages,
            error_message,
        } => {
            let mut notice_messages = vec![blocked_message];
            notice_messages.extend(messages);
            print_runtime_launch_notice("Quota Preflight", notice_messages)?;
            return Err(command_exit_error(2, error_message));
        }
    }

    Ok(())
}

fn print_runtime_launch_notice(title: &str, messages: Vec<String>) -> Result<()> {
    print_stderr_panel(title, &messages)?;
    Ok(())
}

fn activate_runtime_launch_profile(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
    profile_name: &str,
) -> Result<()> {
    selection.select_profile(
        state,
        profile_name,
        request.model_provider_override,
        request.profile_v2_name,
    )?;
    state.active_profile = Some(profile_name.to_string());
    state.save(paths)?;
    Ok(())
}

pub(super) fn runtime_launch_profile_home(state: &AppState, profile_name: &str) -> Result<PathBuf> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    if !profile.provider.supports_codex_runtime() {
        bail!(
            "profile '{}' uses {} (route {}). `prodex run` currently supports native OpenAI/Codex profiles only; provider adapters are launched through `prodex s --provider <provider>`.",
            profile_name,
            profile.provider.display_name(),
            profile.provider.capabilities().runtime_route_policy.label()
        );
    }
    Ok(profile.codex_home.clone())
}
