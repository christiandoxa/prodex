use super::{GoalUsageLimitMonitor, RuntimeWorkflowEvidence};
use crate::AppState;
use crate::app_state::{AppStateIoExt, ProfileProviderExt};
use anyhow::Result;
use prodex_mojo_core::launch::{
    UsageLimitRecoveryCandidateInput, UsageLimitRecoverySelection, usage_limit_recovery_candidate,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalResumeRelaunchPlan {
    pub(crate) session_id: String,
    pub(crate) failed_profile_name: String,
    pub(crate) profile_name: String,
    pub(crate) failure_class: &'static str,
    pub(crate) resume_goal: bool,
    pub(crate) evidence: RuntimeWorkflowEvidence,
}

#[derive(Clone, Copy)]
pub(crate) struct RuntimeUsageLimitResumeOptions<'a> {
    pub(crate) requested_profile: Option<&'a str>,
    pub(crate) no_auto_rotate: bool,
    pub(crate) skip_quota_check: bool,
    pub(crate) base_url: Option<&'a str>,
    pub(crate) include_code_review: bool,
    pub(crate) no_proxy: bool,
    pub(crate) requested_model: Option<&'a str>,
    pub(crate) failure_class: &'static str,
    pub(crate) allow_failed_profile: bool,
    pub(crate) resume_goal: bool,
    pub(crate) attempted_profiles: &'a BTreeSet<String>,
}

pub(crate) fn next_runtime_usage_limit_plan(
    state: &AppState,
    session_id: &str,
    options: &RuntimeUsageLimitResumeOptions<'_>,
) -> Option<GoalResumeRelaunchPlan> {
    let failed_profile = state
        .session_profile_bindings
        .get(session_id)
        .map(|binding| binding.profile_name.clone())
        .or_else(|| options.requested_profile.map(ToOwned::to_owned))
        .or_else(|| state.active_profile.clone())
        .unwrap_or_default();
    let candidates = if options.skip_quota_check {
        super::super::profile_rotation_order(state, &failed_profile)
    } else {
        super::super::find_ready_profiles_for_model(
            state,
            &failed_profile,
            options.base_url,
            options.include_code_review,
            options.no_proxy,
            options.requested_model,
        )
    };
    let eligible = |candidate: &str| {
        state.profiles.get(candidate).is_some_and(|profile| {
            profile.provider.supports_codex_runtime()
                && profile
                    .provider
                    .auth_summary(&profile.codex_home)
                    .quota_compatible
        })
    };
    let candidate_eligible = candidates
        .iter()
        .map(|candidate| eligible(candidate))
        .collect::<Vec<_>>();
    let candidate_attempted = candidates
        .iter()
        .map(|candidate| options.attempted_profiles.contains(candidate))
        .collect::<Vec<_>>();
    let failed_profile_eligible = eligible(&failed_profile);
    let failed_profile_attempted = options.attempted_profiles.contains(&failed_profile);
    let failed_profile_ready = options.skip_quota_check
        || (!failed_profile_attempted
            && failed_profile_eligible
            && !super::super::find_ready_current_profile_for_model(
                state,
                &failed_profile,
                options.base_url,
                options.include_code_review,
                options.no_proxy,
                options.requested_model,
            )
            .is_empty());
    let selection = usage_limit_recovery_candidate(UsageLimitRecoveryCandidateInput {
        allow_failed_profile: options.allow_failed_profile,
        skip_quota_check: options.skip_quota_check,
        failed_profile_eligible,
        failed_profile_attempted,
        failed_profile_ready,
        candidate_eligible: &candidate_eligible,
        candidate_attempted: &candidate_attempted,
    })
    .expect("Mojo usage-limit recovery candidate policy failed");
    let profile_name = match selection {
        UsageLimitRecoverySelection::None => return None,
        UsageLimitRecoverySelection::Candidate(index) => candidates.get(index)?.clone(),
        UsageLimitRecoverySelection::FailedProfile => failed_profile.clone(),
    };
    Some(GoalResumeRelaunchPlan {
        session_id: session_id.to_string(),
        failed_profile_name: failed_profile,
        profile_name,
        failure_class: options.failure_class,
        resume_goal: options.resume_goal,
        evidence: RuntimeWorkflowEvidence::default(),
    })
}

pub(crate) fn plan_runtime_usage_limit_relaunch(
    monitor: &mut GoalUsageLimitMonitor,
    status: &std::process::ExitStatus,
    options: &RuntimeUsageLimitResumeOptions<'_>,
) -> Result<Option<GoalResumeRelaunchPlan>> {
    if status.success() || options.no_auto_rotate || runtime_exit_status_is_cancelled(status) {
        return Ok(None);
    }
    let Some(session_id) = monitor.detect_usage_limit_after_child()? else {
        return Ok(None);
    };
    next_observed_runtime_recovery_plan_for_session(monitor, &session_id, options)
}

pub(crate) fn next_observed_runtime_recovery_plan(
    monitor: &GoalUsageLimitMonitor,
    options: &RuntimeUsageLimitResumeOptions<'_>,
) -> Result<Option<GoalResumeRelaunchPlan>> {
    let Some(session_id) = monitor.session_id.as_deref() else {
        return Ok(None);
    };
    next_observed_runtime_recovery_plan_for_session(monitor, session_id, options)
}

fn next_observed_runtime_recovery_plan_for_session(
    monitor: &GoalUsageLimitMonitor,
    session_id: &str,
    options: &RuntimeUsageLimitResumeOptions<'_>,
) -> Result<Option<GoalResumeRelaunchPlan>> {
    let state = AppState::load_and_repair(&monitor.paths)?;
    let observed_model = monitor.recovery_model().map(ToOwned::to_owned);
    let mut options = *options;
    options.requested_model = observed_model.as_deref().or(options.requested_model);
    options.failure_class = monitor.recovery_failure_class();
    options.resume_goal = monitor.has_session_goal();
    let evidence = monitor.recovery_evidence();
    Ok(
        next_runtime_usage_limit_plan(&state, session_id, &options).map(|mut plan| {
            plan.evidence = evidence;
            plan
        }),
    )
}

pub(crate) fn runtime_exit_status_is_cancelled(status: &std::process::ExitStatus) -> bool {
    let exit_code = status.code().map(i64::from).unwrap_or(-1);
    #[cfg(unix)]
    let (signal, interrupt_signal) = {
        use std::os::unix::process::ExitStatusExt as _;
        (
            status.signal().map(i64::from).unwrap_or(-1),
            i64::from(libc::SIGINT),
        )
    };
    #[cfg(not(unix))]
    let (signal, interrupt_signal) = (-1, -1);
    #[cfg(windows)]
    let control_c_exit_code = i64::from(windows_sys::Win32::Foundation::CONTROL_C_EXIT);
    #[cfg(not(windows))]
    let control_c_exit_code = -1;
    prodex_mojo_core::launch::runtime_exit_status_cancelled(
        exit_code,
        signal,
        interrupt_signal,
        control_c_exit_code,
    )
    .expect("Mojo runtime launch exit policy failed")
}
