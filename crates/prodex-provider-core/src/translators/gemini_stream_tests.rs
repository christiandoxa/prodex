use super::*;

#[test]
fn fixed_values_cover_precedence_and_unicode() {
    let translator = GeminiTranslator;

    let function_call = translator.transform_stream_event(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        r#"data: {"candidates":[{"content":{"parts":[{"text":"ignored","thought":true,"functionCall":{"id":"call_1","args":{"a":1,"z":"🌍"}}},{"text":"later"}]}},{"content":{"parts":[{"text":"also later"}]}}]}"#.to_string() + "\n\n",
    ));
    let (event, value) = transformed_gemini_sse_value(
        function_call
            .body
            .as_deref()
            .expect("function-call event body"),
    );
    assert_eq!(event, "response.function_call_arguments.delta");
    assert_eq!(
        value,
        json!({
            "type": "response.function_call_arguments.delta",
            "call_id": "call_1",
            "delta": r#"{"a":1,"z":"🌍"}"#,
        })
    );

    let sparse_function_call = translator.transform_stream_event(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        br#"data: {"candidates":[{"content":{"parts":[{"text":"ignored","functionCall":{"id":7}}]}}]}

"#,
    ));
    let (event, value) = transformed_gemini_sse_value(
        sparse_function_call
            .body
            .as_deref()
            .expect("sparse function-call event body"),
    );
    assert_eq!(event, "response.function_call_arguments.delta");
    assert_eq!(
        value,
        json!({
            "type": "response.function_call_arguments.delta",
            "delta": "{}",
        })
    );

    let unicode_text = translator.transform_stream_event(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"héllo 🌍\",\"thought\":\"true\"}]}}]}\n\n"
            .as_bytes()
            .to_vec(),
    ));
    let (event, value) = transformed_gemini_sse_value(
        unicode_text
            .body
            .as_deref()
            .expect("Unicode text event body"),
    );
    assert_eq!(event, "response.output_text.delta");
    assert_eq!(
        value,
        json!({"type": "response.output_text.delta", "delta": "héllo 🌍"})
    );

    let completion = translator.transform_stream_event(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        b"data: [DONE]\n\n",
    ));
    assert!(matches!(
        completion.status(),
        crate::translator::TransformStatus::Rejected { .. }
    ));
}
