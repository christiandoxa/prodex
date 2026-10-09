#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::rich::{
    AnthropicRequestKernelInput, AnthropicRequestKernelOperation, anthropic_request_kernel,
};

fn run(input: AnthropicRequestKernelInput<'_>) -> String {
    String::from_utf8(anthropic_request_kernel(input).expect("Anthropic kernel"))
        .expect("Anthropic UTF-8 output")
}

#[test]
fn web_search_result_sources_filter_malformed_fields_and_keep_order() {
    let content = r#"{"content":[{"url":"https://example.com/one","title":"🦀"},{"url":false,"title":"ignored"},{"url":"https://example.com/two","title":7},{"url":""},"ignored"]}"#;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::WebSearchResult);
    input.content = Some(content);

    assert_eq!(
        run(input),
        r#"[{"type":"url","url":"https://example.com/one","title":"🦀"},{"type":"url","url":"https://example.com/two"},{"type":"url","url":""}]"#
    );
}

#[test]
fn web_search_result_updates_only_the_last_matching_call() {
    let blocks = r#"[{"type":"web_search_call","id":"duplicate","status":"completed","action":{"type":"search","queries":["first"],"sources":[]}},{"type":"web_search_call","id":"duplicate","status":"completed","action":{"type":"search","queries":["last"],"sources":[]}}]"#;
    let content = r#"{"tool_use_id":"duplicate","content":[{"url":"https://example.com/result","title":"Result"}]}"#;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::WebSearchResult);
    input.choice_kind = 1;
    input.blocks = Some(blocks);
    input.content = Some(content);

    assert_eq!(
        run(input),
        r#"[{"type":"web_search_call","id":"duplicate","status":"completed","action":{"type":"search","queries":["first"],"sources":[]}},{"type":"web_search_call","id":"duplicate","status":"completed","action":{"type":"search","queries":["last"],"sources":[{"type":"url","url":"https://example.com/result","title":"Result"}]}}]"#
    );
}

#[test]
fn web_search_call_accepts_incomplete_stream_input_without_recomputing_in_rust() {
    let id = r#""srv_1""#;
    let input_json = r#"{"query":"unfinished""#;
    let sources = "[]";
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::WebSearchCall);
    input.stream = true;
    input.choice_kind = 1;
    input.id = Some(id);
    input.input = Some(input_json);
    input.blocks = Some(sources);

    assert_eq!(
        run(input),
        r#"{"type":"web_search_call","id":"srv_1","status":"in_progress","action":{"type":"search","queries":[],"sources":[]}}"#
    );
}

#[test]
fn web_search_call_stream_preserves_query_array_values() {
    let id = r#""srv_1""#;
    let input_json = r#"{"queries":["query",7,{"kind":"legacy"}]}"#;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::WebSearchCall);
    input.stream = true;
    input.choice_kind = 1;
    input.id = Some(id);
    input.input = Some(input_json);
    input.blocks = Some("[]");

    assert_eq!(
        run(input),
        r#"{"type":"web_search_call","id":"srv_1","status":"in_progress","action":{"type":"search","queries":["query",7,{"kind":"legacy"}],"sources":[]}}"#
    );
}

#[test]
fn response_envelope_defaults_missing_and_non_string_identifiers() {
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ResponseEnvelope);
    input.model = Some("1");
    input.blocks = Some("[]");
    input.created_at = 123;

    assert_eq!(
        run(input),
        r#"{"id":"resp_anthropic","object":"response","created_at":123,"model":"unknown","output":[]}"#
    );
}

#[test]
fn response_envelope_raw_source_preserves_presence_and_overflow_policy() {
    let source = r#"{"id":null,"model":1,"usage":{"input_tokens":18446744073709551615,"output_tokens":1,"server_tool_use":{"web_search_requests":0}},"stop_reason":null}"#;
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ResponseEnvelope);
    input.choice_kind = -1;
    input.content = Some(source);
    input.blocks = Some("[]");
    input.created_at = 123;

    assert_eq!(
        run(input),
        r#"{"id":"resp_anthropic","object":"response","created_at":123,"model":"unknown","output":[],"usage":{"input_tokens":18446744073709551615,"output_tokens":1,"total_tokens":18446744073709551615},"tool_usage":{"web_search":{"num_requests":0}},"metadata":{"anthropic":{"stop_reason":null}}}"#
    );

    let source = r#"{"usage":{"input_tokens":18446744073709551616,"output_tokens":true}}"#;
    input.content = Some(source);
    assert_eq!(
        run(input),
        r#"{"id":"resp_anthropic","object":"response","created_at":123,"model":"unknown","output":[],"usage":{"input_tokens":0,"output_tokens":0,"total_tokens":0}}"#
    );
}

#[test]
fn response_envelope_raw_source_fails_closed_for_malformed_and_oversized_input() {
    let mut input =
        AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::ResponseEnvelope);
    input.choice_kind = -1;
    input.content = Some(r#"{"usage":}"#);
    input.blocks = Some("[]");
    assert!(anthropic_request_kernel(input).is_err());

    let oversized = format!(r#"{{"padding":"{}"}}"#, "x".repeat(4 * 1024 * 1024));
    input.content = Some(&oversized);
    assert!(anthropic_request_kernel(input).is_err());
}

#[test]
fn stream_event_kernel_preserves_finish_and_error_shapes() {
    let mut input = AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::StreamEvent);
    input.content = Some(r#"{"type":"content_block_delta","delta":{"type":"text_delta"}}"#);
    let output = anthropic_request_kernel(input).expect("stream event");
    assert_eq!(output[0], 1);
    assert_eq!(
        String::from_utf8(output[1..].to_vec()).expect("stream event UTF-8"),
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"\"}\n\n"
    );

    input.content = Some(r#"{"type":"error"}"#);
    let output = anthropic_request_kernel(input).expect("stream error");
    assert_eq!(output[0], 1);
    assert_eq!(
        String::from_utf8(output[1..].to_vec()).expect("stream error UTF-8"),
        "event: error\ndata: {\"type\":\"error\",\"error\":null}\n\n"
    );
}
