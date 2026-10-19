pub fn scored_runtime_candidate_message(
    initial_profile_name: &str,
    best_candidate: &prodex_shared_types::ReadyProfileCandidate,
    selected_report: Option<&prodex_shared_types::RunProfileProbeReport>,
    include_code_review: bool,
) -> terminal_ui::RuntimeLaunchScoredCandidateOutput {
    let quota_summary = prodex_quota::format_main_windows_compact(&best_candidate.usage);
    let blocked_summary = selected_report.and_then(|report| {
        report.result.as_ref().ok().and_then(|usage| {
            let blocked = prodex_quota::collect_blocked_limits(usage, include_code_review);
            (!blocked.is_empty()).then(|| prodex_quota::format_blocked_limits(&blocked))
        })
    });
    let selected_profile_status = selected_report.map(|report| match &report.result {
        Ok(_) => blocked_summary
            .as_deref()
            .map(
                |blocked_summary| terminal_ui::RuntimeLaunchSelectedProfileStatus::Blocked {
                    blocked_summary,
                },
            )
            .unwrap_or(terminal_ui::RuntimeLaunchSelectedProfileStatus::Ready),
        Err(err) => terminal_ui::RuntimeLaunchSelectedProfileStatus::ProbeFailed { error: err },
    });

    terminal_ui::format_runtime_launch_scored_candidate_message(
        terminal_ui::RuntimeLaunchScoredCandidateMessage {
            initial_profile_name,
            candidate: terminal_ui::RuntimeLaunchCandidateDisplay {
                name: &best_candidate.name,
                quota_summary: &quota_summary,
            },
            selected_profile_status,
        },
    )
}

pub use prodex_mojo_core::info_render::{
    RuntimeLaunchBlockedSelectedProfilePlan, RuntimeLaunchNoReadyProfilesPlan,
};

pub fn no_ready_runtime_profiles_plan(
    report: &prodex_shared_types::RunProfileProbeReport,
    profile_name: &str,
    include_code_review: bool,
) -> RuntimeLaunchNoReadyProfilesPlan {
    use prodex_mojo_core::info_render::InfoRuntimeLaunchSelectedProfileStatus;
    match &report.result {
        Ok(usage) => {
            let blocked = prodex_quota::collect_blocked_limits(usage, include_code_review);
            prodex_mojo_core::info_render::no_ready_runtime_profiles_plan(
                profile_name,
                InfoRuntimeLaunchSelectedProfileStatus::Blocked {
                    blocked_summary: &prodex_quota::format_blocked_limits(&blocked),
                },
            )
        }
        Err(error) => prodex_mojo_core::info_render::no_ready_runtime_profiles_plan(
            profile_name,
            InfoRuntimeLaunchSelectedProfileStatus::ProbeFailed { error },
        ),
    }
    .expect("Mojo no-ready preflight plan failed")
}

pub fn blocked_selected_runtime_profile_plan(
    profile_name: &str,
    blocked: &[prodex_quota::BlockedLimit],
    allow_auto_rotate: bool,
    alternatives: &[String],
) -> RuntimeLaunchBlockedSelectedProfilePlan {
    prodex_mojo_core::info_render::blocked_selected_runtime_profile_plan(
        profile_name,
        &prodex_quota::format_blocked_limits(blocked),
        allow_auto_rotate,
        alternatives,
    )
    .expect("Mojo blocked-profile preflight plan failed")
}
