use super::*;

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};

#[cfg(feature = "mojo")]
fn anthropic_stream_mojo_event(input: AnthropicRequestKernelInput<'_>) -> Result<String, String> {
    String::from_utf8(super::super::anthropic_mojo_body(input)?)
        .map_err(|error| format!("Anthropic stream kernel returned invalid UTF-8: {error}"))
}

#[cfg(not(feature = "mojo"))]
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

#[cfg(feature = "mojo")]
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
    let translated = match anthropic_stream_mojo_value(&value) {
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
fn anthropic_stream_mojo_value(value: &Value) -> Result<Option<String>, String> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match event_type {
        "message_start" => {
            let message = value.get("message").cloned().unwrap_or(Value::Null);
            let id = super::json_fragment(&Value::String(
                message
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("resp_anthropic")
                    .to_string(),
            ))?;
            let model = super::json_fragment(&Value::String(
                message
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
            ))?;
            let mut input = AnthropicRequestKernelInput::new(
                AnthropicRequestKernelOperation::StreamMessageStart,
            );
            input.id = Some(&id);
            input.model = Some(&model);
            input.created_at = super::unix_now_secs();
            Ok(Some(anthropic_stream_mojo_event(input)?))
        }
        "content_block_start" => {
            let Some(block) = value.get("content_block") else {
                return Err("Anthropic content_block_start requires content_block".to_string());
            };
            let index =
                super::json_fragment(&value.get("index").cloned().unwrap_or(Value::from(0)))?;
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    let mut input = AnthropicRequestKernelInput::new(
                        AnthropicRequestKernelOperation::StreamTextStart,
                    );
                    input.index = Some(&index);
                    Ok(Some(anthropic_stream_mojo_event(input)?))
                }
                Some("tool_use") => {
                    let id =
                        super::json_fragment(&block.get("id").cloned().unwrap_or(Value::Null))?;
                    let name =
                        super::json_fragment(&block.get("name").cloned().unwrap_or(Value::Null))?;
                    let mut input = AnthropicRequestKernelInput::new(
                        AnthropicRequestKernelOperation::StreamToolStart,
                    );
                    input.index = Some(&index);
                    input.id = Some(&id);
                    input.name = Some(&name);
                    Ok(Some(anthropic_stream_mojo_event(input)?))
                }
                Some("server_tool_use")
                    if block.get("name").and_then(Value::as_str) == Some("web_search") =>
                {
                    let Some(id) = block.get("id").and_then(Value::as_str) else {
                        return Err("Anthropic server_tool_use block must contain id".to_string());
                    };
                    let id = super::json_fragment(&Value::String(id.to_string()))?;
                    let queries =
                        super::web_search::anthropic_web_search_queries(block.get("input"));
                    let queries = super::json_fragment(&Value::Array(queries))?;
                    let mut input = AnthropicRequestKernelInput::new(
                        AnthropicRequestKernelOperation::StreamWebSearchStart,
                    );
                    input.index = Some(&index);
                    input.id = Some(&id);
                    input.queries = Some(&queries);
                    input.choice_kind = 1;
                    Ok(Some(anthropic_stream_mojo_event(input)?))
                }
                Some("web_search_tool_result") | Some(_) => Ok(None),
                None => Err("Anthropic content block requires type".to_string()),
            }
        }
        "content_block_delta" => {
            let Some(delta) = value.get("delta") else {
                return Err("Anthropic content_block_delta requires delta.type".to_string());
            };
            let Some(delta_type) = delta.get("type").and_then(Value::as_str) else {
                return Err("Anthropic content_block_delta requires delta.type".to_string());
            };
            let index =
                super::json_fragment(&value.get("index").cloned().unwrap_or(Value::from(0)))?;
            let (operation, delta) = match delta_type {
                "text_delta" => (
                    AnthropicRequestKernelOperation::StreamTextDelta,
                    delta.get("text").and_then(Value::as_str).unwrap_or(""),
                ),
                "input_json_delta" => (
                    AnthropicRequestKernelOperation::StreamArgumentsDelta,
                    delta
                        .get("partial_json")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                ),
                "thinking_delta" => (
                    AnthropicRequestKernelOperation::StreamThinkingDelta,
                    delta.get("thinking").and_then(Value::as_str).unwrap_or(""),
                ),
                _ => return Ok(None),
            };
            let delta = super::json_fragment(&Value::String(delta.to_string()))?;
            let mut input = AnthropicRequestKernelInput::new(operation);
            input.index = Some(&index);
            input.delta = Some(&delta);
            Ok(Some(anthropic_stream_mojo_event(input)?))
        }
        "message_stop" => {
            let input =
                AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::StreamCompleted);
            Ok(Some(anthropic_stream_mojo_event(input)?))
        }
        "error" => {
            let error = super::json_fragment(&value.get("error").cloned().unwrap_or(Value::Null))?;
            let mut input =
                AnthropicRequestKernelInput::new(AnthropicRequestKernelOperation::StreamError);
            input.error = Some(&error);
            Ok(Some(anthropic_stream_mojo_event(input)?))
        }
        "ping" | "content_block_stop" | "message_delta" => Ok(None),
        _ => Ok(None),
    }
}
