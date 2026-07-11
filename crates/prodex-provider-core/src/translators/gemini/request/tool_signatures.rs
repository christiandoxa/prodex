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
            let Some(signature) = object
                .get("extra_content")
                .and_then(|value| value.get("google"))
                .and_then(|value| value.get("thought_signature"))
                .or_else(|| object.get("gemini_thought_signature"))
                .or_else(|| object.get("thought_signature"))
                .or_else(|| object.get("thoughtSignature"))
                .or_else(|| {
                    object
                        .get("function")
                        .and_then(|value| value.get("gemini_thought_signature"))
                })
                .or_else(|| {
                    object
                        .get("function")
                        .and_then(|value| value.get("thought_signature"))
                })
                .or_else(|| {
                    object
                        .get("function")
                        .and_then(|value| value.get("thoughtSignature"))
                })
                .and_then(Value::as_str)
                .filter(|signature| !signature.trim().is_empty())
                .map(str::to_string)
            else {
                continue;
            };
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
