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
    if !gemini_provider_core_model_requires_tool_thought_signature(model) {
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
    part.get("thoughtSignature")
        .or_else(|| part.get("thought_signature"))
        .and_then(serde_json::Value::as_str)
        .filter(|signature| !signature.trim().is_empty())
        .map(str::to_string)
}

fn gemini_provider_core_model_requires_tool_thought_signature(model: &str) -> bool {
    model.contains("gemini-3")
}

#[cfg(test)]
mod tests {
    use super::gemini_provider_core_harden_tool_call_thought_signatures;

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
