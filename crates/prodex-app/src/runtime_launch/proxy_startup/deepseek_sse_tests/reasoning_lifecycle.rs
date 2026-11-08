use super::*;
use serde_json::json;

#[test]
fn local_reasoning_lifecycle_opens_reasoning_before_deltas_and_closes_before_answer() {
    let mut state = RuntimeDeepSeekSseState::new_with_provider(
        super::super::super::provider_bridge::RuntimeProviderBridgeKind::OpenAiResponses,
        71,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut wire = state.observe_chat_chunk(&json!({"id":"chatcmpl_fixture","model":"offrail-27b",
        "choices":[{"delta":{"reasoning_content":"Check the local fixture."}}]}));
    wire.extend(state.observe_chat_chunk(
        &json!({"choices":[{"delta":{"content":"Result."},"finish_reason":"stop"}]}),
    ));
    wire.extend(state.complete_event());
    let events = sse_values(&wire.join(""));
    let opened = events
        .iter()
        .position(|e| e["type"] == "response.output_item.added" && e["item"]["type"] == "reasoning")
        .expect("Codex requires an active reasoning item before any summary delta");
    let delta = events
        .iter()
        .position(|e| e["type"] == "response.reasoning_summary_text.delta")
        .unwrap();
    let closed = events
        .iter()
        .position(|e| e["type"] == "response.output_item.done" && e["item"]["type"] == "reasoning")
        .unwrap();
    let answer = events
        .iter()
        .position(|e| e["type"] == "response.output_item.added" && e["item"]["type"] == "message")
        .unwrap();
    assert!(opened < delta && delta < closed && closed < answer);
    assert_eq!(events[delta]["item_id"], events[opened]["item"]["id"]);
    assert_eq!(
        events[delta]["output_index"],
        events[opened]["output_index"]
    );
    assert_ne!(
        events[opened]["output_index"],
        events[answer]["output_index"]
    );
    let output = events.last().unwrap()["response"]["output"]
        .as_array()
        .unwrap();
    assert_eq!(output[0]["type"], "reasoning");
    assert_eq!(output[0]["summary"][0]["text"], "Check the local fixture.");
    assert_eq!(output[1]["content"][0]["text"], "Result.");
    for pair in events.windows(2) {
        assert!(
            pair[0]["sequence_number"].as_u64().unwrap()
                < pair[1]["sequence_number"].as_u64().unwrap()
        );
    }
}

#[test]
fn local_reasoning_is_visible_to_prodex_log_and_tool_items_have_distinct_positions() {
    let mut state = RuntimeDeepSeekSseState::new(72, Vec::new(), None, conversation_store());
    let mut wire=state.observe_chat_chunk(&json!({"id":"chatcmpl_tools","choices":[{"delta":{"reasoning_content":"Inspect the fixture safely. 雪"}}]}));
    wire.extend(state.observe_chat_chunk(&json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_fixture","function":{"name":"read_file","arguments":"{}"}}]},"finish_reason":"tool_calls"}]})));
    wire.extend(state.complete_event());
    let events = sse_values(&wire.join(""));
    let closed = events
        .iter()
        .position(|e| e["type"] == "response.output_item.done" && e["item"]["type"] == "reasoning")
        .unwrap();
    let tool = events
        .iter()
        .position(|e| {
            e["type"] == "response.output_item.added" && e["item"]["type"] == "function_call"
        })
        .unwrap();
    assert!(closed < tool);
    assert_ne!(events[closed]["output_index"], events[tool]["output_index"]);
    let log_line=json!({"timestamp":"2026-01-01T00:00:00Z","type":"response_item","payload":events[closed]["item"]}).to_string();
    let log_events = crate::transcript_events_from_session_line(&log_line);
    assert!(
        log_events
            .iter()
            .any(|event| event.text.contains("Inspect the fixture safely. 雪")),
        "{log_events:?}"
    );
    let output = events.last().unwrap()["response"]["output"]
        .as_array()
        .unwrap();
    assert_eq!(output.len(), 2);
    assert_eq!(output[0]["type"], "reasoning");
    assert_eq!(output[1]["call_id"], "call_fixture");
}
