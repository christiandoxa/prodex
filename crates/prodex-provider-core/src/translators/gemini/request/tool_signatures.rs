//! Gemini request tool-call thought-signature preservation.

use crate::mojo_json::Document;
use prodex_mojo_core::provider_constraints::{
    GeminiBridgeRequestKernelInput, GeminiBridgeRequestOperation, gemini_bridge_request_kernel,
};
use serde_json::Value;

pub(crate) fn gemini_preserve_tool_call_signatures(messages: &mut [Value]) {
    let mut document = Document::default();
    document.array(messages.iter());
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let body = {
        let mut input = GeminiBridgeRequestKernelInput::new(
            GeminiBridgeRequestOperation::PreserveToolCallSignatures,
        );
        input.primary = Some(raw.as_bytes());
        gemini_bridge_request_kernel(input)
            .expect("Mojo Gemini thought-signature preservation failed")
    };
    drop(document);
    let output: Vec<Value> = serde_json::from_slice(&body)
        .expect("Mojo Gemini thought-signature preservation returned invalid JSON");
    assert_eq!(output.len(), messages.len());
    messages.clone_from_slice(&output);
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
