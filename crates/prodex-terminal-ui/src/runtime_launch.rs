#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLaunchCandidateDisplay<'a> {
    pub name: &'a str,
    pub quota_summary: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLaunchSelectedProfileStatus<'a> {
    Ready,
    Blocked { blocked_summary: &'a str },
    ProbeFailed { error: &'a str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLaunchScoredCandidateMessage<'a> {
    pub initial_profile_name: &'a str,
    pub candidate: RuntimeLaunchCandidateDisplay<'a>,
    pub selected_profile_status: Option<RuntimeLaunchSelectedProfileStatus<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLaunchScoredCandidateOutput {
    pub warning: Option<String>,
    pub selection: String,
}

pub fn format_runtime_launch_scored_candidate_message(
    message: RuntimeLaunchScoredCandidateMessage<'_>,
) -> RuntimeLaunchScoredCandidateOutput {
    let RuntimeLaunchCandidateDisplay {
        name,
        quota_summary,
    } = message.candidate;
    let mut output = RuntimeLaunchScoredCandidateOutput {
        warning: None,
        selection: format!("Using profile '{name}' ({quota_summary})"),
    };

    match message.selected_profile_status {
        Some(RuntimeLaunchSelectedProfileStatus::Blocked { blocked_summary }) => {
            output.warning = Some(format!(
                "Quota preflight blocked profile '{}': {}",
                message.initial_profile_name, blocked_summary
            ));
            output.selection = format!(
                "Auto-rotating to profile '{name}' using quota-pressure scoring ({quota_summary})."
            );
        }
        Some(RuntimeLaunchSelectedProfileStatus::Ready) => {
            output.selection = format!(
                "Auto-selecting profile '{name}' over active profile '{}' using quota-pressure scoring ({quota_summary}).",
                message.initial_profile_name
            );
        }
        Some(RuntimeLaunchSelectedProfileStatus::ProbeFailed { error }) => {
            output.warning = Some(format!(
                "Warning: quota preflight failed for '{}': {error}",
                message.initial_profile_name
            ));
            output.selection = format!(
                "Using ready profile '{name}' after quota preflight failed ({quota_summary})"
            );
        }
        None => {}
    }

    output
}

pub fn format_runtime_provider_direct_launch_message(provider_id: &str, source: &str) -> String {
    format!(
        "Detected model_provider '{provider_id}' from {source}. Launching directly without prodex quota preflight or auto-rotate proxy."
    )
}

pub fn format_runtime_launch_quota_inspect_hint(profile_name: &str) -> String {
    format!(
        "Inspect with `prodex quota --profile {profile_name}` or bypass with `prodex run --skip-quota-check`."
    )
}
