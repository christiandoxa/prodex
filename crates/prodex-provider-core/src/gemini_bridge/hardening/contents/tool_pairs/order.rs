//! Gemini response-order projection through the existing compiled policy.
pub(in crate::gemini_bridge::hardening::contents) fn gemini_provider_core_refine_tool_response_order(
    contents: &mut [serde_json::Value],
) {
    let ordered = super::super::mojo::transform(
        contents,
        prodex_mojo_core::json::GeminiContentOperation::OrderResponses,
    );
    assert_eq!(
        ordered.len(),
        contents.len(),
        "Mojo ordering must preserve content row count"
    );
    for (target, source) in contents.iter_mut().zip(ordered) {
        *target = source;
    }
}

#[cfg(test)]
mod tests {
    use super::gemini_provider_core_refine_tool_response_order;
    use serde_json::json;

    #[test]
    fn rust_caller_keeps_duplicate_unmatched_and_other_part_order() {
        let mut contents = vec![
            json!({"role": "model", "parts": [
                {"functionCall": {"id": "call-a", "name": "tool"}},
                {"functionCall": {"id": "call-b", "name": "tool"}},
                {"functionCall": {"id": "call-a", "name": "tool"}}
            ]}),
            json!({"role": "user", "parts": [
                {"text": "before"},
                {"functionResponse": {"id": "missing-a", "response": {"item": 1}}},
                {"functionResponse": {"id": "call-b", "response": {"item": 2}}},
                {"inlineData": {"mimeType": "image/png", "data": "fixture"}},
                {"functionResponse": {"id": "call-a", "response": {"item": 3}}},
                {"functionResponse": {"id": "call-a", "response": {"item": 4}}},
                {"functionResponse": {"id": "missing-b", "response": {"item": 5}}},
                {"functionResponse": {"response": {"item": 6}}},
                {"text": "after"}
            ]}),
        ];

        gemini_provider_core_refine_tool_response_order(&mut contents);

        assert_eq!(
            contents[1]["parts"],
            json!([
                {"functionResponse": {"id": "call-a", "response": {"item": 3}}},
                {"functionResponse": {"id": "call-a", "response": {"item": 4}}},
                {"functionResponse": {"id": "call-b", "response": {"item": 2}}},
                {"functionResponse": {"id": "missing-a", "response": {"item": 1}}},
                {"functionResponse": {"id": "missing-b", "response": {"item": 5}}},
                {"functionResponse": {"response": {"item": 6}}},
                {"text": "before"},
                {"inlineData": {"mimeType": "image/png", "data": "fixture"}},
                {"text": "after"}
            ])
        );
    }

    #[test]
    fn rust_caller_keeps_parts_unchanged_without_call_ids() {
        let mut contents = vec![
            json!({"role": "model", "parts": [{"text": "no calls"}]}),
            json!({"role": "user", "parts": [
                {"text": "before"},
                {"functionResponse": {"id": "call-a"}},
                {"text": "after"}
            ]}),
        ];

        gemini_provider_core_refine_tool_response_order(&mut contents);

        assert_eq!(
            contents[1]["parts"],
            json!([
                {"text": "before"},
                {"functionResponse": {"id": "call-a"}},
                {"text": "after"}
            ])
        );
    }
}

#[cfg(test)]
mod projection_tests {
    use super::gemini_provider_core_refine_tool_response_order;
    use serde_json::json;

    #[test]
    fn mojo_ordering_preserves_ineligible_rows_and_empty_ids() {
        let model = json!({"role":"model", "parts":[{"functionCall":null}]});
        let user = json!({"role":"user", "parts":[{"text":"keep"},{"functionResponse":null}], "metadata":42});
        let unchanged =
            json!({"role":"custom", "parts":[{"text":"custom"},{"functionResponse":null}]});
        let mut contents = vec![model.clone(), user, unchanged.clone()];
        gemini_provider_core_refine_tool_response_order(&mut contents);
        assert_eq!(
            contents,
            vec![
                model,
                json!({"role":"user", "parts":[{"functionResponse":null},{"text":"keep"}], "metadata":42}),
                unchanged
            ]
        );
        let once = contents.clone();
        gemini_provider_core_refine_tool_response_order(&mut contents);
        assert_eq!(contents, once);
    }
}
