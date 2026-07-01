use super::super::super::deepseek_rewrite::runtime_deepseek_rtk_wrapped_tool_arguments;
use super::super::super::gemini_rewrite::runtime_gemini_custom_tool_input_from_arguments;
use super::super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_stream_function_call_arguments_delta_event,
};
use super::RuntimeGeminiSseState;
use super::runtime_gemini_blocked_tool_call_item;
use super::runtime_gemini_blocked_tool_call_message;
use super::runtime_provider_split_flat_namespace_tool_name;
use prodex_domain::CallId;

#[derive(Debug, Default)]
pub(super) struct RuntimeGeminiToolCall {
    pub(super) call_id: Option<String>,
    pub(super) explicit_call_id: bool,
    pub(super) name: Option<String>,
    pub(super) arguments: String,
    pub(super) thought_signature: Option<String>,
    pub(super) added: bool,
    pub(super) done: bool,
}

impl RuntimeGeminiSseState {
    pub(super) fn observe_function_call(
        &mut self,
        part_index: usize,
        value: &serde_json::Value,
        thought_signature: Option<String>,
    ) -> Vec<String> {
        let explicit_call_id = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .map(str::to_string);
        let name = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("tool_call")
            .to_string();
        let index = self.function_call_index(part_index, explicit_call_id.as_deref(), &name);
        let args = value
            .get("args")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let args = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
        let mut events = Vec::new();
        let (call_id, should_add) = {
            let tool_call = self.tool_calls.entry(index).or_default();
            tool_call.call_id = explicit_call_id
                .clone()
                .or_else(|| tool_call.call_id.clone())
                .or_else(|| Some(format!("call_gemini_{}", CallId::new())));
            if let Some(explicit_call_id) = explicit_call_id {
                self.tool_call_indices_by_id.insert(explicit_call_id, index);
                tool_call.explicit_call_id = true;
            }
            tool_call.name = Some(name.clone());
            tool_call.arguments = args.clone();
            if thought_signature.is_some() {
                tool_call.thought_signature = thought_signature.clone();
            }
            let call_id = tool_call.call_id.clone().unwrap_or_default();
            let should_add = !tool_call.added;
            if should_add {
                tool_call.added = true;
            }
            (call_id, should_add)
        };
        if should_add && name != "tool_search" && name != "apply_patch" {
            let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(&name);
            let mut item = serde_json::json!({
                "type": "function_call",
                "call_id": call_id,
                "name": name,
            });
            if let Some(namespace) = namespace {
                item["namespace"] = serde_json::Value::String(namespace);
            }
            if let Some(tool_call) = self.tool_calls.get(&index)
                && let Some(signature) = tool_call.thought_signature.clone()
            {
                item["gemini_thought_signature"] = serde_json::Value::String(signature);
            }
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.output_item.added",
                serde_json::json!({
                    "type": "response.output_item.added",
                    "sequence_number": sequence_number,
                    "item": item,
                }),
            ));
        }

        if name != "tool_search" && name != "apply_patch" {
            let sequence_number = self.next_sequence_number();
            let upstream_value = serde_json::json!({
                "candidates": [{
                    "content": {
                        "parts": [{
                            "functionCall": {
                                "id": call_id,
                                "name": name,
                                "args": serde_json::from_str::<serde_json::Value>(&args)
                                    .unwrap_or_else(|_| serde_json::json!({})),
                            }
                        }]
                    }
                }]
            });
            let mut delta_event = runtime_provider_stream_function_call_arguments_delta_event(
                RuntimeProviderBridgeKind::Gemini,
                &upstream_value,
                sequence_number,
            )
            .map(|(_, data)| data)
            .unwrap_or_else(|| {
                serde_json::json!({
                    "type": "response.function_call_arguments.delta",
                    "sequence_number": sequence_number,
                    "call_id": call_id,
                    "delta": args,
                })
            });
            if let Some(ref signature) = thought_signature.clone() {
                delta_event["thought_signature"] = serde_json::Value::String(signature.to_string());
            }
            events.push(self.event("response.function_call_arguments.delta", delta_event));
        }
        events
    }

    fn function_call_index(
        &self,
        part_index: usize,
        explicit_call_id: Option<&str>,
        name: &str,
    ) -> usize {
        if let Some(call_id) = explicit_call_id
            && let Some(index) = self.tool_call_indices_by_id.get(call_id)
        {
            return *index;
        }
        if explicit_call_id.is_none()
            && let Some(tool_call) = self.tool_calls.get(&part_index)
            && !tool_call.done
            && !tool_call.explicit_call_id
            && tool_call
                .name
                .as_deref()
                .is_none_or(|existing| existing == name)
        {
            return part_index;
        }
        if explicit_call_id.is_none()
            && let Some((index, _)) = self.tool_calls.iter().find(|(_, tool_call)| {
                !tool_call.done
                    && !tool_call.explicit_call_id
                    && tool_call.name.as_deref() == Some(name)
            })
        {
            return *index;
        }
        if !self.tool_calls.contains_key(&part_index) {
            return part_index;
        }
        let mut index = self.tool_calls.len();
        while self.tool_calls.contains_key(&index) {
            index = index.saturating_add(1);
        }
        index
    }

    pub(super) fn complete_tool_call_events(&mut self) -> Vec<String> {
        let pending = self
            .tool_calls
            .iter_mut()
            .filter_map(|(index, tool_call)| {
                if tool_call.done {
                    return None;
                }
                tool_call.done = true;
                let call_id = tool_call
                    .call_id
                    .clone()
                    .unwrap_or_else(|| format!("call_gemini_{}", CallId::new()));
                let name = tool_call
                    .name
                    .clone()
                    .unwrap_or_else(|| "tool_call".to_string());
                let raw_arguments_value =
                    serde_json::from_str::<serde_json::Value>(&tool_call.arguments)
                        .unwrap_or_else(|_| serde_json::Value::String(tool_call.arguments.clone()));
                if let Some(blocked) =
                    runtime_gemini_blocked_tool_call_message(&name, &raw_arguments_value)
                {
                    return Some((call_id, name, blocked, *index, true));
                }
                let arguments = if name == "apply_patch" {
                    tool_call.arguments.clone()
                } else {
                    runtime_deepseek_rtk_wrapped_tool_arguments(&name, &tool_call.arguments)
                };
                tool_call.arguments = arguments.clone();
                Some((call_id, name, arguments, *index, false))
            })
            .collect::<Vec<_>>();
        let mut events = Vec::new();
        for (call_id, name, arguments, index, blocked) in pending {
            if blocked {
                let sequence_number = self.next_sequence_number();
                events.push(self.event(
                    "response.output_item.done",
                    serde_json::json!({
                        "type": "response.output_item.done",
                        "sequence_number": sequence_number,
                        "item": runtime_gemini_blocked_tool_call_item(&arguments),
                    }),
                ));
                continue;
            }
            if name == "tool_search" {
                let arguments = serde_json::from_str::<serde_json::Value>(&arguments)
                    .unwrap_or_else(|_| serde_json::json!({}));
                let sequence_number = self.next_sequence_number();
                events.push(self.event(
                    "response.output_item.done",
                    serde_json::json!({
                        "type": "response.output_item.done",
                        "sequence_number": sequence_number,
                        "item": {
                            "type": "tool_search_call",
                            "call_id": call_id,
                            "execution": "client",
                            "arguments": arguments,
                        },
                    }),
                ));
                continue;
            }
            if name == "apply_patch" {
                let sequence_number = self.next_sequence_number();
                events.push(self.event(
                    "response.output_item.done",
                    serde_json::json!({
                        "type": "response.output_item.done",
                        "sequence_number": sequence_number,
                        "item": {
                            "type": "custom_tool_call",
                            "call_id": call_id,
                            "name": name,
                            "input": runtime_gemini_custom_tool_input_from_arguments(&arguments),
                        },
                    }),
                ));
                continue;
            }
            let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(&name);
            let mut item = serde_json::json!({
                "type": "function_call",
                "call_id": call_id,
                "name": name,
                "arguments": arguments,
            });
            if let Some(namespace) = namespace {
                item["namespace"] = serde_json::Value::String(namespace);
            }
            if let Some(tool_call) = self.tool_calls.get(&index)
                && let Some(signature) = tool_call.thought_signature.clone()
            {
                item["gemini_thought_signature"] = serde_json::Value::String(signature);
            }
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.output_item.done",
                serde_json::json!({
                    "type": "response.output_item.done",
                    "sequence_number": sequence_number,
                    "item": item,
                }),
            ));
        }
        events
    }
}
