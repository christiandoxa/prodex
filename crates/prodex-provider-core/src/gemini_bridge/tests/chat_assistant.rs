use super::super::gemini_provider_core_chat_assistant_messages;
use serde_json::{Value, json};

#[test]
fn gemini_chat_assistant_message_assembly_runs_in_mojo_in_source_order() {
    let response = json!({
        "candidates": [{
            "content": {"parts": [
                {"text": "suppressed before tool"},
                {"text": "🧠", "thought": true},
                {"executableCode": {"language": "rust", "code": "println!(\"雪\");"}},
                {"inlineData": {"mimeType": "image/png", "data": "fake-image"}},
                {"inlineData": {"mimeType": "image/png", "data": "fake-image"}},
                {"videoMetadata": {"startOffset": "0s"}},
                {"videoMetadata": {"startOffset": "0s"}},
                {"functionCall": null},
                {"text": "suppressed after tool"},
                {"functionCall": {
                    "id": "  ",
                    "name": "exec_command",
                    "args": {"cmd": "rg foo"}
                }},
                {"functionCall": {"id": "call_explicit"}}
            ]}
        }]
    });
    let mut callback_calls = Vec::<(String, Value)>::new();
    let messages = gemini_provider_core_chat_assistant_messages(&response, 77, |name, args| {
        let block = callback_calls.is_empty();
        callback_calls.push((name.to_string(), args.clone()));
        block.then(|| "blocked λ".to_string())
    });

    assert_eq!(
        callback_calls,
        vec![
            ("tool_call".to_string(), json!({})),
            ("exec_command".to_string(), json!({"cmd": "rg foo"})),
            ("tool_call".to_string(), json!({})),
        ]
    );
    let code = "Gemini executable code (rust):\n```rust\nprintln!(\"雪\");\n```";
    let video = "Gemini video metadata: {\"startOffset\":\"0s\"}";
    assert_eq!(
        messages,
        vec![json!({
            "role": "assistant",
            "content": format!("{code}\n{video}\n{video}\nblocked λ"),
            "reasoning_content": "🧠",
            "gemini_media_content": [
                {"type": "input_image", "image_url": "data:image/png;base64,fake-image"},
                {"type": "input_image", "image_url": "data:image/png;base64,fake-image"}
            ],
            "gemini_native_parts": [
                {"inlineData": {"mimeType": "image/png", "data": "fake-image"}},
                {"inlineData": {"mimeType": "image/png", "data": "fake-image"}},
                {"videoMetadata": {"startOffset": "0s"}}
            ],
            "tool_calls": [{
                "id": "call_gemini_77_9",
                "type": "function",
                "function": {
                    "name": "exec_command",
                    "arguments": "{\"cmd\":\"rtk rg foo\"}"
                }
            }, {
                "id": "call_explicit",
                "type": "function",
                "function": {"name": "tool_call", "arguments": "{}"}
            }]
        })]
    );
}

#[test]
fn gemini_chat_assistant_message_accepts_unicode_text_above_four_mib() {
    let text = "雪".repeat(1_400_000);
    assert!(text.len() > 4 * 1024 * 1024);
    let response = json!({"candidates": [{"content": {"parts": [{"text": text}]}}]});

    let messages = gemini_provider_core_chat_assistant_messages(&response, 78, |_, _| None);

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["content"], text);
}
