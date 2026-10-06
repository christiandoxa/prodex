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

#[test]
fn gemini_stream_shaping_preserves_presence_and_whitespace() {
    let chunk = json!({
        "responseId": null, "id": "ignored", "modelVersion": 7, "model": "ignored",
        "usageMetadata": null,
        "candidates": [{"finishReason": "\u{2003}"}, {"finishReason": "STOP"}]
    });
    let metadata = gemini_provider_core_stream_chunk_metadata("resp_gemini_1", &chunk);
    assert_eq!(metadata.response_id, None);
    assert_eq!(metadata.model, None);
    assert_eq!(metadata.finish_reason, None);
    assert_eq!(
        metadata.response_metadata,
        Some(json!({"gemini":{"finishReason":"\u{2003}"}}))
    );
    assert_eq!(
        metadata.usage,
        Some(json!({
            "input_tokens": 0, "input_tokens_details": {"cached_tokens":0,"tool_tokens":0},
            "output_tokens":0,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":0
        }))
    );
    let chunk = json!({"id":"", "model":"", "candidates":[{"finishReason":" STOP "}]});
    let metadata = gemini_provider_core_stream_chunk_metadata("resp_gemini_1", &chunk);
    assert_eq!(metadata.response_id, Some(String::new()));
    assert_eq!(metadata.model, Some(String::new()));
    assert_eq!(metadata.finish_reason, Some(" STOP ".into()));
    assert_eq!(
        gemini_provider_core_stream_chunk_metadata("upstream", &chunk).response_id,
        None
    );
    for id in [json!(null), json!(7), json!(""), json!("\u{2003}")] {
        assert_eq!(
            gemini_provider_core_stream_function_call_delta(&json!({"id":id})).explicit_call_id,
            None
        );
    }
    assert_eq!(
        gemini_provider_core_stream_function_call_delta(&json!({"id":" id "})).explicit_call_id,
        Some(" id ".into())
    );
    let calls = ["", "\u{2003}", " id ", " id "]
        .map(|id| gemini_provider_core_stream_tool_call(1, 0, Some(id), Some("shell"), "{}", None));
    // StreamToolCall supplies fallbacks for blank IDs; test filtering with explicit stored IDs.
    let calls = calls
        .into_iter()
        .zip(["", "\u{2003}", " id ", " id "])
        .map(|(mut call, id)| {
            call.call_id = id.into();
            call
        })
        .collect::<Vec<_>>();
    assert_eq!(
        gemini_provider_core_stream_tool_call_ids(&calls),
        vec![" id ", " id "]
    );
    for name in ["apply_patch", "tool_search"] {
        assert_eq!(
            gemini_provider_core_stream_tool_call_added_item("call", name, Some("sig")),
            None
        );
    }
    assert_eq!(
        gemini_provider_core_stream_tool_call_added_item("call", "ns--shell", Some("sig")),
        Some(json!({
            "type":"function_call", "call_id":"call", "namespace":"ns", "name":"shell", "gemini_thought_signature":"sig"
        }))
    );
}
