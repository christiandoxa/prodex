use prodex_mojo_core::info_render::{
    self, InfoRuntimeLaunchSelectedProfileStatus as MojoRuntimeLaunchSelectedProfileStatus,
};

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
    let selected_profile_status = message.selected_profile_status.map(|status| match status {
        RuntimeLaunchSelectedProfileStatus::Ready => MojoRuntimeLaunchSelectedProfileStatus::Ready,
        RuntimeLaunchSelectedProfileStatus::Blocked { blocked_summary } => {
            MojoRuntimeLaunchSelectedProfileStatus::Blocked { blocked_summary }
        }
        RuntimeLaunchSelectedProfileStatus::ProbeFailed { error } => {
            MojoRuntimeLaunchSelectedProfileStatus::ProbeFailed { error }
        }
    });
    let output = info_render::format_runtime_launch_scored_candidate(
        message.initial_profile_name,
        message.candidate.name,
        message.candidate.quota_summary,
        selected_profile_status,
    )
    .expect("Mojo runtime-launch message renderer returned invalid output");
    RuntimeLaunchScoredCandidateOutput {
        warning: output.warning,
        selection: output.selection,
    }
}

pub fn format_runtime_provider_direct_launch_message(provider_id: &str, source: &str) -> String {
    info_render::format_runtime_provider_direct_launch_message(provider_id, source)
        .expect("Mojo runtime-provider direct-launch renderer returned invalid output")
}

pub fn format_runtime_launch_quota_inspect_hint(profile_name: &str) -> String {
    info_render::format_runtime_launch_quota_inspect_hint(profile_name)
        .expect("Mojo runtime quota-inspect hint renderer returned invalid output")
}
