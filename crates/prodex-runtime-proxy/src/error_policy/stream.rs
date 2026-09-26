use super::{RuntimeHttpErrorClass, RuntimeHttpErrorPhase, RuntimeHttpErrorPolicy};

pub fn runtime_stream_error_policy(
    body: &[u8],
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    super::runtime_error_policy_from_mojo(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_STREAM,
        0,
        phase,
        body,
    )
}

pub fn runtime_stream_error_policy_from_value(
    value: &serde_json::Value,
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    let Ok(body) = serde_json::to_vec(value) else {
        return RuntimeHttpErrorPolicy::pass_through();
    };
    runtime_stream_error_policy(&body, phase)
}

pub fn runtime_http_error_class_label(class: RuntimeHttpErrorClass) -> &'static str {
    match class {
        RuntimeHttpErrorClass::Quota => "quota",
        RuntimeHttpErrorClass::RateLimited => "rate_limited",
        RuntimeHttpErrorClass::ProfileUnavailable => "profile_unavailable",
        RuntimeHttpErrorClass::Overload => "overload",
        RuntimeHttpErrorClass::TransientServer => "transient_5xx",
        RuntimeHttpErrorClass::Other => "other",
    }
}

pub fn runtime_http_error_action_label(action: super::RuntimeHttpErrorAction) -> &'static str {
    match action {
        super::RuntimeHttpErrorAction::PassThrough => "pass_through",
        super::RuntimeHttpErrorAction::RotateProfile => "rotate_profile",
        super::RuntimeHttpErrorAction::RetryProfile => "retry_profile",
    }
}
