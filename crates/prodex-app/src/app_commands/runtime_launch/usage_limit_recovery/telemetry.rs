use super::GoalResumeRelaunchPlan;

pub(crate) fn runtime_session_recovery_wait_message(
    recovery_round: usize,
    failure_class: &str,
) -> String {
    runtime_proxy_crate::runtime_proxy_structured_log_message(
        "runtime_recovery",
        [
            runtime_proxy_crate::runtime_proxy_log_field("retry_layer", "session"),
            runtime_proxy_crate::runtime_proxy_log_field(
                "recovery_generation",
                recovery_round.to_string(),
            ),
            runtime_proxy_crate::runtime_proxy_log_field("failure_class", failure_class),
            runtime_proxy_crate::runtime_proxy_log_field("recovery_outcome", "waiting"),
        ],
    )
}

pub(crate) fn runtime_session_recovery_message(
    plan: &GoalResumeRelaunchPlan,
    recovery_generation: usize,
    requested_model: Option<&str>,
    effective_model: Option<&str>,
) -> String {
    let requested_model = requested_model
        .filter(|model| model.len() <= 128)
        .unwrap_or("unknown");
    let effective_model = effective_model
        .filter(|model| model.len() <= 128)
        .unwrap_or("unknown");
    runtime_proxy_crate::runtime_proxy_structured_log_message(
        "runtime_recovery",
        [
            runtime_proxy_crate::runtime_proxy_log_field("retry_layer", "session"),
            runtime_proxy_crate::runtime_proxy_log_field(
                "recovery_generation",
                recovery_generation.to_string(),
            ),
            runtime_proxy_crate::runtime_proxy_log_field("failure_class", plan.failure_class),
            runtime_proxy_crate::runtime_proxy_log_field(
                "profile_hash",
                runtime_proxy_crate::runtime_proxy_identifier_hash(Some(&plan.profile_name)),
            ),
            runtime_proxy_crate::runtime_proxy_log_field("requested_model", requested_model),
            runtime_proxy_crate::runtime_proxy_log_field("effective_model", effective_model),
            runtime_proxy_crate::runtime_proxy_log_field(
                "acceptance_state",
                plan.evidence.acceptance_state,
            ),
            runtime_proxy_crate::runtime_proxy_log_field(
                "stream_committed",
                match plan.evidence.stream_committed {
                    Some(true) => "true",
                    Some(false) => "false",
                    None => "unknown",
                },
            ),
            runtime_proxy_crate::runtime_proxy_log_field(
                "side_effect_state",
                plan.evidence.side_effect_state,
            ),
            runtime_proxy_crate::runtime_proxy_log_field("last_prompt_requeued", "false"),
            runtime_proxy_crate::runtime_proxy_log_field("requeue_reason", "none"),
            runtime_proxy_crate::runtime_proxy_log_field("eligible_profiles_remaining", "unknown"),
            runtime_proxy_crate::runtime_proxy_log_field("recovery_outcome", "relaunch_applied"),
        ],
    )
}
