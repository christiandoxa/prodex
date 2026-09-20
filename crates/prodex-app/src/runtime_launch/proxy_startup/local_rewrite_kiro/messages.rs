use super::*;

pub(super) fn runtime_kiro_messages_streaming_upstream_result(
    context: RuntimeKiroRequestContext<'_>,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let profile_name = context.auth.profile_name.clone();
    let buffered = runtime_kiro_buffered_upstream_result(context)?;
    let RuntimeLocalRewriteUpstreamResponse::Buffered(parts) = buffered.response else {
        anyhow::bail!("Kiro Messages compatibility expected a buffered ACP result");
    };
    let message: Value = serde_json::from_slice(&parts.body)
        .context("failed to parse Kiro Anthropic Messages response JSON")?;
    let body = runtime_kiro_anthropic_sse_body(&message)?;
    Ok(RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Streaming(
            RuntimeLocalRewriteStreamingResponse {
                status: 200,
                headers: vec![(
                    "content-type".to_string(),
                    "text/event-stream; charset=utf-8".to_string(),
                )],
                body: Box::new(Cursor::new(body)),
                profile_name,
                accepted_binding_recorder: None,
                accepted_binding: None,
            },
        ),
        gemini_context: None,
        copilot_context: None,
    })
}

fn runtime_kiro_anthropic_sse_body(message: &Value) -> Result<Vec<u8>> {
    fn push_event(output: &mut Vec<u8>, event: &str, value: Value) -> Result<()> {
        use std::io::Write as _;
        writeln!(output, "event: {event}")?;
        writeln!(output, "data: {}", serde_json::to_string(&value)?)?;
        writeln!(output)?;
        Ok(())
    }

    let mut output = Vec::new();
    let mut start_message = message.clone();
    start_message["content"] = Value::Array(Vec::new());
    start_message["stop_reason"] = Value::Null;
    push_event(
        &mut output,
        "message_start",
        serde_json::json!({"type":"message_start","message":start_message}),
    )?;

    if let Some(content) = message.get("content").and_then(Value::as_array) {
        for (index, block) in content.iter().enumerate() {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    push_event(
                        &mut output,
                        "content_block_start",
                        serde_json::json!({
                            "type":"content_block_start",
                            "index":index,
                            "content_block":{"type":"text","text":""}
                        }),
                    )?;
                    let text = block
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if !text.is_empty() {
                        push_event(
                            &mut output,
                            "content_block_delta",
                            serde_json::json!({
                                "type":"content_block_delta",
                                "index":index,
                                "delta":{"type":"text_delta","text":text}
                            }),
                        )?;
                    }
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("call_kiro");
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("tool_call");
                    push_event(
                        &mut output,
                        "content_block_start",
                        serde_json::json!({
                            "type":"content_block_start",
                            "index":index,
                            "content_block":{"type":"tool_use","id":id,"name":name,"input":{}}
                        }),
                    )?;
                    let input = block
                        .get("input")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({}));
                    push_event(
                        &mut output,
                        "content_block_delta",
                        serde_json::json!({
                            "type":"content_block_delta",
                            "index":index,
                            "delta":{
                                "type":"input_json_delta",
                                "partial_json":serde_json::to_string(&input)?
                            }
                        }),
                    )?;
                }
                _ => continue,
            }
            push_event(
                &mut output,
                "content_block_stop",
                serde_json::json!({"type":"content_block_stop","index":index}),
            )?;
        }
    }

    let stop_reason = message
        .get("stop_reason")
        .cloned()
        .unwrap_or_else(|| Value::String("end_turn".to_string()));
    let output_tokens = message
        .pointer("/usage/output_tokens")
        .cloned()
        .unwrap_or_else(|| Value::from(0));
    push_event(
        &mut output,
        "message_delta",
        serde_json::json!({
            "type":"message_delta",
            "delta":{"stop_reason":stop_reason,"stop_sequence":Value::Null},
            "usage":{"output_tokens":output_tokens}
        }),
    )?;
    push_event(
        &mut output,
        "message_stop",
        serde_json::json!({"type":"message_stop"}),
    )?;
    Ok(output)
}
