use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

const MAX_TOOL_NAMES: usize = 128;
const MAX_STREAM_MESSAGE_CHARS: usize = 16 * 1024;

#[derive(Debug, Default)]
pub(super) struct RuntimeWebsocketStreamState {
    tool_names: BTreeMap<String, String>,
    tool_delta_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWebsocketStreamPayload {
    pub(super) source: String,
    pub(super) message: String,
}

impl RuntimeWebsocketStreamState {
    pub(super) fn payload_from_text(
        &mut self,
        payload: &str,
    ) -> Option<RuntimeWebsocketStreamPayload> {
        let value = serde_json::from_str::<Value>(payload).ok()?;
        let event_type = value.get("type").and_then(Value::as_str)?;
        match event_type {
            "response.output_item.added" => {
                self.remember_tool(value.get("item")?);
                None
            }
            "response.output_text.delta"
            | "response.refusal.delta"
            | "response.audio_transcript.delta"
            | "response.output_audio_transcript.delta" => {
                self.payload("assistant", value.get("delta").and_then(Value::as_str)?)
            }
            "response.function_call_arguments.delta" | "response.custom_tool_call_input.delta" => {
                let call_id = value
                    .get("call_id")
                    .or_else(|| value.get("item_id"))
                    .and_then(Value::as_str);
                if let Some(call_id) = call_id.filter(|id| !id.is_empty()) {
                    if self.tool_delta_ids.len() >= MAX_TOOL_NAMES {
                        self.tool_delta_ids.pop_first();
                    }
                    self.tool_delta_ids.insert(call_id.to_string());
                }
                let name = self
                    .tool_name(&value, call_id)
                    .unwrap_or_else(|| "tool".into());
                self.payload(
                    &format_tool_source(&name),
                    value.get("delta").and_then(Value::as_str)?,
                )
            }
            "response.function_call_arguments.done" | "response.custom_tool_call_input.done" => {
                let call_id = value
                    .get("call_id")
                    .or_else(|| value.get("item_id"))
                    .and_then(Value::as_str);
                if call_id.is_some_and(|id| self.tool_delta_ids.contains(id)) {
                    return None;
                }
                let name = self
                    .tool_name(&value, call_id)
                    .unwrap_or_else(|| "tool".into());
                let text = value
                    .get("arguments")
                    .or_else(|| value.get("input"))
                    .and_then(Value::as_str)?;
                self.payload(&format_tool_source(&name), text)
            }
            "response.output_item.done" => {
                let item = value.get("item")?;
                let item_type = item.get("type").and_then(Value::as_str)?;
                if !matches!(item_type, "function_call" | "custom_tool_call" | "mcp_call") {
                    return None;
                }
                let call_id = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str);
                if call_id.is_some_and(|id| self.tool_delta_ids.contains(id)) {
                    return None;
                }
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(item_type);
                let text = item
                    .get("arguments")
                    .or_else(|| item.get("input"))
                    .and_then(Value::as_str)?;
                self.payload(&format_tool_source(name), text)
            }
            _ => None,
        }
    }

    fn remember_tool(&mut self, item: &Value) {
        let Some(call_id) = item
            .get("call_id")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        else {
            return;
        };
        let Some(name) = item
            .get("name")
            .or_else(|| {
                item.get("function")
                    .and_then(|function| function.get("name"))
            })
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
        else {
            return;
        };
        if self.tool_names.len() >= MAX_TOOL_NAMES {
            self.tool_names.pop_first();
        }
        self.tool_names
            .insert(call_id.to_string(), name.chars().take(96).collect());
    }

    fn tool_name(&self, value: &Value, call_id: Option<&str>) -> Option<String> {
        value
            .get("name")
            .or_else(|| {
                value
                    .get("function")
                    .and_then(|function| function.get("name"))
            })
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| call_id.and_then(|id| self.tool_names.get(id).cloned()))
    }

    fn payload(&self, source: &str, message: &str) -> Option<RuntimeWebsocketStreamPayload> {
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
    fn maps_function_arguments_to_the_tool_name_from_the_added_item() {
        let mut state = RuntimeWebsocketStreamState::default();

        assert_eq!(
            state.payload_from_text(
                r#"{"type":"response.output_item.added","item":{"type":"function_call","call_id":"call-1","name":"exec"}}"#
            ),
            None
        );
        assert_eq!(
            state.payload_from_text(
                r#"{"type":"response.function_call_arguments.delta","call_id":"call-1","delta":"const r = await tools.web__run({});"}"#
            ),
            Some(RuntimeWebsocketStreamPayload {
                source: "tool-call:exec".to_string(),
                message: "const r = await tools.web__run({});".to_string(),
            })
        );
    }

    #[test]
    fn maps_output_text_delta_to_assistant_text() {
        let mut state = RuntimeWebsocketStreamState::default();

        assert_eq!(
            state.payload_from_text(r#"{"type":"response.output_text.delta","delta":"hello"}"#),
            Some(RuntimeWebsocketStreamPayload {
                source: "assistant".to_string(),
                message: "hello".to_string(),
            })
        );
    }
}
