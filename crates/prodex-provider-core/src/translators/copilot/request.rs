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
    copilot_provider_core_responses_input_has_image(&value)
        || copilot_provider_core_chat_messages_have_image(&value)
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
            let preserve_encrypted_content =
                object.get("type").and_then(Value::as_str) == Some("compaction");
            let mut changed =
                !preserve_encrypted_content && object.remove("encrypted_content").is_some();
            for value in object.values_mut() {
                changed |= copilot_provider_core_strip_encrypted_content(value);
            }
            changed
        }
        _ => false,
    }
}

fn copilot_provider_core_responses_input_has_image(value: &Value) -> bool {
    value
        .get("input")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(copilot_provider_core_responses_input_item_has_image)
        })
}

fn copilot_provider_core_responses_input_item_has_image(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    match object.get("type").and_then(Value::as_str) {
        Some("input_image") => copilot_provider_core_responses_image_payload_is_present(object),
        Some("message") => copilot_provider_core_responses_content_has_image(object.get("content")),
        _ => false,
    }
}

fn copilot_provider_core_responses_content_has_image(value: Option<&Value>) -> bool {
    value.and_then(Value::as_array).is_some_and(|items| {
        items
            .iter()
            .any(copilot_provider_core_responses_image_item_has_payload)
    })
}

fn copilot_provider_core_responses_image_item_has_payload(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        object.get("type").and_then(Value::as_str) == Some("input_image")
            && copilot_provider_core_responses_image_payload_is_present(object)
    })
}

fn copilot_provider_core_responses_image_payload_is_present(
    object: &serde_json::Map<String, Value>,
) -> bool {
    ["image_url", "file_id"].into_iter().any(|key| {
        object
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    })
}

fn copilot_provider_core_chat_messages_have_image(value: &Value) -> bool {
    value
        .get("messages")
        .and_then(Value::as_array)
        .is_some_and(|messages| {
            messages.iter().any(|message| {
                let Some(object) = message.as_object() else {
                    return false;
                };
                object.get("role").and_then(Value::as_str) == Some("user")
                    && copilot_provider_core_chat_content_has_image(object.get("content"))
            })
        })
}

fn copilot_provider_core_chat_content_has_image(value: Option<&Value>) -> bool {
    value.and_then(Value::as_array).is_some_and(|items| {
        items
            .iter()
            .any(copilot_provider_core_chat_image_item_has_payload)
    })
}

fn copilot_provider_core_chat_image_item_has_payload(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.get("type").and_then(Value::as_str) == Some("image_url")
        && object
            .get("image_url")
            .and_then(Value::as_object)
            .and_then(|image| image.get("url"))
            .and_then(Value::as_str)
            .is_some_and(|url| !url.trim().is_empty())
}
