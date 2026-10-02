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
    prodex_mojo_core::observability::runtime_http_error_class_label(class as i64)
        .expect("Mojo HTTP error-class label returned invalid output")
}

pub fn runtime_http_error_action_label(action: super::RuntimeHttpErrorAction) -> &'static str {
    prodex_mojo_core::observability::runtime_http_error_action_label(action as i64)
        .expect("Mojo HTTP error-action label returned invalid output")
}
