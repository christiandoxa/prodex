#[cfg(feature = "mojo")]
use super::runtime_error_policy_from_mojo;
#[cfg(not(feature = "mojo"))]
use super::{
    RUNTIME_STREAM_ERROR_RULES, RuntimeSignalMatchMode, runtime_error_policy_match,
    runtime_error_signal_message_from_value_mode,
};
use super::{RuntimeHttpErrorClass, RuntimeHttpErrorPhase, RuntimeHttpErrorPolicy};

pub fn runtime_stream_error_policy(
    body: &[u8],
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    #[cfg(feature = "mojo")]
    {
        runtime_error_policy_from_mojo(
            prodex_mojo_core::rich::RUNTIME_ERROR_MODE_STREAM,
            0,
            phase,
            body,
        )
    }
    #[cfg(not(feature = "mojo"))]
    {
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
            return runtime_stream_error_policy_from_value(&value, phase);
        }
        RuntimeHttpErrorPolicy::pass_through()
    }
}

pub fn runtime_stream_error_policy_from_value(
    value: &serde_json::Value,
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    #[cfg(feature = "mojo")]
    {
        let body = serde_json::to_vec(value).expect("runtime error payload serializes");
        runtime_stream_error_policy(&body, phase)
    }
    #[cfg(not(feature = "mojo"))]
    {
        for &(class, action, rule) in RUNTIME_STREAM_ERROR_RULES {
            if let Some(message) = runtime_error_signal_message_from_value_mode(
                value,
                class,
                RuntimeSignalMatchMode::ExplicitCode,
            ) {
                return runtime_error_policy_match(class, action, rule, message, phase);
            }
        }
        RuntimeHttpErrorPolicy::pass_through()
    }
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
