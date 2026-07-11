use super::*;
use std::cell::Cell;

fn test_messages_request() -> RuntimeAnthropicMessagesRequest {
    let mut server_tools = RuntimeAnthropicServerTools::default();
    server_tools.register("web_search", "web_search");
    RuntimeAnthropicMessagesRequest {
        translated_request: RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: Vec::new(),
            body: b"{}".to_vec(),
        },
        requested_model: "gpt-5.4".to_string(),
        stream: false,
        want_thinking: false,
        server_tools,
        carried_web_search_requests: 0,
        carried_web_fetch_requests: 0,
        carried_code_execution_requests: 0,
        carried_tool_search_requests: 0,
    }
}

#[test]
fn server_tool_followup_translation_passes_upstream_errors_without_followup() {
    let observed = Cell::new(false);
    let result =
        translate_runtime_buffered_responses_reply_to_anthropic_with_server_tool_followups(
            RuntimeBufferedResponseParts {
                status: 429,
                headers: vec![("content-type".to_string(), b"text/plain".to_vec())],
                body: b"too many requests".to_vec(),
            },
            &test_messages_request(),
            2,
            |observation| {
                observed.set(true);
                assert_eq!(observation.status, 429);
                assert_eq!(observation.content_type, Some("text/plain"));
                assert_eq!(observation.followup_attempt, 0);
            },
            |_| panic!("upstream errors should not trigger server-tool followup"),
        )
        .unwrap();

    assert!(observed.get());
    match result {
        RuntimeResponsesReply::Buffered(parts) => assert_eq!(parts.status, 429),
        RuntimeResponsesReply::Streaming(_) => panic!("translation should be buffered"),
    }
}
