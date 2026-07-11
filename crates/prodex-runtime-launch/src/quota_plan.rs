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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLaunchNoReadyProfilesPlan {
    Blocked {
        blocked_message: String,
        no_ready_message: String,
        inspect_hint: String,
        error_message: String,
    },
    ProbeFailed {
        warning_message: String,
        continue_message: String,
    },
}

pub fn no_ready_runtime_profiles_plan(
    report: &prodex_shared_types::RunProfileProbeReport,
    profile_name: &str,
    include_code_review: bool,
) -> RuntimeLaunchNoReadyProfilesPlan {
    match &report.result {
        Ok(usage) => {
            let blocked = prodex_quota::collect_blocked_limits(usage, include_code_review);
            RuntimeLaunchNoReadyProfilesPlan::Blocked {
                blocked_message: format!(
                    "Quota preflight blocked profile '{}': {}",
                    profile_name,
                    prodex_quota::format_blocked_limits(&blocked)
                ),
                no_ready_message: "No ready profile was found.".to_string(),
                inspect_hint: terminal_ui::format_runtime_launch_quota_inspect_hint(profile_name),
                error_message: format!(
                    "quota preflight blocked profile '{}' and no ready profile was found",
                    profile_name
                ),
            }
        }
        Err(err) => RuntimeLaunchNoReadyProfilesPlan::ProbeFailed {
            warning_message: format!(
                "Warning: quota preflight failed for '{}': {err:#}",
                profile_name
            ),
            continue_message: "Continuing without quota gate.".to_string(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLaunchBlockedSelectedProfilePlan {
    Rotate {
        blocked_message: String,
        next_profile: String,
        rotate_message: String,
    },
    Stop {
        blocked_message: String,
        messages: Vec<String>,
        error_message: String,
    },
}

pub fn blocked_selected_runtime_profile_plan(
    profile_name: &str,
    blocked: &[prodex_quota::BlockedLimit],
    allow_auto_rotate: bool,
    alternatives: &[String],
) -> RuntimeLaunchBlockedSelectedProfilePlan {
    let blocked_message = format!(
        "Quota preflight blocked profile '{}': {}",
        profile_name,
        prodex_quota::format_blocked_limits(blocked)
    );

    if allow_auto_rotate {
        if let Some(next_profile) = alternatives.first() {
            let next_profile = next_profile.clone();
            return RuntimeLaunchBlockedSelectedProfilePlan::Rotate {
                blocked_message,
                rotate_message: format!("Auto-rotating to profile '{}'.", next_profile),
                next_profile,
            };
        }

        return RuntimeLaunchBlockedSelectedProfilePlan::Stop {
            blocked_message,
            messages: vec![
                "No other ready profile was found.".to_string(),
                terminal_ui::format_runtime_launch_quota_inspect_hint(profile_name),
            ],
            error_message: format!(
                "quota preflight blocked profile '{}' and no other ready profile was found",
                profile_name
            ),
        };
    }

    let mut messages = Vec::new();
    if !alternatives.is_empty() {
        messages.push(format!(
            "Other profiles that look ready: {}",
            alternatives.join(", ")
        ));
        messages.push("Rerun without `--no-auto-rotate` to allow fallback.".to_string());
    }
    messages.push(terminal_ui::format_runtime_launch_quota_inspect_hint(
        profile_name,
    ));

    RuntimeLaunchBlockedSelectedProfilePlan::Stop {
        blocked_message,
        messages,
        error_message: format!(
            "quota preflight blocked profile '{}' with auto-rotate disabled",
            profile_name
        ),
    }
}
