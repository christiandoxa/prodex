use super::{RuntimeHttpErrorClass, RuntimeHttpErrorPhase, RuntimeHttpErrorPolicy};

pub fn runtime_stream_error_policy(
    body: &[u8],
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    let policy = super::runtime_error_policy_from_mojo(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_STREAM,
        0,
        phase,
        body,
    );
    if body.len() > 65_536 {
        return policy;
    }
    match serde_json::from_slice(body) {
        Ok(value) => with_json_retry_advice(&value, body.len(), policy),
        Err(_) => policy,
    }
}

pub fn runtime_stream_error_policy_from_value(
    value: &serde_json::Value,
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    let Ok(body) = serde_json::to_vec(value) else {
        return RuntimeHttpErrorPolicy::pass_through();
    };
    let policy = super::runtime_error_policy_from_mojo(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_STREAM,
        0,
        phase,
        &body,
    );
    with_json_retry_advice(value, body.len(), policy)
}

fn with_json_retry_advice(
    value: &serde_json::Value,
    body_len: usize,
    mut policy: RuntimeHttpErrorPolicy,
) -> RuntimeHttpErrorPolicy {
    if body_len <= 65_536
        && matches!(
            policy.class,
            RuntimeHttpErrorClass::RateLimited
                | RuntimeHttpErrorClass::Overload
                | RuntimeHttpErrorClass::TransientServer
        )
    {
        policy.retry_after = super::retry_after_json::retry_after(value, policy.retry_after);
    }
    policy
}

pub fn runtime_http_error_class_label(class: RuntimeHttpErrorClass) -> &'static str {
    prodex_mojo_core::observability::runtime_http_error_class_label(class as i64)
        .expect("Mojo HTTP error-class label returned invalid output")
}

pub fn runtime_http_error_action_label(action: super::RuntimeHttpErrorAction) -> &'static str {
    prodex_mojo_core::observability::runtime_http_error_action_label(action as i64)
        .expect("Mojo HTTP error-action label returned invalid output")
}
