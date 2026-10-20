//! Gemini tool-call thought-signature hardening.

use prodex_mojo_core::provider_constraints::{
    GeminiBridgeRequestKernelInput, GeminiBridgeRequestOperation, gemini_bridge_request_kernel,
};
use serde_json::Value;

fn hardening_error(message: &str) -> serde_json::Error {
    <serde_json::Error as serde::de::Error>::custom(message)
}

pub fn gemini_provider_core_harden_tool_call_thought_signatures(
    body: &mut Vec<u8>,
    model: &str,
) -> Result<usize, serde_json::Error> {
    if !prodex_mojo_core::json::gemini_model_requires_tool_thought_signature(model)
        .expect("compiled Mojo Gemini model signature policy returned invalid output")
    {
        return Ok(0);
    }
    let value: Value = serde_json::from_slice(body)?;
    let canonical = serde_json::to_vec(&value)?;
    let mut input = GeminiBridgeRequestKernelInput::new(
        GeminiBridgeRequestOperation::HardenToolCallThoughtSignatures,
    );
    input.primary = Some(&canonical);
    let output = gemini_bridge_request_kernel(input)
        .map_err(|_| hardening_error("Mojo Gemini hardening kernel failed"))?;
    let envelope: Vec<Value> = serde_json::from_slice(&output)?;
    let [rewritten, injected] = envelope.as_slice() else {
        return Err(hardening_error(
            "Mojo Gemini hardening kernel returned an invalid envelope",
        ));
    };
    let injected = injected
        .as_u64()
        .ok_or_else(|| hardening_error("Mojo Gemini hardening kernel returned an invalid count"))?;
    if injected == 0 {
        return Ok(0);
    }
    *body = serde_json::to_vec(rewritten)?;
    usize::try_from(injected)
        .map_err(|_| hardening_error("Mojo Gemini hardening kernel returned an overflowing count"))
}

pub fn gemini_provider_core_thought_signature(part: &serde_json::Value) -> Option<String> {
    let mut document = crate::mojo_json::Document::default();
    document.borrowed_shape(part);
    let result = prodex_mojo_core::json::gemini_value_projection(
        &document.nodes,
        "",
        prodex_mojo_core::json::GeminiValueOperation::ThoughtSignature,
    )
    .expect("compiled Mojo Gemini thought-signature selection returned invalid output");
    serde_json::from_slice(&result)
        .expect("compiled Mojo Gemini thought-signature selection must return an optional string")
}

#[cfg(test)]
mod tests {
    use super::gemini_provider_core_harden_tool_call_thought_signatures;

    #[test]
    fn compiled_thought_signature_preserves_original_text_and_presence_barriers() {
        for (input, expected) in [
            (
                serde_json::json!({"thoughtSignature":" signed 東京 "}),
                Some(" signed 東京 "),
            ),
            (
                serde_json::json!({"thought_signature":"legacy"}),
                Some("legacy"),
            ),
            (
                serde_json::json!({"thoughtSignature":"modern","thought_signature":"legacy"}),
                Some("modern"),
            ),
            (
                serde_json::json!({"thoughtSignature":null,"thought_signature":"blocked"}),
                None,
            ),
            (
                serde_json::json!({"thoughtSignature":12,"thought_signature":"blocked"}),
                None,
            ),
            (
                serde_json::json!({"thoughtSignature":"","thought_signature":"blocked"}),
                None,
            ),
            (
                serde_json::json!({"thoughtSignature":" \u{3000} ","thought_signature":"blocked"}),
                None,
            ),
            (serde_json::json!({"thought_signature":" \u{2003} "}), None),
            (serde_json::json!({}), None),
            (serde_json::json!([]), None),
            (serde_json::json!("text"), None),
            (serde_json::json!(null), None),
        ] {
            assert_eq!(
                super::gemini_provider_core_thought_signature(&input).as_deref(),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn compiled_gemini_three_model_policy_requires_exact_case_sensitive_substring() {
        for (model, expected) in [
            ("gemini-3", true),
            ("gemini-3.1-pro-preview", true),
            ("prefix-gemini-3-suffix", true),
            ("gemini-3-雪", true),
            ("gemini-2.5-pro", false),
            ("Gemini-3", false),
            ("gemini- 3", false),
            ("gemini-30", true),
            ("gemini-3.0", true),
            ("", false),
        ] {
            assert_eq!(
                prodex_mojo_core::json::gemini_model_requires_tool_thought_signature(model),
                Ok(expected),
            );
        }
    }

    #[test]
    fn mojo_hardening_injects_only_the_first_unsigned_call() {
        let mut body = serde_json::to_vec(&serde_json::json!({
            "request": {
                "contents": [{
                    "role": "model",
                    "parts": [
                        {"functionCall": {"name": "first"}},
                        {"functionCall": {"name": "second"}}
                    ]
                }]
            }
        }))
        .unwrap();

        assert_eq!(
            gemini_provider_core_harden_tool_call_thought_signatures(
                &mut body,
                "gemini-3.1-pro-preview",
            )
            .unwrap(),
            1
        );
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value["request"]["contents"][0]["parts"][0]["thoughtSignature"],
            "skip_thought_signature_validator"
        );
        assert!(
            value["request"]["contents"][0]["parts"][1]
                .get("thoughtSignature")
                .is_none()
        );
    }

    #[test]
    fn mojo_hardening_preserves_signature_presence_barriers_and_root_contents() {
        let mut body = serde_json::to_vec(&serde_json::json!({
            "contents": [{
                "role": "model",
                "parts": [
                    {"functionCall": {"name": "null"}, "thoughtSignature": null},
                    {"functionCall": {"name": "blank"}, "thought_signature": "\u{3000}"}
                ]
            }]
        }))
        .unwrap();
        assert_eq!(
            gemini_provider_core_harden_tool_call_thought_signatures(&mut body, "gemini-3")
                .unwrap(),
            1
        );
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value["contents"][0]["parts"][0]["thoughtSignature"],
            "skip_thought_signature_validator"
        );
        assert!(
            value["contents"][0]["parts"][1]
                .get("thoughtSignature")
                .is_none()
        );
    }

    #[test]
    fn non_gemini_three_models_skip_the_mojo_kernel() {
        let original = br#"{"contents":[{"role":"model","parts":[{"functionCall":{}}]}]}"#;
        let mut body = original.to_vec();
        assert_eq!(
            gemini_provider_core_harden_tool_call_thought_signatures(&mut body, "gemini-2.5-pro")
                .unwrap(),
            0
        );
        assert_eq!(body, original);
    }

    #[test]
    fn signed_gemini_three_body_keeps_original_bytes_when_no_injection_is_needed() {
        let original = br#" { "contents" : [ { "role" : "model", "parts" : [ { "functionCall" : {}, "thoughtSignature" : "signed" } ] } ] } "#;
        let mut body = original.to_vec();
        assert_eq!(
            gemini_provider_core_harden_tool_call_thought_signatures(&mut body, "gemini-3")
                .unwrap(),
            0
        );
        assert_eq!(body, original);
    }

    #[test]
    fn escaped_signature_key_is_seen_before_mojo_rewrite() {
        let original = br#"{"contents":[{"role":"model","parts":[{"functionCall":{},"thought\u0053ignature":"signed"}]}]}"#;
        let mut body = original.to_vec();
        assert_eq!(
            gemini_provider_core_harden_tool_call_thought_signatures(&mut body, "gemini-3")
                .unwrap(),
            0
        );
        assert_eq!(body, original);
    }

    #[test]
    fn large_gemini_three_body_uses_the_signature_size_budget() {
        let mut body = serde_json::to_vec(&serde_json::json!({
            "padding": "x".repeat(5 * 1024 * 1024),
            "contents": [{"role": "user", "parts": [{"text": "keep"}]}]
        }))
        .unwrap();
        let original = body.clone();
        assert_eq!(
            gemini_provider_core_harden_tool_call_thought_signatures(&mut body, "gemini-3")
                .unwrap(),
            0
        );
        assert_eq!(body, original);
    }
}
