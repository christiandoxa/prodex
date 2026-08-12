use serde_json::Value;

const MAX_STREAM_MESSAGE_CHARS: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWebsocketStreamPayload {
    pub(super) source: String,
    pub(super) message: String,
}

pub(super) fn runtime_websocket_stream_payload_from_text(
    payload: &str,
) -> Option<RuntimeWebsocketStreamPayload> {
    let value = serde_json::from_str::<Value>(payload).ok()?;
    if value.get("type").and_then(Value::as_str)? != "response.output_item.done" {
        return None;
    }
    let item = value.get("item")?;
    match item.get("type").and_then(Value::as_str)? {
        "message" if item.get("role").and_then(Value::as_str) == Some("assistant") => {
            stream_payload("assistant", &completed_message_text(item)?)
        }
        item_type @ ("function_call" | "custom_tool_call" | "mcp_call") => {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(item_type);
            let text = completed_tool_input(item)?;
            stream_payload(&format_tool_source(name), &text)
        }
        _ => None,
    }
}

fn stream_payload(source: &str, message: &str) -> Option<RuntimeWebsocketStreamPayload> {
    if message.trim().is_empty()
        || message
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\t' | '\n' | '\r'))
    {
        return None;
    }
    let truncated = message.chars().count() > MAX_STREAM_MESSAGE_CHARS;
    let mut message = message
        .chars()
        .take(MAX_STREAM_MESSAGE_CHARS)
        .collect::<String>();
    if truncated {
        message.push_str("...");
    }
    Some(RuntimeWebsocketStreamPayload {
        source: source.to_string(),
        message,
    })
}

fn completed_message_text(item: &Value) -> Option<String> {
    let mut parts = Vec::new();
    for content in item.get("content")?.as_array()? {
        if let Some(text) = ["text", "refusal", "transcript"]
            .into_iter()
            .find_map(|field| content.get(field).and_then(Value::as_str))
            .filter(|text| !text.trim().is_empty())
        {
            parts.push(text);
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn completed_tool_input(item: &Value) -> Option<String> {
    match item.get("arguments").or_else(|| item.get("input"))? {
        Value::String(text) => Some(text.clone()),
        value @ (Value::Array(_) | Value::Object(_)) => serde_json::to_string(value).ok(),
        _ => None,
    }
}

fn format_tool_source(name: &str) -> String {
    let mut source = String::from("tool-call:");
    for character in name.chars().take(96) {
        source.push(
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':') {
                character
            } else {
                '_'
            },
        );
    }
    if source == "tool-call:" {
        source.push_str("tool");
    }
    source
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_function_arguments_once_after_the_item_finishes_streaming() {
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.output_item.added","item":{"type":"function_call","call_id":"call-1","name":"exec"}}"#
            ),
            None
        );
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.function_call_arguments.delta","call_id":"call-1","delta":"const r = await "}"#
            ),
            None
        );
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.function_call_arguments.delta","call_id":"call-1","delta":"tools.web__run({});"}"#
            ),
            None
        );
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.function_call_arguments.done","call_id":"call-1","arguments":"const r = await tools.web__run({});"}"#
            ),
            None
        );
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.output_item.done","item":{"type":"function_call","call_id":"call-1","name":"exec","arguments":"const r = await tools.web__run({});"}}"#
            ),
            Some(RuntimeWebsocketStreamPayload {
                source: "tool-call:exec".to_string(),
                message: "const r = await tools.web__run({});".to_string(),
            })
        );
    }

    #[test]
    fn emits_assistant_text_only_after_it_finishes_streaming() {
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.output_text.delta","delta":"hel"}"#
            ),
            None
        );
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.output_text.delta","delta":"lo"}"#
            ),
            None
        );
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.output_text.done","text":"hello"}"#
            ),
            None
        );
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.output_item.done","item":{"type":"message","role":"assistant","id":"msg-1","content":[{"type":"output_text","text":"hello"}]}}"#
            ),
            Some(RuntimeWebsocketStreamPayload {
                source: "assistant".to_string(),
                message: "hello".to_string(),
            })
        );
    }

    #[test]
    fn emits_complete_message_and_structured_tool_items() {
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.output_item.done","item":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"first"},{"type":"refusal","refusal":"second"},{"type":"audio","transcript":"third"}]}}"#
            ),
            Some(RuntimeWebsocketStreamPayload {
                source: "assistant".to_string(),
                message: "first\nsecond\nthird".to_string(),
            })
        );
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.output_item.done","item":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch"}}"#
            ),
            Some(RuntimeWebsocketStreamPayload {
                source: "tool-call:apply_patch".to_string(),
                message: "*** Begin Patch".to_string(),
            })
        );
        assert_eq!(
            runtime_websocket_stream_payload_from_text(
                r#"{"type":"response.output_item.done","item":{"type":"mcp_call","name":"search","arguments":{"query":"status"}}}"#
            ),
            Some(RuntimeWebsocketStreamPayload {
                source: "tool-call:search".to_string(),
                message: r#"{"query":"status"}"#.to_string(),
            })
        );
    }
}
