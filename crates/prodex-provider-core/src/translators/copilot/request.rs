//! Copilot request-shape helpers used by the runtime compatibility shim.

use crate::{
    ProviderId, provider_canonical_model, provider_model_from_request_body,
    provider_request_body_with_model,
};
use serde_json::Value;

pub fn copilot_provider_core_request_body_with_canonical_model(body: &[u8]) -> Vec<u8> {
    let Some(model) = provider_model_from_request_body(body) else {
        return body.to_vec();
    };
    let canonical = provider_canonical_model(ProviderId::Copilot, &model);
    provider_request_body_with_model(body, &canonical)
}

pub fn copilot_provider_core_request_body_without_encrypted_content(
    body: &[u8],
) -> (Vec<u8>, bool) {
    let Ok(mut value) = serde_json::from_slice::<Value>(body) else {
        return (body.to_vec(), false);
    };
    if !copilot_provider_core_strip_encrypted_content(&mut value) {
        return (body.to_vec(), false);
    }
    match serde_json::to_vec(&value) {
        Ok(body) => (body, true),
        Err(_) => (body.to_vec(), false),
    }
}

pub fn copilot_provider_core_request_has_agent_input(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return false;
    };
    if value
        .get("messages")
        .and_then(Value::as_array)
        .is_some_and(|messages| {
            messages.iter().any(|message| {
                message
                    .get("role")
                    .and_then(Value::as_str)
                    .is_some_and(|role| {
                        role.eq_ignore_ascii_case("assistant") || role.eq_ignore_ascii_case("tool")
                    })
            })
        })
    {
        return true;
    }

    value
        .get("input")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.as_object().is_some_and(|object| {
                    object
                        .get("role")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|role| !role.is_empty())
                        .is_none_or(|role| role.eq_ignore_ascii_case("assistant"))
                })
            })
        })
}

pub fn copilot_provider_core_request_has_vision_input(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return false;
    };
    copilot_provider_core_value_contains_text(&value, "input_image")
        || copilot_provider_core_value_contains_key(&value, "image_url")
}

fn copilot_provider_core_strip_encrypted_content(value: &mut Value) -> bool {
    match value {
        Value::Array(values) => {
            let mut changed = false;
            for value in values {
                changed |= copilot_provider_core_strip_encrypted_content(value);
            }
            changed
        }
        Value::Object(object) => {
            let mut changed = object.remove("encrypted_content").is_some();
            for value in object.values_mut() {
                changed |= copilot_provider_core_strip_encrypted_content(value);
            }
            changed
        }
        _ => false,
    }
}

fn copilot_provider_core_value_contains_text(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.contains(needle),
        Value::Array(values) => values
            .iter()
            .any(|value| copilot_provider_core_value_contains_text(value, needle)),
        Value::Object(object) => object
            .values()
            .any(|value| copilot_provider_core_value_contains_text(value, needle)),
        _ => false,
    }
}

fn copilot_provider_core_value_contains_key(value: &Value, needle: &str) -> bool {
    match value {
        Value::Array(values) => values
            .iter()
            .any(|value| copilot_provider_core_value_contains_key(value, needle)),
        Value::Object(object) => {
            object.contains_key(needle)
                || object
                    .values()
                    .any(|value| copilot_provider_core_value_contains_key(value, needle))
        }
        _ => false,
    }
}
