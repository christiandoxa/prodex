use super::*;

#[test]
fn deepseek_response_event_builders_keep_timestamps_and_payloads() {
    let response = serde_json::json!({
        "id": "resp_東京",
        "output": [{"type": "message", "text": "hello"}],
        "metadata": {"app": {"key": "value"}},
    });
    assert_eq!(
        deepseek_provider_core_response_completed_event(7, 123, &response),
        serde_json::json!({
            "type": "response.completed",
            "sequence_number": 7,
            "created_at": 123,
            "response": response,
        })
    );
    assert_eq!(
        deepseek_provider_core_response_completed_event(8, 456, &serde_json::json!("response")),
        serde_json::json!({
            "type": "response.completed",
            "sequence_number": 8,
            "created_at": 456,
            "response": "response",
        })
    );
    assert_eq!(
        deepseek_provider_core_response_created_event(9, 789, "resp_東京"),
        serde_json::json!({
            "type": "response.created",
            "sequence_number": 9,
            "created_at": 789,
            "response": {"id": "resp_東京"},
        })
    );
}

#[test]
fn deepseek_output_item_and_delta_builders_keep_payloads() {
    let text = "hello 東京\n🙂";
    assert_eq!(
        deepseek_provider_core_stream_output_text_item(text),
        serde_json::json!({
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": text}],
        })
    );

    let item = serde_json::json!(["東京", null, 7]);
    assert_eq!(
        deepseek_provider_core_output_item_added_event(10, &item),
        serde_json::json!({
            "type": "response.output_item.added",
            "sequence_number": 10,
            "item": item,
        })
    );
    assert_eq!(
        deepseek_provider_core_output_item_done_event(11, &item),
        serde_json::json!({
            "type": "response.output_item.done",
            "sequence_number": 11,
            "item": item,
        })
    );

    let arguments = "{\"path\":\"東京\"}";
    assert_eq!(
        deepseek_provider_core_stream_function_call_arguments_delta_source("call_東京", arguments),
        serde_json::json!({
            "choices": [{"delta": {"tool_calls": [{
                "id": "call_東京",
                "function": {"arguments": arguments},
            }]}}]
        })
    );
    assert_eq!(
        deepseek_provider_core_stream_text_delta_source(text),
        serde_json::json!({"choices": [{"delta": {"content": text}}]})
    );
    assert_eq!(
        deepseek_provider_core_function_call_arguments_delta_event(12, "call_東京", arguments),
        serde_json::json!({
            "type": "response.function_call_arguments.delta",
            "sequence_number": 12,
            "call_id": "call_東京",
            "delta": arguments,
        })
    );
    assert_eq!(
        deepseek_provider_core_output_text_delta_event(13, 456, "resp_東京", text),
        serde_json::json!({
            "type": "response.output_text.delta",
            "sequence_number": 13,
            "created_at": 456,
            "response_id": "resp_東京",
            "delta": text,
        })
    );
}

#[test]
fn deepseek_tool_call_item_builders_keep_tool_kinds_and_namespaces() {
    assert_eq!(
        deepseek_provider_core_stream_tool_call_added_item("call_東京", "mcp__org__tool"),
        Some(serde_json::json!({
            "type": "function_call",
            "call_id": "call_東京",
            "name": "tool",
            "namespace": "mcp__org",
        }))
    );
    assert_eq!(
        deepseek_provider_core_stream_tool_call_added_item("call_2", "apply_patch"),
        Some(serde_json::json!({
            "type": "custom_tool_call",
            "call_id": "call_2",
            "name": "apply_patch",
            "input": "",
        }))
    );
    assert_eq!(
        deepseek_provider_core_stream_tool_call_added_item("call_3", "tool_search"),
        None
    );

    let arguments = "{\"city\":\"東京\"}";
    assert_eq!(
        deepseek_provider_core_stream_tool_call_item(
            "call_4",
            "mcp__org__tool",
            arguments,
            Some("sig_1"),
        ),
        serde_json::json!({
            "type": "function_call",
            "call_id": "call_4",
            "name": "tool",
            "namespace": "mcp__org",
            "arguments": arguments,
            "gemini_thought_signature": "sig_1",
        })
    );
    assert_eq!(
        deepseek_provider_core_stream_tool_call_item(
            "call_5",
            "tool_search",
            "{\"query\":\"東京\"}",
            None,
        ),
        serde_json::json!({
            "type": "tool_search_call",
            "call_id": "call_5",
            "execution": "client",
            "arguments": {"query": "東京"},
        })
    );
    assert_eq!(
        deepseek_provider_core_stream_tool_call_item("call_6", "tool_search", "{bad", None),
        serde_json::json!({
            "type": "tool_search_call",
            "call_id": "call_6",
            "execution": "client",
            "arguments": {},
        })
    );
    assert_eq!(
        deepseek_provider_core_stream_tool_call_item(
            "call_7",
            "apply_patch",
            "{\"path\":\"note.txt\",\"content\":\"hello\"}",
            None,
        ),
        serde_json::json!({
            "type": "custom_tool_call",
            "call_id": "call_7",
            "name": "apply_patch",
            "input": "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch",
        })
    );
}
