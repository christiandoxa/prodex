//! Gemini schema sanitization for request tools and response formats.

use serde_json::Value;

fn sanitize_with_mojo(
    schema: &Value,
    operation: prodex_mojo_core::provider_constraints::GeminiRequestContentOperation,
) -> Value {
    let input = serde_json::to_vec(schema).expect("Gemini schema serializes");
    super::super::request_contents::gemini_request_content_mojo_value(
        operation,
        Some(&input),
        None,
        None,
        None,
        0,
    )
}

pub(crate) fn sanitize_schema(schema: &Value) -> Value {
    sanitize_with_mojo(
        schema,
        prodex_mojo_core::provider_constraints::GeminiRequestContentOperation::SanitizeSchema,
    )
}

pub(crate) fn sanitize_function_schema(schema: &Value) -> Value {
    sanitize_with_mojo(
        schema,
        prodex_mojo_core::provider_constraints::GeminiRequestContentOperation::SanitizeFunctionSchema,
    )
}
