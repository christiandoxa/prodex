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
