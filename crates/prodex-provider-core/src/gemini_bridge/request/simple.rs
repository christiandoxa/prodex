//! Gemini simple-request fast-path classification.

pub fn gemini_provider_core_simple_request(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    // Normalize escapes, duplicate keys, and object order at the Serde boundary.
    let Ok(body) = serde_json::to_vec(&value) else {
        return false;
    };
    #[cfg(feature = "mojo")]
    {
        super::request_contents::gemini_bridge_request_simple(&body)
    }
    #[cfg(not(feature = "mojo"))]
    {
        let mut input = prodex_mojo_core::provider_constraints::GeminiBridgeRequestKernelInput::new(
            prodex_mojo_core::provider_constraints::GeminiBridgeRequestOperation::SimpleRequest,
        );
        input.primary = Some(&body);
        prodex_mojo_core::provider_constraints::gemini_bridge_request_kernel(input)
            .ok()
            .and_then(|body| serde_json::from_slice::<bool>(&body).ok())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::gemini_provider_core_simple_request;

    #[test]
    fn simple_request_uses_mojo_for_classification() {
        for (body, expected) in [
            (br#"{"input":"hello"}"#.as_slice(), true),
            (br#"{"tools":[{"type":"\u0077eb_search"}],"input":"q"}"#.as_slice(), true),
            (br#"{"tools":[{"type":"function","function":{"name":"lookup","parameters":{}}}],"input":"q"}"#.as_slice(), false),
            (br#"{"input":[{"type":"message","content":[{"type":"image","url":"x"}]}]}"#.as_slice(), false),
            (br#"{"tools":[{"type":"function"}],"tools":[],"input":"q"}"#.as_slice(), true),
            (b"{\"input\":", false),
            (b"{\"input\":\"\xff\"}", false),
        ] {
            assert_eq!(gemini_provider_core_simple_request(body), expected);
        }
        let unicode = serde_json::to_vec(&serde_json::json!({"input": "雪🙂"})).unwrap();
        assert!(gemini_provider_core_simple_request(&unicode));
    }
}
