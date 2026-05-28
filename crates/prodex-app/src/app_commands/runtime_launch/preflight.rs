use super::*;
use crate::command_dispatch::command_exit_error;

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
    if selection.non_openai_model_provider.is_some() {
        return Ok(selection);
    }
    if request.skip_quota_check {
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

    Ok(selection)
}

fn run_auto_runtime_launch_preflight(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
) -> Result<()> {
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
    print_wrapped_stderr(&section_header("Quota Preflight"));
    let selection_message = scored_runtime_candidate_message(
        &selection.initial_profile_name,
        best_candidate,
        selected_report,
        include_code_review,
    );

    activate_runtime_launch_profile(paths, state, request, selection, &best_candidate.name)?;
    if let Some(warning) = selection_message.warning.as_deref() {
        print_wrapped_stderr(warning);
    }
    print_wrapped_stderr(&selection_message.selection);
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
            print_wrapped_stderr(&section_header("Quota Preflight"));
            print_wrapped_stderr(&blocked_message);
            print_wrapped_stderr(&no_ready_message);
            print_wrapped_stderr(&inspect_hint);
            return Err(command_exit_error(2, error_message));
        }
        prodex_runtime_launch::RuntimeLaunchNoReadyProfilesPlan::ProbeFailed {
            warning_message,
            continue_message,
        } => {
            print_wrapped_stderr(&section_header("Quota Preflight"));
            print_wrapped_stderr(&warning_message);
            print_wrapped_stderr(&continue_message);
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
            print_wrapped_stderr(&section_header("Quota Preflight"));
            print_wrapped_stderr(&format!(
                "Warning: quota preflight failed for '{}': {err:#}",
                selection.initial_profile_name
            ));
            print_wrapped_stderr("Continuing without quota gate.");
        }
    }

    Ok(())
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

    print_wrapped_stderr(&section_header("Quota Preflight"));
    match plan {
        prodex_runtime_launch::RuntimeLaunchBlockedSelectedProfilePlan::Rotate {
            blocked_message,
            next_profile,
            rotate_message,
        } => {
            print_wrapped_stderr(&blocked_message);
            activate_runtime_launch_profile(paths, state, request, selection, &next_profile)?;
            print_wrapped_stderr(&rotate_message);
        }
        prodex_runtime_launch::RuntimeLaunchBlockedSelectedProfilePlan::Stop {
            blocked_message,
            messages,
            error_message,
        } => {
            print_wrapped_stderr(&blocked_message);
            for message in messages {
                print_wrapped_stderr(&message);
            }
            return Err(command_exit_error(2, error_message));
        }
    }

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
            "profile '{}' uses {}. `prodex run` currently supports OpenAI/Codex profiles only.",
            profile_name,
            profile.provider.display_name()
        );
    }
    Ok(profile.codex_home.clone())
}
