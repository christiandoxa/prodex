use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_model_fallback_chain,
    runtime_provider_model_from_body, runtime_provider_request_body_with_model,
};
use crate::RuntimeProxyRequest;

pub(super) fn runtime_copilot_request_body_with_canonical_model(body: &[u8]) -> Vec<u8> {
    let Some(model) = runtime_provider_model_from_body(body) else {
        return body.to_vec();
    };
    let canonical =
        runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Copilot, &model)
            .into_iter()
            .next()
            .unwrap_or(model);
    runtime_provider_request_body_with_model(body, &canonical)
}

pub(super) fn runtime_copilot_initiator_header(request: &RuntimeProxyRequest) -> &'static str {
    if runtime_copilot_request_has_agent_input(&request.body) {
        "agent"
    } else {
        "user"
    }
}

fn runtime_copilot_request_has_agent_input(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.as_object().is_some_and(|object| {
                    object
                        .get("role")
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|role| !role.is_empty())
                        .is_none_or(|role| role.eq_ignore_ascii_case("assistant"))
                })
            })
        })
}

pub(super) fn runtime_copilot_request_has_vision_input(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    runtime_copilot_value_contains_text(&value, "input_image")
        || runtime_copilot_value_contains_key(&value, "image_url")
}

fn runtime_copilot_value_contains_text(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains(needle),
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| runtime_copilot_value_contains_text(value, needle)),
        serde_json::Value::Object(object) => object
            .values()
            .any(|value| runtime_copilot_value_contains_text(value, needle)),
        _ => false,
    }
}

fn runtime_copilot_value_contains_key(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| runtime_copilot_value_contains_key(value, needle)),
        serde_json::Value::Object(object) => {
            object.contains_key(needle)
                || object
                    .values()
                    .any(|value| runtime_copilot_value_contains_key(value, needle))
        }
        _ => false,
    }
}
