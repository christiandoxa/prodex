use super::*;

pub(super) fn select_runtime_launch_profile(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<RuntimeLaunchSelection> {
    let mut selection = RuntimeLaunchSelection::resolve(state, request.profile)?;
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

pub(super) fn run_auto_runtime_launch_preflight(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
) -> Result<()> {
    let current_report =
        probe_run_profile(state, &selection.initial_profile_name, 0, request.base_url)?;
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
                selection,
                best_candidate,
                selected_report,
                request.include_code_review,
            )?;
        }
        return Ok(());
    }

    if let Some(report) = selected_report {
        handle_no_ready_runtime_profiles(report, &selection.initial_profile_name, request);
    }
    Ok(())
}

pub(super) fn rotate_to_scored_runtime_candidate(
    paths: &AppPaths,
    state: &mut AppState,
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

    activate_runtime_launch_profile(paths, state, selection, &best_candidate.name)?;
    print_wrapped_stderr(&selection_message);
    Ok(())
}

pub(super) fn scored_runtime_candidate_message(
    initial_profile_name: &str,
    best_candidate: &ReadyProfileCandidate,
    selected_report: Option<&RunProfileProbeReport>,
    include_code_review: bool,
) -> String {
    let mut selection_message = format!(
        "Using profile '{}' ({})",
        best_candidate.name,
        format_main_windows_compact(&best_candidate.usage)
    );

    if let Some(report) = selected_report {
        match &report.result {
            Ok(usage) => {
                let blocked = collect_blocked_limits(usage, include_code_review);
                if !blocked.is_empty() {
                    print_wrapped_stderr(&format!(
                        "Quota preflight blocked profile '{}': {}",
                        initial_profile_name,
                        format_blocked_limits(&blocked)
                    ));
                    selection_message = format!(
                        "Auto-rotating to profile '{}' using quota-pressure scoring ({}).",
                        best_candidate.name,
                        format_main_windows_compact(&best_candidate.usage)
                    );
                } else {
                    selection_message = format!(
                        "Auto-selecting profile '{}' over active profile '{}' using quota-pressure scoring ({}).",
                        best_candidate.name,
                        initial_profile_name,
                        format_main_windows_compact(&best_candidate.usage)
                    );
                }
            }
            Err(err) => {
                print_wrapped_stderr(&format!(
                    "Warning: quota preflight failed for '{}': {err}",
                    initial_profile_name
                ));
                selection_message = format!(
                    "Using ready profile '{}' after quota preflight failed ({})",
                    best_candidate.name,
                    format_main_windows_compact(&best_candidate.usage)
                );
            }
        }
    }

    selection_message
}

pub(super) fn handle_no_ready_runtime_profiles(
    report: &RunProfileProbeReport,
    profile_name: &str,
    request: &RuntimeLaunchRequest<'_>,
) {
    match &report.result {
        Ok(usage) => {
            let blocked = collect_blocked_limits(usage, request.include_code_review);
            print_wrapped_stderr(&section_header("Quota Preflight"));
            print_wrapped_stderr(&format!(
                "Quota preflight blocked profile '{}': {}",
                profile_name,
                format_blocked_limits(&blocked)
            ));
            print_wrapped_stderr("No ready profile was found.");
            print_quota_preflight_inspect_hint(profile_name);
            std::process::exit(2);
        }
        Err(err) => {
            print_wrapped_stderr(&section_header("Quota Preflight"));
            print_wrapped_stderr(&format!(
                "Warning: quota preflight failed for '{}': {err:#}",
                profile_name
            ));
            print_wrapped_stderr("Continuing without quota gate.");
        }
    }
}

pub(super) fn run_selected_runtime_launch_preflight(
    paths: &AppPaths,
    state: &mut AppState,
    request: &RuntimeLaunchRequest<'_>,
    selection: &mut RuntimeLaunchSelection,
) -> Result<()> {
    match fetch_usage(&selection.codex_home, request.base_url) {
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

pub(super) fn handle_blocked_selected_runtime_profile(
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
    );

    print_wrapped_stderr(&section_header("Quota Preflight"));
    print_wrapped_stderr(&format!(
        "Quota preflight blocked profile '{}': {}",
        selection.initial_profile_name,
        format_blocked_limits(blocked)
    ));

    if request.allow_auto_rotate {
        if let Some(next_profile) = alternatives.first() {
            let next_profile = next_profile.clone();
            activate_runtime_launch_profile(paths, state, selection, &next_profile)?;
            print_wrapped_stderr(&format!("Auto-rotating to profile '{}'.", next_profile));
        } else {
            print_wrapped_stderr("No other ready profile was found.");
            print_quota_preflight_inspect_hint(&selection.initial_profile_name);
            std::process::exit(2);
        }
    } else {
        if !alternatives.is_empty() {
            print_wrapped_stderr(&format!(
                "Other profiles that look ready: {}",
                alternatives.join(", ")
            ));
            print_wrapped_stderr("Rerun without `--no-auto-rotate` to allow fallback.");
        }
        print_quota_preflight_inspect_hint(&selection.initial_profile_name);
        std::process::exit(2);
    }

    Ok(())
}

pub(super) fn activate_runtime_launch_profile(
    paths: &AppPaths,
    state: &mut AppState,
    selection: &mut RuntimeLaunchSelection,
    profile_name: &str,
) -> Result<()> {
    selection.select_profile(state, profile_name)?;
    state.active_profile = Some(profile_name.to_string());
    state.save(paths)?;
    Ok(())
}

pub(super) fn print_quota_preflight_inspect_hint(profile_name: &str) {
    print_wrapped_stderr(&format!(
        "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
        profile_name
    ));
}

pub(super) fn runtime_launch_profile_home(state: &AppState, profile_name: &str) -> Result<PathBuf> {
    Ok(state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone())
}
