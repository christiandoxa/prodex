#![cfg(feature = "mojo-provider-constraints")]

use prodex_mojo_core::provider_constraints::{
    GeminiBridgeRequestKernelInput, GeminiBridgeRequestOperation, GeminiRequestContentKernelInput,
    GeminiRequestContentOperation, gemini_bridge_request_kernel, gemini_request_content_kernel,
};

fn sanitize_function_schema(schema: &[u8]) -> Result<Vec<u8>, prodex_mojo_core::MojoError> {
    let mut input =
        GeminiRequestContentKernelInput::new(GeminiRequestContentOperation::SanitizeFunctionSchema);
    input.primary = Some(schema);
    gemini_request_content_kernel(input)
}

fn tool_config(request: &[u8]) -> Result<Vec<u8>, prodex_mojo_core::MojoError> {
    let mut input = GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::ToolConfig);
    input.primary = Some(request);
    gemini_bridge_request_kernel(input)
}

#[test]
fn schema_union_ignores_empty_any_of_before_one_of() {
    assert_eq!(
        sanitize_function_schema(br#"{"anyOf":[],"oneOf":[{"type":"string"}]}"#).unwrap(),
        br#"{"type":"string"}"#
    );
}

#[test]
fn schema_sanitizer_keeps_array_order_duplicates_unicode_and_filters_wrong_types() {
    assert_eq!(
        sanitize_function_schema(
            r#"{"type":"string","enum":["β","a","β"],"required":["z","x","z"]}"#.as_bytes()
        )
        .unwrap(),
        r#"{"type":"string","enum":["β","a","β"],"required":["z","x","z"]}"#.as_bytes()
    );
    assert_eq!(
        sanitize_function_schema(
            br#"{"type":false,"description":7,"format":" ","enum":[1,"x"],"properties":[],"required":[" ",false,"x"],"items":false}"#
        )
        .unwrap(),
        br#"{"type":"object","enum":["x"],"required":["x"],"items":{"type":"object"}}"#
    );
    assert_eq!(
        sanitize_function_schema(b"null").unwrap(),
        br#"{"type":"object"}"#
    );
}

#[test]
fn schema_sanitizer_handles_large_unicode_descriptions() {
    let description = "λ🙂".repeat(40_000);
    let input = format!(r#"{{"type":"string","description":"{description}"}}"#);
    let expected = input.clone();

    assert!(input.len() > 64 * 1024);
    assert_eq!(
        sanitize_function_schema(input.as_bytes()).unwrap(),
        expected.as_bytes()
    );
}

#[test]
fn tool_choice_uses_nested_name_when_valid_and_outer_name_when_nested_is_wrong_typed() {
    assert_eq!(
        tool_config(r#"{"tool_choice":{"function":{"name":7},"name":"fallback🙂"}}"#.as_bytes())
            .unwrap(),
        r#"{"functionCallingConfig":{"mode":"ANY","allowedFunctionNames":["fallback🙂"]}}"#
            .as_bytes()
    );
    assert_eq!(
        tool_config(br#"{"tool_choice":{"function":{"name":"nested"},"name":"outer"}}"#).unwrap(),
        br#"{"functionCallingConfig":{"mode":"ANY","allowedFunctionNames":["nested"]}}"#
    );
    assert_eq!(
        tool_config(br#"{"tool_choice":{"name":false}}"#).unwrap(),
        b"null"
    );
}

#[test]
fn tool_declaration_keeps_the_existing_json_shape() {
    let mut input =
        GeminiRequestContentKernelInput::new(GeminiRequestContentOperation::ToolDeclaration);
    input.primary = Some(br#""lookup""#);
    input.secondary = Some(br#""Look up a record""#);
    input.tertiary = Some(br#"{"type":"object"}"#);

    assert_eq!(
        gemini_request_content_kernel(input).unwrap(),
        br#"{"name":"lookup","description":"Look up a record","parameters":{"type":"object"}}"#
    );
}
