use super::*;

pub(in super::super) fn translate_anthropic_stream_event_to_responses(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return unsupported(
            input.endpoint,
            "native Messages translation only supports responses",
        );
    }
    let event = String::from_utf8_lossy(&input.body);
    let Some(data) = event.lines().find_map(|line| line.strip_prefix("data: ")) else {
        return unsupported(
            ProviderEndpoint::Responses,
            "Anthropic SSE event must contain data: <json> framing",
        );
    };
    let value: Value = match serde_json::from_str(data) {
        Ok(value) => value,
        Err(error) => {
            return rejected_stream(format!("failed to parse Anthropic SSE JSON: {error}"));
        }
    };
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let translated = match event_type {
        "message_start" => {
            let message = value.get("message").cloned().unwrap_or(Value::Null);
            responses_sse_event(
                "response.created",
                json!({
                    "type": "response.created",
                    "response": {
                        "id": message.get("id").and_then(Value::as_str).unwrap_or("resp_anthropic"),
                        "object": "response",
                        "created_at": unix_now_secs(),
                        "model": message.get("model").and_then(Value::as_str).unwrap_or("unknown"),
                        "output": [],
                    }
                }),
            )
        }
        "content_block_start" => {
            let Some(block) = value.get("content_block") else {
                return rejected_stream("Anthropic content_block_start requires content_block");
            };
            let index = value.get("index").cloned().unwrap_or(Value::from(0));
            match block.get("type").and_then(Value::as_str) {
                Some("text") => responses_sse_event(
                    "response.output_item.added",
                    json!({
                        "type": "response.output_item.added",
                        "output_index": index,
                        "item": {"type": "message", "role": "assistant", "content": []},
                    }),
                ),
                Some("tool_use") => responses_sse_event(
                    "response.output_item.added",
                    json!({
                        "type": "response.output_item.added",
                        "output_index": index,
                        "item": {
                            "type": "function_call",
                            "call_id": block.get("id").cloned().unwrap_or(Value::Null),
                            "name": block.get("name").cloned().unwrap_or(Value::Null),
                            "arguments": "",
                        },
                    }),
                ),
                Some("server_tool_use")
                    if block.get("name").and_then(Value::as_str) == Some("web_search") =>
                {
                    let mut item = match anthropic_web_search_call(block) {
                        Ok(item) => item,
                        Err(reason) => return rejected_stream(reason),
                    };
                    item["status"] = Value::String("in_progress".to_string());
                    responses_sse_event(
                        "response.output_item.added",
                        json!({
                            "type": "response.output_item.added",
                            "output_index": index,
                            "item": item,
                        }),
                    )
                }
                Some("web_search_tool_result") => return empty_lossless_stream(),
                Some("thinking") => responses_sse_event(
                    "response.output_item.added",
                    json!({
                        "type": "response.output_item.added",
                        "output_index": index,
                        "item": {"type": "reasoning", "summary": []},
                    }),
                ),
                Some(_) => return empty_lossless_stream(),
                None => return rejected_stream("Anthropic content block requires type"),
            }
        }
        "content_block_delta" => match value
            .get("delta")
            .and_then(|delta| delta.get("type"))
            .and_then(Value::as_str)
        {
            Some("text_delta") => responses_sse_event(
                "response.output_text.delta",
                json!({
                    "type": "response.output_text.delta",
                    "output_index": value.get("index").cloned().unwrap_or(Value::from(0)),
                    "delta": value.pointer("/delta/text").and_then(Value::as_str).unwrap_or(""),
                }),
            ),
            Some("input_json_delta") => responses_sse_event(
                "response.function_call_arguments.delta",
                json!({
                    "type": "response.function_call_arguments.delta",
                    "output_index": value.get("index").cloned().unwrap_or(Value::from(0)),
                    "delta": value.pointer("/delta/partial_json").and_then(Value::as_str).unwrap_or(""),
                }),
            ),
            Some("thinking_delta") => responses_sse_event(
                "response.reasoning_summary_text.delta",
                json!({
                    "type": "response.reasoning_summary_text.delta",
                    "output_index": value.get("index").cloned().unwrap_or(Value::from(0)),
                    "delta": value.pointer("/delta/thinking").and_then(Value::as_str).unwrap_or(""),
                }),
            ),
            Some(_) => return empty_lossless_stream(),
            None => return rejected_stream("Anthropic content_block_delta requires delta.type"),
        },
        "message_stop" => responses_sse_event(
            "response.completed",
            json!({
                "type": "response.completed"
            }),
        ),
        "error" => responses_sse_event(
            "error",
            json!({
                "type": "error",
                "error": value.get("error").cloned().unwrap_or(Value::Null),
            }),
        ),
        "ping" | "content_block_stop" | "message_delta" => return empty_lossless_stream(),
        _ => return empty_lossless_stream(),
    };
    ProviderTransformResult::lossless(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::AnthropicMessages,
        ProviderWireFormat::OpenAiResponses,
        translated.into_bytes(),
    )
}
