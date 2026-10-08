//! Gemini request tool-call thought-signature preservation.

use serde_json::{Value, json};

pub(crate) fn gemini_preserve_tool_call_signatures(messages: &mut [Value]) {
    for message in messages {
        let Some(tool_calls) = message.get_mut("tool_calls").and_then(Value::as_array_mut) else {
            continue;
        };
        for tool_call in tool_calls {
            let Some(object) = tool_call.as_object_mut() else {
                continue;
            };
            // The ABI describes candidate JSON fields; Mojo alone decides
            // precedence, the first-present barrier, and Unicode trimming.
            let google = object
                .get("extra_content")
                .and_then(|value| value.get("google"));
            let function = object.get("function");
            let values = [
                google.and_then(|value| value.get("thought_signature")),
                object.get("gemini_thought_signature"),
                object.get("thought_signature"),
                object.get("thoughtSignature"),
                function.and_then(|value| value.get("gemini_thought_signature")),
                function.and_then(|value| value.get("thought_signature")),
                function.and_then(|value| value.get("thoughtSignature")),
            ];
            let candidates = values.map(|value| {
                prodex_mojo_core::provider_constraints::GeminiSignatureCandidate {
                    present: value.is_some(),
                    text: value.and_then(Value::as_str),
                }
            });
            let Some(selected) =
                prodex_mojo_core::provider_constraints::gemini_signature_choice(&candidates)
                    .expect("Mojo Gemini thought-signature precedence failed")
            else {
                continue;
            };
            let signature = values[selected]
                .and_then(Value::as_str)
                .expect("Mojo Gemini signature choice selected a non-string")
                .to_string();
            object.remove("gemini_thought_signature");
            object.remove("thought_signature");
            object.remove("thoughtSignature");
            object.insert(
                "extra_content".to_string(),
                json!({
                    "google": {
                        "thought_signature": signature,
                    }
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::gemini_preserve_tool_call_signatures;
    use serde_json::json;

    #[test]
    fn gemini_preserve_tool_call_signatures_moves_google_signature_to_extra_content() {
        let mut messages = vec![json!({
            "role": "assistant",
            "tool_calls": [{
                "id": "call_1",
                "function": {"name": "shell", "arguments": "{\"cmd\":\"pwd\"}"},
                "gemini_thought_signature": "old",
                "thought_signature": "older",
                "thoughtSignature": "oldest",
                "extra_content": {
                    "google": {
                        "thought_signature": "signature-a"
                    }
                }
            }]
        })];

        gemini_preserve_tool_call_signatures(&mut messages);

        assert_eq!(
            messages[0]["tool_calls"][0]["extra_content"]["google"]["thought_signature"],
            "signature-a"
        );
        assert!(
            messages[0]["tool_calls"][0]
                .get("gemini_thought_signature")
                .is_none()
        );
        assert!(
            messages[0]["tool_calls"][0]
                .get("thought_signature")
                .is_none()
        );
        assert!(
            messages[0]["tool_calls"][0]
                .get("thoughtSignature")
                .is_none()
        );
    }

    #[test]
    fn gemini_signature_mojo_presence_barrier_is_not_a_fallback_chain() {
        let mut messages = vec![json!({
            "tool_calls": [
                {"extra_content": {"google": {"thought_signature": null}},
                 "gemini_thought_signature": "must-not-win"},
                {"gemini_thought_signature": "\u{2003}\u{3000}",
                 "thought_signature": "also-must-not-win"},
                {"function": {"thoughtSignature": "\u{2003}winner\u{3000}"}},
                {"gemini_thought_signature": 42,
                 "thought_signature": "must-not-win"}
            ]
        })];
        gemini_preserve_tool_call_signatures(&mut messages);
        let calls = messages[0]["tool_calls"].as_array().unwrap();
        assert!(calls[0]["extra_content"]["google"]["thought_signature"].is_null());
        assert_eq!(calls[0]["gemini_thought_signature"], "must-not-win");
        assert!(calls[1]["extra_content"].is_null());
        assert_eq!(calls[1]["thought_signature"], "also-must-not-win");
        assert_eq!(
            calls[2]["extra_content"]["google"]["thought_signature"],
            "\u{2003}winner\u{3000}"
        );
        assert_eq!(calls[3]["gemini_thought_signature"], 42);
        assert_eq!(calls[3]["thought_signature"], "must-not-win");
    }

    #[test]
    fn gemini_signature_mojo_handles_large_selected_string_without_json_reencoding() {
        let signature = "a".repeat(5 * 1024 * 1024);
        let mut messages = vec![json!({
            "tool_calls": [{"thought_signature": signature}]
        })];
        gemini_preserve_tool_call_signatures(&mut messages);
        assert_eq!(
            messages[0]["tool_calls"][0]["extra_content"]["google"]["thought_signature"]
                .as_str()
                .unwrap()
                .len(),
            5 * 1024 * 1024,
        );
    }

    #[test]
    fn gemini_preserve_tool_call_signatures_uses_top_level_signature_as_fallback() {
        let mut messages = vec![json!({
            "role": "assistant",
            "tool_calls": [{
                "id": "call_1",
                "function": {"name": "shell", "arguments": "{\"cmd\":\"pwd\"}"},
                "gemini_thought_signature": "signature-a"
            }]
        })];

        gemini_preserve_tool_call_signatures(&mut messages);

        assert_eq!(
            messages[0]["tool_calls"][0]["extra_content"]["google"]["thought_signature"],
            "signature-a"
        );
        assert!(
            messages[0]["tool_calls"][0]
                .get("gemini_thought_signature")
                .is_none()
        );
    }
}
