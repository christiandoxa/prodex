use super::{
    RuntimeHttpErrorClass, RuntimeHttpErrorSignal, RuntimeSignalMatchMode, json::runtime_json_find,
    runtime_error_signal_candidate,
};

pub fn runtime_error_signal_message_from_value(
    value: &serde_json::Value,
    signal: RuntimeHttpErrorClass,
) -> Option<String> {
    runtime_error_signal_message_from_value_mode(
        value,
        signal,
        RuntimeSignalMatchMode::UsageMessage,
    )
}

pub(super) fn runtime_error_signal_message_from_value_mode(
    value: &serde_json::Value,
    signal: RuntimeHttpErrorClass,
    mode: RuntimeSignalMatchMode,
) -> Option<String> {
    match signal {
        RuntimeHttpErrorClass::Quota => runtime_json_find(value, |candidate| {
            runtime_error_signal_candidate(candidate, RuntimeHttpErrorSignal::ExplicitQuota, mode)
        }),
        RuntimeHttpErrorClass::RateLimited => runtime_json_find(value, |candidate| {
            runtime_error_signal_candidate(
                candidate,
                RuntimeHttpErrorSignal::ExplicitRateLimit,
                RuntimeSignalMatchMode::ExplicitCode,
            )
        }),
        RuntimeHttpErrorClass::ProfileUnavailable => runtime_json_find(value, |candidate| {
            runtime_error_signal_candidate(
                candidate,
                RuntimeHttpErrorSignal::ExplicitProfileUnavailable,
                mode,
            )
        }),
        RuntimeHttpErrorClass::Overload => runtime_json_find(value, |candidate| {
            runtime_error_signal_candidate(
                candidate,
                RuntimeHttpErrorSignal::ExplicitOverload,
                mode,
            )
        }),
        RuntimeHttpErrorClass::TransientServer | RuntimeHttpErrorClass::Other => None,
    }
}
