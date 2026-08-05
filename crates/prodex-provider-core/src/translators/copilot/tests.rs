//! Copilot provider-core characterization tests.

use super::{
    copilot_provider_core_request_body_with_canonical_model,
    copilot_provider_core_request_body_without_encrypted_content,
    copilot_provider_core_request_has_agent_input, copilot_provider_core_request_has_vision_input,
    copilot_provider_core_response_id_from_value,
};
use serde_json::{Value, json};

#[test]
fn copilot_provider_core_extracts_response_id_shapes() {
    assert_eq!(
        copilot_provider_core_response_id_from_value(&json!({
            "response": {"id": " resp_nested "}
        }))
        .as_deref(),
        Some("resp_nested")
    );
    assert_eq!(
        copilot_provider_core_response_id_from_value(&json!({"id": "resp_top"})).as_deref(),
        Some("resp_top")
    );
    assert_eq!(
        copilot_provider_core_response_id_from_value(&json!({"response_id": "resp_event"}))
            .as_deref(),
        Some("resp_event")
    );
    assert_eq!(
        copilot_provider_core_response_id_from_value(&json!({"id": "   "})),
        None
    );
}

#[test]
fn copilot_provider_core_rewrites_request_to_canonical_model() {
    let rewritten = copilot_provider_core_request_body_with_canonical_model(
        br#"{"model":"codex","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let value: Value = serde_json::from_slice(&rewritten).unwrap();

    assert_eq!(value["model"], "gpt-5.3-codex");
}

#[test]
fn copilot_provider_core_preserves_required_compaction_content() {
    let body = serde_json::to_vec(&json!({
        "messages": [{
            "role": "assistant",
            "content": [{"type": "text", "text": "keep", "encrypted_content": "qAAA.VDK="}]
        }],
        "reasoning": {"encrypted_content": "qBBB.VDK=", "summary": "keep"},
        "input": [{"type": "compaction", "encrypted_content": "enc-compact-v2"}]
    }))
    .unwrap();

    let (stripped, changed) = copilot_provider_core_request_body_without_encrypted_content(&body);
    let value: Value = serde_json::from_slice(&stripped).unwrap();

    assert!(changed);
    assert!(
        value["messages"][0]["content"][0]
            .get("encrypted_content")
            .is_none()
    );
    assert!(value["reasoning"].get("encrypted_content").is_none());
    assert_eq!(value["input"][0]["encrypted_content"], "enc-compact-v2");
    assert_eq!(value["messages"][0]["content"][0]["text"], "keep");
    assert_eq!(value["reasoning"]["summary"], "keep");
}

#[test]
fn copilot_provider_core_detects_agent_and_vision_input() {
    let agent = serde_json::to_vec(&json!({
        "messages": [{"role": "assistant", "content": "hello"}]
    }))
    .unwrap();
    let user = serde_json::to_vec(&json!({
        "messages": [{"role": "user", "content": "hello"}]
    }))
    .unwrap();
    let vision = serde_json::to_vec(&json!({
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_image",
                "image_url": "data:image/png;base64,AAAA"
            }]
        }]
    }))
    .unwrap();
    let direct_file_vision = serde_json::to_vec(&json!({
        "input": [{"type": "input_image", "file_id": "file-image-1"}]
    }))
    .unwrap();
    let direct_url_vision = serde_json::to_vec(&json!({
        "input": [{"type": "input_image", "image_url": "https://example.com/image.png"}]
    }))
    .unwrap();
    let nested_file_vision = serde_json::to_vec(&json!({
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{"type": "input_image", "file_id": "file-image-2"}]
        }]
    }))
    .unwrap();
    let chat_vision = serde_json::to_vec(&json!({
        "messages": [{
            "role": "user",
            "content": [{
                "type": "image_url",
                "image_url": {"url": "https://example.com/image.png"}
            }]
        }]
    }))
    .unwrap();
    let text_mention = serde_json::to_vec(&json!({
        "input": [{
            "type": "input_text",
            "text": "input_image and image_url are payload terms"
        }]
    }))
    .unwrap();
    let unrelated_type = serde_json::to_vec(&json!({
        "input": "hello",
        "metadata": {"type": "input_image"},
        "tools": [{
            "type": "function",
            "function": {
                "name": "inspect",
                "parameters": {"properties": {"kind": {"type": "image_url"}}}
            }
        }]
    }))
    .unwrap();

    assert!(copilot_provider_core_request_has_agent_input(&agent));
    assert!(!copilot_provider_core_request_has_agent_input(&user));
    assert!(copilot_provider_core_request_has_vision_input(&vision));
    assert!(copilot_provider_core_request_has_vision_input(
        &direct_file_vision
    ));
    assert!(copilot_provider_core_request_has_vision_input(
        &direct_url_vision
    ));
    assert!(copilot_provider_core_request_has_vision_input(
        &nested_file_vision
    ));
    assert!(copilot_provider_core_request_has_vision_input(&chat_vision));
    assert!(!copilot_provider_core_request_has_vision_input(
        &text_mention
    ));
    assert!(!copilot_provider_core_request_has_vision_input(
        &unrelated_type
    ));
}

#[test]
fn copilot_provider_core_rejects_nested_or_incomplete_vision_markers() {
    let cases = [
        json!({
            "input": [{
                "role": "user",
                "metadata": {"type": "input_image", "image_url": "https://example.com/image.png"}
            }]
        }),
        json!({
            "input": [{"type": "input_image", "image_url": {"url": "https://example.com/image.png"}}]
        }),
        json!({
            "input": [{"type": "input_image", "image_url": "   "}]
        }),
        json!({
            "input": [{"type": "input_image", "file_id": ""}]
        }),
        json!({
            "input": [{"type": "image_url", "image_url": "https://example.com/image.png"}]
        }),
        json!({
            "input": [{
                "type": "item_reference",
                "content": [{"type": "input_image", "image_url": "https://example.com/image.png"}]
            }]
        }),
        json!({
            "messages": [{
                "role": "tool",
                "content": [{
                    "type": "tool_output",
                    "output": {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
                }]
            }]
        }),
        json!({
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "schema"}]}],
            "tools": [{
                "type": "function",
                "function": {"parameters": {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}}
            }]
        }),
        json!({"input": [{"content": [{"type": "input_image"}]}]}),
        json!({
            "messages": [{
                "role": "user",
                "content": [{"type": "image_url", "image_url": "https://example.com/image.png"}]
            }]
        }),
        json!({
            "messages": [{
                "role": "user",
                "content": [{"type": "image_url", "image_url": {"url": "   "}}]
            }]
        }),
        json!({
            "messages": [{
                "role": "system",
                "content": [{"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}]
            }]
        }),
        json!({
            "messages": [{
                "role": "developer",
                "content": [{"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}]
            }]
        }),
        json!({
            "messages": [{
                "role": "assistant",
                "content": [{"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}]
            }]
        }),
    ];

    for value in cases {
        let body = serde_json::to_vec(&value).unwrap();
        assert!(!copilot_provider_core_request_has_vision_input(&body));
    }
}
