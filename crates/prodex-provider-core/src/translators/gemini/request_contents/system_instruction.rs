//! Gemini systemInstruction shaping from Responses input.

use prodex_mojo_core::provider_constraints::{
    GeminiRequestContentKernelInput, GeminiRequestContentOperation,
};
use serde_json::Value;

pub(crate) fn gemini_system_instruction_from_request(
    value: &Value,
) -> Result<Option<Value>, String> {
    let request = serde_json::to_vec(value).map_err(|error| {
        format!("failed to serialize Gemini system-instruction request: {error}")
    })?;
    let mut input = GeminiRequestContentKernelInput::new(
        GeminiRequestContentOperation::SystemInstructionFromRequest,
    );
    input.primary = Some(&request);
    let body = prodex_mojo_core::provider_constraints::gemini_request_content_kernel(input)
        .map_err(|error| format!("Mojo Gemini system-instruction kernel failed: {error:?}"))?;
    let instruction: Value = serde_json::from_slice(&body).map_err(|error| {
        format!("Mojo Gemini system-instruction kernel returned invalid JSON: {error}")
    })?;
    Ok((!instruction.is_null()).then_some(instruction))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fixed_mojo_result_joins_system_and_contextual_user_text() {
        assert_eq!(
            gemini_system_instruction_from_request(&json!({
                "input": [
                    {"role": "system", "content": "system one"},
                    {"role": "user", "content": "  <environment_context>synthetic</environment_context>"},
                    {"role": "system", "content": [{"text": "system two"}, {"content": "system three"}]},
                    {"role": "user", "content": "actual request"}
                ]
            }))
            .unwrap(),
            Some(json!({"parts": [{"text": "system one\n\nsystem two\nsystem three\n\n  <environment_context>synthetic</environment_context>"}]}))
        );
    }

    #[test]
    fn empty_and_malformed_shapes_keep_none_and_text_fallback() {
        assert_eq!(
            gemini_system_instruction_from_request(&json!({"input": [
                {"role": "system", "content": " \t"},
                {"role": "user", "content": " \n\n "}
            ]}))
            .unwrap(),
            None
        );
        assert_eq!(
            gemini_system_instruction_from_request(&json!({"input": [
                null,
                7,
                {"role": "system", "content": false, "text": "fallback"},
                {"role": "assistant", "content": "ignored"}
            ]}))
            .unwrap(),
            Some(json!({"parts": [{"text": "fallback"}]}))
        );
    }
}
