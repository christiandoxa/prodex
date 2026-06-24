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
    if value
        .get("messages")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|messages| {
            messages.iter().any(|message| {
                message
                    .get("role")
                    .and_then(serde_json::Value::as_str)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn request(body: serde_json::Value) -> RuntimeProxyRequest {
        RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/chat/completions".to_string(),
            headers: Vec::new(),
            body: serde_json::to_vec(&body).unwrap(),
        }
    }

    #[test]
    fn copilot_initiator_header_detects_chat_agent_messages() {
        let user = request(serde_json::json!({
            "model": "gpt-5.1-codex",
            "messages": [{"role": "user", "content": "hi"}]
        }));
        assert_eq!(runtime_copilot_initiator_header(&user), "user");

        let assistant = request(serde_json::json!({
            "model": "gpt-5.1-codex",
            "messages": [
                {"role": "user", "content": "hi"},
                {"role": "assistant", "content": "hello"}
            ]
        }));
        assert_eq!(runtime_copilot_initiator_header(&assistant), "agent");

        let tool = request(serde_json::json!({
            "model": "gpt-5.1-codex",
            "messages": [
                {"role": "assistant", "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "run", "arguments": "{}"}}]},
                {"role": "tool", "tool_call_id": "call_1", "content": "ok"}
            ]
        }));
        assert_eq!(runtime_copilot_initiator_header(&tool), "agent");
    }
}
