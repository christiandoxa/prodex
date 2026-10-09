#![cfg(feature = "mojo-provider-constraints")]

use prodex_mojo_core::provider_constraints::{
    GeminiBridgeRequestKernelInput, GeminiBridgeRequestOperation, GeminiRequestContentKernelInput,
    GeminiRequestContentOperation, GeminiSignatureCandidate, gemini_bridge_request_kernel,
    gemini_request_content_kernel, gemini_signature_choice,
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

fn validate_translator(request: &[u8]) -> Result<Vec<u8>, prodex_mojo_core::MojoError> {
    let mut input = GeminiBridgeRequestKernelInput::new(
        GeminiBridgeRequestOperation::ValidateTranslatorRequest,
    );
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

#[test]
fn translator_validation_keeps_malformed_and_null_tool_precedence_in_mojo() {
    let cases = [
        (
            br#"{"tools":null}"#.as_slice(),
            "invalid_tool_declaration: Gemini request field `tools` must be an array",
        ),
        (
            br#"{"tools":[null]}"#.as_slice(),
            "invalid_tool_declaration: Gemini request field `tools[0]` must be an object",
        ),
        (
            br#"{"tools":[{"type":"function","function":{"name":"lookup"}}]}"#.as_slice(),
            "invalid_tool_declaration: Gemini request field `tools[0].function.parameters` is required",
        ),
    ];
    for (request, reason) in cases {
        let output = String::from_utf8(validate_translator(request).unwrap()).unwrap();
        assert!(output.contains(reason), "output={output}");
    }
}

#[test]
fn translator_validation_accepts_flat_function_and_preserves_unsupported_tool_tag() {
    let flat = String::from_utf8(
        validate_translator(br#"{"tools":[{"type":"function","name":"lookup","parameters":{}}]}"#)
            .unwrap(),
    )
    .unwrap();
    assert!(flat.contains(r#""tag":0"#), "output={flat}");

    let unsupported = String::from_utf8(
        validate_translator(br#"{"tools":[{"type":"custom","name":"lookup"}]}"#).unwrap(),
    )
    .unwrap();
    assert!(unsupported.contains(r#""tag":16"#), "output={unsupported}");
    assert!(
        unsupported.contains(r#""reason":null"#),
        "output={unsupported}"
    );
}

#[test]
fn translator_validation_rejects_oversized_json_before_mojo_execution() {
    let oversized = vec![b' '; 4 * 1024 * 1024 + 1];
    assert_eq!(
        validate_translator(&oversized),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}

#[test]
fn signature_choice_keeps_invalid_presence_from_falling_through() {
    let candidates = [
        GeminiSignatureCandidate {
            present: true,
            text: None,
        },
        GeminiSignatureCandidate {
            present: true,
            text: Some("fallback"),
        },
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
    ];
    assert_eq!(gemini_signature_choice(&candidates).unwrap(), None);

    let candidates = [
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
        GeminiSignatureCandidate {
            present: true,
            text: Some("\u{2003}雪\u{3000}"),
        },
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
        GeminiSignatureCandidate {
            present: false,
            text: None,
        },
    ];
    assert_eq!(gemini_signature_choice(&candidates).unwrap(), Some(1));
}

#[test]
fn system_instruction_from_request_joins_system_and_contextual_user_text() {
    let mut input = GeminiRequestContentKernelInput::new(
        GeminiRequestContentOperation::SystemInstructionFromRequest,
    );
    input.primary = Some(
        br#"{"input":[
            {"role":"system","content":"system one"},
            {"role":"user","content":"  <environment_context>synthetic</environment_context>"},
            {"role":"system","content":[{"text":"system two"},{"content":"system three"}]},
            {"role":"user","content":"actual request"}
        ]}"#,
    );

    assert_eq!(
        gemini_request_content_kernel(input).unwrap(),
        br#"{"parts":[{"text":"system one\n\nsystem two\nsystem three\n\n  <environment_context>synthetic</environment_context>"}]}"#
    );
}

#[test]
fn system_instruction_from_request_handles_empty_wrong_shapes_and_invalid_json() {
    for request in [
        br#"{"input":[{"role":"system","content":" \t"},{"role":"user","content":" \n\n "}]}"#
            .as_slice(),
        br#"{"input":"not-an-array"}"#.as_slice(),
    ] {
        let mut input = GeminiRequestContentKernelInput::new(
            GeminiRequestContentOperation::SystemInstructionFromRequest,
        );
        input.primary = Some(request);
        assert_eq!(gemini_request_content_kernel(input).unwrap(), b"null");
    }

    let mut input = GeminiRequestContentKernelInput::new(
        GeminiRequestContentOperation::SystemInstructionFromRequest,
    );
    input.primary = Some(b"{");
    assert_eq!(
        gemini_request_content_kernel(input),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}
