use crate::RuntimeProxyRequest;

pub(super) fn runtime_copilot_request_body_with_canonical_model(body: &[u8]) -> Vec<u8> {
    prodex_provider_core::copilot_provider_core_request_body_with_canonical_model(body)
}

pub(super) fn runtime_copilot_request_body_without_encrypted_content(
    body: &[u8],
) -> (Vec<u8>, bool) {
    prodex_provider_core::copilot_provider_core_request_body_without_encrypted_content(body)
}

pub(super) fn runtime_copilot_initiator_header(request: &RuntimeProxyRequest) -> &'static str {
    if runtime_copilot_request_has_agent_input(&request.body) {
        "agent"
    } else {
        "user"
    }
}

fn runtime_copilot_request_has_agent_input(body: &[u8]) -> bool {
    prodex_provider_core::copilot_provider_core_request_has_agent_input(body)
}

pub(super) fn runtime_copilot_request_has_vision_input(body: &[u8]) -> bool {
    prodex_provider_core::copilot_provider_core_request_has_vision_input(body)
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

    #[test]
    fn copilot_request_body_strips_encrypted_content_only() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gpt-5.5",
            "input": [
                {
                    "type": "reasoning",
                    "id": "rs_1",
                    "encrypted_content": "qAAA.VDK="
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "what is this?"},
                        {
                            "type": "input_image",
                            "image_url": "data:image/png;base64,iVBORw0KGgo="
                        }
                    ]
                }
            ],
            "metadata": {
                "nested": {
                    "encrypted_content": "qBBB.VDK="
                }
            }
        }))
        .unwrap();

        let (stripped, changed) = runtime_copilot_request_body_without_encrypted_content(&body);
        let value: serde_json::Value = serde_json::from_slice(&stripped).unwrap();

        assert!(changed);
        assert!(value.to_string().contains("input_image"));
        assert!(value.to_string().contains("iVBORw0KGgo="));
        assert!(!value.to_string().contains("encrypted_content"));
        assert_eq!(value["input"][0]["type"], "reasoning");
    }

    #[test]
    fn copilot_request_body_without_encrypted_content_is_noop_when_absent() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gpt-5.5",
            "input": "hi"
        }))
        .unwrap();

        let (next, changed) = runtime_copilot_request_body_without_encrypted_content(&body);

        assert!(!changed);
        assert_eq!(next, body);
    }
}
