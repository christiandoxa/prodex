//! Streamed tool-call accumulation, validation, and Codex SSE event emission.

use super::super::provider_bridge::{
    runtime_provider_label, runtime_provider_stream_function_call_arguments_delta_event,
};
use super::RuntimeDeepSeekSseState;
use prodex_provider_core::{
    deepseek_provider_core_function_call_arguments_delta_event,
    deepseek_provider_core_incremental_tool_argument_delta,
    deepseek_provider_core_output_item_added_event, deepseek_provider_core_output_item_done_event,
    deepseek_provider_core_rtk_wrapped_tool_arguments,
    deepseek_provider_core_stream_fallback_tool_call_id,
    deepseek_provider_core_stream_function_call_arguments_delta_source,
    deepseek_provider_core_stream_tool_call_added_item,
    deepseek_provider_core_stream_tool_call_delta, deepseek_provider_core_stream_tool_call_item,
    deepseek_provider_core_validate_stream_tool_call_arguments,
    deepseek_provider_core_validate_stream_tool_call_delta,
};

impl RuntimeDeepSeekSseState {
    pub(super) fn observe_tool_call_delta(&mut self, value: &serde_json::Value) -> Vec<String> {
        let delta = deepseek_provider_core_stream_tool_call_delta(value);
        let index = delta.index;
        let mut events = Vec::new();
        let provider_label = self.provider_kind.chat_compatible_adapter_label();
        if let Err(message) = deepseek_provider_core_validate_stream_tool_call_delta(
            provider_label,
            self.tool_calls.contains_key(&index),
            value,
        ) {
            if let Some(event) = self.failed_event("invalid_tool_call_arguments", &message) {
                events.push(event);
            }
            return events;
        }
        let (call_id, name, should_add, argument_delta) = {
            let tool_call = self.tool_calls.entry(index).or_default();
            if let Some(id) = delta.call_id {
                tool_call.call_id = Some(id);
            }
            if let Some(signature) = delta.thought_signature {
                tool_call.thought_signature = Some(signature);
            }
            if let Some(name) = delta.name {
                tool_call.name = Some(name);
            }
            let argument_delta = delta.argument_delta.as_deref().and_then(|incoming| {
                let delta = deepseek_provider_core_incremental_tool_argument_delta(
                    &tool_call.arguments,
                    incoming,
                )?;
                tool_call.arguments.push_str(&delta);
                Some(delta)
            });
            let call_id = tool_call.call_id.clone().unwrap_or_else(|| {
                deepseek_provider_core_stream_fallback_tool_call_id(
                    runtime_provider_label(self.provider_kind),
                    self.request_id,
                    index,
                )
            });
            let name = tool_call
                .name
                .clone()
                .unwrap_or_else(|| "tool_call".to_string());
            let should_add = !tool_call.added;
            if should_add {
                tool_call.added = true;
            }
            (call_id, name, should_add, argument_delta)
        };
        if should_add
            && let Some(item) = deepseek_provider_core_stream_tool_call_added_item(&call_id, &name)
        {
            let sequence_number = self.next_sequence_number();
            let mut data = deepseek_provider_core_output_item_added_event(sequence_number, &item);
            data["output_index"] = serde_json::Value::from(index);
            events.push(self.event("response.output_item.added", data));
        }
        if name == "tool_search" || name == "apply_patch" {
            return events;
        }
        let Some(arguments) = argument_delta else {
            return events;
        };
        let sequence_number = self.next_sequence_number();
        let upstream_value = deepseek_provider_core_stream_function_call_arguments_delta_source(
            &call_id, &arguments,
        );
        if let Some((event_name, mut data)) =
            runtime_provider_stream_function_call_arguments_delta_event(
                self.provider_kind,
                &upstream_value,
                sequence_number,
            )
        {
            data["output_index"] = serde_json::Value::from(index);
            events.push(self.event(&event_name, data));
        } else {
            let mut data = deepseek_provider_core_function_call_arguments_delta_event(
                sequence_number,
                &call_id,
                &arguments,
            );
            data["output_index"] = serde_json::Value::from(index);
            events.push(self.event("response.function_call_arguments.delta", data));
        }
        events
    }

    pub(super) fn complete_tool_call_events(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        let pending = self
            .tool_calls
            .iter_mut()
            .filter_map(|(index, tool_call)| {
                if tool_call.done {
                    return None;
                }
                tool_call.done = true;
                let call_id = tool_call.call_id.clone().unwrap_or_else(|| {
                    deepseek_provider_core_stream_fallback_tool_call_id(
                        runtime_provider_label(self.provider_kind),
                        self.request_id,
                        *index,
                    )
                });
                let name = tool_call
                    .name
                    .clone()
                    .unwrap_or_else(|| "tool_call".to_string());
                let arguments =
                    deepseek_provider_core_rtk_wrapped_tool_arguments(&name, &tool_call.arguments);
                tool_call.arguments = arguments.clone();
                Some((
                    *index,
                    call_id,
                    name,
                    arguments,
                    tool_call.thought_signature.clone(),
                ))
            })
            .collect::<Vec<_>>();
        for (index, call_id, name, arguments, thought_signature) in pending {
            let sequence_number = self.next_sequence_number();
            let item = deepseek_provider_core_stream_tool_call_item(
                &call_id,
                &name,
                &arguments,
                thought_signature.as_deref(),
            );
            let mut data = deepseek_provider_core_output_item_done_event(sequence_number, &item);
            data["output_index"] = serde_json::Value::from(index);
            events.push(self.event("response.output_item.done", data));
        }
        events
    }

    pub(super) fn validate_tool_call_arguments(&self) -> Result<(), String> {
        let provider_label = self.provider_kind.chat_compatible_adapter_label();
        for (index, tool_call) in &self.tool_calls {
            deepseek_provider_core_validate_stream_tool_call_arguments(
                provider_label,
                *index,
                tool_call.name.as_deref(),
                &tool_call.arguments,
            )?;
        }
        Ok(())
    }
}
