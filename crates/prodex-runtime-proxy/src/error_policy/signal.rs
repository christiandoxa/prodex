use super::RuntimeHttpErrorClass;

pub fn runtime_error_signal_message_from_value(
    value: &serde_json::Value,
    signal: RuntimeHttpErrorClass,
) -> Option<String> {
    let (mode, expected_class) = match signal {
        RuntimeHttpErrorClass::Quota => (prodex_mojo_core::rich::RUNTIME_ERROR_MODE_JSON_QUOTA, 1),
        RuntimeHttpErrorClass::RateLimited => {
            (prodex_mojo_core::rich::RUNTIME_ERROR_MODE_JSON_RATE, 2)
        }
        RuntimeHttpErrorClass::ProfileUnavailable => {
            (prodex_mojo_core::rich::RUNTIME_ERROR_MODE_JSON_PROFILE, 3)
        }
        RuntimeHttpErrorClass::Overload => {
            (prodex_mojo_core::rich::RUNTIME_ERROR_MODE_JSON_OVERLOAD, 4)
        }
        RuntimeHttpErrorClass::TransientServer | RuntimeHttpErrorClass::Other => return None,
    };
    let body = serde_json::to_vec(value).ok()?;
    prodex_mojo_core::MojoError::rich_runtime_error_policy(mode, 0, 0, &body)
        .ok()
        .filter(|(class, _, _)| *class == expected_class)
        .map(|(_, _, message)| message)
}
