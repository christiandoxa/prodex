use super::*;

use prodex_mojo_core::rich::{AnthropicRequestKernelInput, AnthropicRequestKernelOperation};

pub(in super::super) fn translate_anthropic_stream_event_to_responses(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return unsupported_stream(
            input.endpoint,
            "native Messages translation only supports responses",
        );
    }
    let event = String::from_utf8_lossy(&input.body);
    let Some(data) = event.lines().find_map(|line| line.strip_prefix("data: ")) else {
        return unsupported_stream(
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
    let translated = anthropic_stream_mojo_value(&value, created_at);
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

fn unsupported_stream(
    endpoint: ProviderEndpoint,
    reason: impl Into<String>,
) -> ProviderTransformResult {
    ProviderTransformResult::unsupported(
        ProviderId::Anthropic,
        endpoint,
        ProviderWireFormat::AnthropicMessages,
        ProviderWireFormat::OpenAiResponses,
        reason,
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;

    type ExpectedEvent = Result<Option<(String, Value)>, String>;

    fn normalized(value: Result<Option<String>, String>) -> ExpectedEvent {
        let Some(event) = value? else {
            return Ok(None);
        };
        let mut lines = event.lines();
        let name = lines
            .next()
            .and_then(|line| line.strip_prefix("event: "))
            .ok_or_else(|| "missing event line".to_string())?;
        let data = lines
            .next()
            .and_then(|line| line.strip_prefix("data: "))
            .ok_or_else(|| "missing data line".to_string())?;
        Ok(Some((
            name.to_string(),
            serde_json::from_str(data).map_err(|error| error.to_string())?,
        )))
    }

    fn stream_fixtures() -> Vec<(Value, ExpectedEvent)> {
        vec![
            (
                json!({"type":"message_start","message":{"id":"resp_🦀","model":"claude"}}),
                Ok(Some((
                    "response.created".to_string(),
                    json!({
                        "type":"response.created","response":{"id":"resp_🦀","object":"response","created_at":123,"model":"claude","output":[]}
                    }),
                ))),
            ),
            (
                json!({"type":"message_start","message":{}}),
                Ok(Some((
                    "response.created".to_string(),
                    json!({
                        "type":"response.created","response":{"id":"resp_anthropic","object":"response","created_at":123,"model":"unknown","output":[]}
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_start","index":4,"content_block":{"type":"text"}}),
                Ok(Some((
                    "response.output_item.added".to_string(),
                    json!({
                        "type":"response.output_item.added","output_index":4,"item":{"type":"message","role":"assistant","content":[]}
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_start","content_block":{"type":"text"}}),
                Ok(Some((
                    "response.output_item.added".to_string(),
                    json!({
                        "type":"response.output_item.added","output_index":0,"item":{"type":"message","role":"assistant","content":[]}
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_start","index":3,"content_block":{"type":"tool_use","id":"call_1","name":"read_file"}}),
                Ok(Some((
                    "response.output_item.added".to_string(),
                    json!({
                        "type":"response.output_item.added","output_index":3,"item":{"type":"function_call","call_id":"call_1","name":"read_file","arguments":""}
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_start","content_block":{"type":"tool_use","id":null}}),
                Ok(Some((
                    "response.output_item.added".to_string(),
                    json!({
                        "type":"response.output_item.added","output_index":0,"item":{"type":"function_call","call_id":null,"name":null,"arguments":""}
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_start","index":5,"content_block":{"type":"server_tool_use","id":"srv_1","name":"web_search","input":{"query":"release 🦀"}}}),
                Ok(Some((
                    "response.output_item.added".to_string(),
                    json!({
                        "type":"response.output_item.added","output_index":5,"item":{"type":"web_search_call","id":"srv_1","status":"in_progress","action":{"type":"search","queries":["release 🦀"],"sources":[]}}
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_start","content_block":{"type":"server_tool_use","id":"srv_2","name":"web_search","input":{"queries":["one",null,3,"🦀"]}}}),
                Ok(Some((
                    "response.output_item.added".to_string(),
                    json!({
                        "type":"response.output_item.added","output_index":0,"item":{"type":"web_search_call","id":"srv_2","status":"in_progress","action":{"type":"search","queries":["one","🦀"],"sources":[]}}
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_start","content_block":{"type":"server_tool_use","id":"srv_3","name":"browser_search"}}),
                Ok(None),
            ),
            (
                json!({"type":"content_block_start","content_block":{"type":"thinking"}}),
                Ok(Some((
                    "response.output_item.added".to_string(),
                    json!({
                        "type":"response.output_item.added","output_index":0,"item":{"type":"reasoning","summary":[]}
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_start","content_block":{"type":"unknown"}}),
                Ok(None),
            ),
            (
                json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"line\n🦀"}}),
                Ok(Some((
                    "response.output_text.delta".to_string(),
                    json!({
                        "type":"response.output_text.delta","output_index":1,"delta":"line\n🦀"
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}),
                Ok(Some((
                    "response.function_call_arguments.delta".to_string(),
                    json!({
                        "type":"response.function_call_arguments.delta","output_index":0,"delta":"{\"path\":"
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"reasoning 🦀"}}),
                Ok(Some((
                    "response.reasoning_summary_text.delta".to_string(),
                    json!({
                        "type":"response.reasoning_summary_text.delta","output_index":0,"delta":"reasoning 🦀"
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_delta","delta":{"type":"text_delta","text":3}}),
                Ok(Some((
                    "response.output_text.delta".to_string(),
                    json!({
                        "type":"response.output_text.delta","output_index":0,"delta":""
                    }),
                ))),
            ),
            (
                json!({"type":"content_block_delta","delta":{"type":"unknown_delta"}}),
                Ok(None),
            ),
            (
                json!({"type":"content_block_start"}),
                Err("Anthropic content_block_start requires content_block".to_string()),
            ),
            (
                json!({"type":"content_block_start","content_block":{}}),
                Err("Anthropic content block requires type".to_string()),
            ),
            (
                json!({"type":"content_block_delta"}),
                Err("Anthropic content_block_delta requires delta.type".to_string()),
            ),
            (
                json!({"type":"content_block_delta","delta":{}}),
                Err("Anthropic content_block_delta requires delta.type".to_string()),
            ),
            (
                json!({"type":"content_block_start","content_block":{"type":"server_tool_use","name":"web_search"}}),
                Err("Anthropic server_tool_use block must contain id".to_string()),
            ),
            (
                json!({"type":"message_stop"}),
                Ok(Some((
                    "response.completed".to_string(),
                    json!({"type":"response.completed"}),
                ))),
            ),
            (
                json!({"type":"error","error":{"type":"overloaded_error","message":null}}),
                Ok(Some((
                    "error".to_string(),
                    json!({"type":"error","error":{"type":"overloaded_error","message":null}}),
                ))),
            ),
            (json!({"type":"ping"}), Ok(None)),
            (json!({}), Ok(None)),
        ]
    }

    #[test]
    fn stream_event_matches_independent_fixtures() {
        for (value, expected) in stream_fixtures() {
            let mojo = normalized(anthropic_stream_mojo_value(&value, 123));
            assert_eq!(mojo, expected, "Mojo fixture mismatch for {value}");
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
