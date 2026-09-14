use super::*;

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};

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
    let created_at = unix_now_secs();
    #[cfg(feature = "mojo")]
    let translated = anthropic_stream_mojo_value(&value, created_at);
    #[cfg(not(feature = "mojo"))]
    let translated = anthropic_stream_rust_value(&value, created_at);
    let translated = match translated {
        Ok(Some(value)) => value,
        Ok(None) => return empty_lossless_stream(),
        Err(reason) => return rejected_stream(reason),
    };
    ProviderTransformResult::lossless(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::AnthropicMessages,
        ProviderWireFormat::OpenAiResponses,
        translated.into_bytes(),
    )
}

#[cfg(feature = "mojo")]
fn anthropic_stream_mojo_value(value: &Value, created_at: u64) -> Result<Option<String>, String> {
    let event = super::json_fragment(value)?;
    let mut input = AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::StreamEvent);
    input.content = Some(&event);
    input.created_at = created_at;
    let output = super::super::anthropic_mojo_body(input)?;
    let Some((&kind, body)) = output.split_first() else {
        return Err("Anthropic stream kernel returned an empty result".to_string());
    };
    match kind {
        0 if body.is_empty() => Ok(None),
        1 => String::from_utf8(body.to_vec())
            .map(Some)
            .map_err(|error| format!("Anthropic stream kernel returned invalid UTF-8: {error}")),
        2 => String::from_utf8(body.to_vec())
            .map(Err)
            .map_err(|error| format!("Anthropic stream kernel returned invalid UTF-8: {error}"))?,
        _ => Err("Anthropic stream kernel returned an invalid result".to_string()),
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn anthropic_stream_rust_value(value: &Value, created_at: u64) -> Result<Option<String>, String> {
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
                        "created_at": created_at,
                        "model": message.get("model").and_then(Value::as_str).unwrap_or("unknown"),
                        "output": [],
                    }
                }),
            )
        }
        "content_block_start" => {
            let Some(block) = value.get("content_block") else {
                return Err("Anthropic content_block_start requires content_block".to_string());
            };
            let index = value.get("index").cloned().unwrap_or(Value::from(0));
            match block.get("type").and_then(Value::as_str) {
                Some("text") => responses_sse_event(
                    "response.output_item.added",
                    json!({"type": "response.output_item.added", "output_index": index,
                        "item": {"type": "message", "role": "assistant", "content": []}}),
                ),
                Some("tool_use") => responses_sse_event(
                    "response.output_item.added",
                    json!({"type": "response.output_item.added", "output_index": index,
                        "item": {"type": "function_call",
                            "call_id": block.get("id").cloned().unwrap_or(Value::Null),
                            "name": block.get("name").cloned().unwrap_or(Value::Null),
                            "arguments": ""}}),
                ),
                Some("server_tool_use")
                    if block.get("name").and_then(Value::as_str) == Some("web_search") =>
                {
                    let mut item = anthropic_web_search_call(block)?;
                    item["status"] = Value::String("in_progress".to_string());
                    responses_sse_event(
                        "response.output_item.added",
                        json!({"type": "response.output_item.added", "output_index": index,
                            "item": item}),
                    )
                }
                Some("thinking") => responses_sse_event(
                    "response.output_item.added",
                    json!({"type": "response.output_item.added", "output_index": index,
                        "item": {"type": "reasoning", "summary": []}}),
                ),
                Some(_) => return Ok(None),
                None => return Err("Anthropic content block requires type".to_string()),
            }
        }
        "content_block_delta" => {
            let delta = value
                .get("delta")
                .ok_or_else(|| "Anthropic content_block_delta requires delta.type".to_string())?;
            let delta_type = delta
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| "Anthropic content_block_delta requires delta.type".to_string())?;
            let (name, kind, text) = match delta_type {
                "text_delta" => (
                    "response.output_text.delta",
                    "response.output_text.delta",
                    delta.get("text").and_then(Value::as_str).unwrap_or(""),
                ),
                "input_json_delta" => (
                    "response.function_call_arguments.delta",
                    "response.function_call_arguments.delta",
                    delta
                        .get("partial_json")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                ),
                "thinking_delta" => (
                    "response.reasoning_summary_text.delta",
                    "response.reasoning_summary_text.delta",
                    delta.get("thinking").and_then(Value::as_str).unwrap_or(""),
                ),
                _ => return Ok(None),
            };
            responses_sse_event(
                name,
                json!({"type": kind,
                    "output_index": value.get("index").cloned().unwrap_or(Value::from(0)),
                    "delta": text}),
            )
        }
        "message_stop" => {
            responses_sse_event("response.completed", json!({"type": "response.completed"}))
        }
        "error" => responses_sse_event(
            "error",
            json!({"type": "error", "error": value.get("error").cloned().unwrap_or(Value::Null)}),
        ),
        _ => return Ok(None),
    };
    Ok(Some(translated))
}

#[cfg(all(test, feature = "mojo"))]
mod tests {
    use super::*;

    #[test]
    fn mojo_stream_event_matches_rust_oracle() {
        fn normalized(value: Option<String>) -> Option<(String, Value)> {
            value.map(|event| {
                let mut lines = event.lines();
                let name = lines
                    .next()
                    .and_then(|line| line.strip_prefix("event: "))
                    .unwrap()
                    .to_string();
                let body = lines
                    .next()
                    .and_then(|line| line.strip_prefix("data: "))
                    .unwrap();
                (name, serde_json::from_str(body).unwrap())
            })
        }
        let cases = [
            json!({"type":"message_start","message":{"id":"resp_\u{1f980}","model":"claude"}}),
            json!({"type":"content_block_start","index":2,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_start","index":3,"content_block":{"type":"tool_use","id":null,"name":"x"}}),
            json!({"type":"content_block_start","content_block":{"type":"server_tool_use","id":"srv","name":"web_search","input":{"queries":["a",null,"\u{1f980}"]}}}),
            json!({"type":"content_block_start","content_block":{"type":"thinking"}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"x\n\u{1f980}"}}),
            json!({"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{\"x\":"}}),
            json!({"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":""}}),
            json!({"type":"message_stop"}),
            json!({"type":"error","error":{"type":"overloaded_error","message":null}}),
            json!({"type":"ping"}),
        ];
        for value in cases {
            assert_eq!(
                normalized(anthropic_stream_mojo_value(&value, 123).unwrap()),
                normalized(anthropic_stream_rust_value(&value, 123).unwrap()),
                "{value}"
            );
        }
        for value in [
            json!({"type":"content_block_start"}),
            json!({"type":"content_block_start","content_block":{}}),
            json!({"type":"content_block_delta"}),
        ] {
            assert_eq!(
                anthropic_stream_mojo_value(&value, 123).unwrap_err(),
                anthropic_stream_rust_value(&value, 123).unwrap_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn mojo_stream_event_enforces_kernel_size_boundary() {
        const MAX_BYTES: usize = 4 * 1024 * 1024;
        let prefix = r#"{"padding":""#;
        let suffix = r#"","type":"ping"}"#;
        let exact = format!(
            "{prefix}{}{suffix}",
            "x".repeat(MAX_BYTES - prefix.len() - suffix.len())
        );
        let mut input =
            AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::StreamEvent);
        input.content = Some(&exact);
        assert_eq!(
            super::super::super::anthropic_mojo_body(input).unwrap(),
            [0]
        );

        let oversized = format!("{exact} ");
        input.content = Some(&oversized);
        assert!(super::super::super::anthropic_mojo_body(input).is_err());
    }
}
