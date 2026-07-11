use super::super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_stream_function_call_arguments_delta_event,
};
use super::RuntimeGeminiSseState;
use super::runtime_gemini_blocked_tool_call_message_with_config;
use prodex_domain::CallId;
use prodex_provider_core::{
    gemini_provider_core_function_call_arguments_delta_event,
    gemini_provider_core_function_call_arguments_delta_event_with_thought_signature,
    gemini_provider_core_output_item_added_event, gemini_provider_core_output_item_done_event,
    gemini_provider_core_stream_completed_tool_call_arguments,
    gemini_provider_core_stream_completed_tool_call_item,
    gemini_provider_core_stream_function_call_arguments_delta_source,
    gemini_provider_core_stream_function_call_delta,
    gemini_provider_core_stream_should_emit_function_call_arguments_delta,
    gemini_provider_core_stream_tool_call, gemini_provider_core_stream_tool_call_added_item,
    gemini_provider_core_stream_tool_call_arguments_value,
};

#[derive(Debug)]
pub(super) struct RuntimeGeminiToolCall {
    pub(super) call_id: Option<String>,
    pub(super) explicit_call_id: bool,
    pub(super) name: Option<String>,
    pub(super) arguments: String,
    pub(super) thought_signature: Option<String>,
    pub(super) added: bool,
    pub(super) done: bool,
}

impl Default for RuntimeGeminiToolCall {
    fn default() -> Self {
        Self {
            call_id: Some(format!("call_gemini_{}", CallId::new())),
            explicit_call_id: false,
            name: None,
            arguments: String::new(),
            thought_signature: None,
            added: false,
            done: false,
        }
    }
}

impl RuntimeGeminiSseState {
    pub(super) fn observe_function_call(
        &mut self,
        part_index: usize,
        value: &serde_json::Value,
        thought_signature: Option<String>,
    ) -> Vec<String> {
        let function_call_delta = gemini_provider_core_stream_function_call_delta(value);
        let explicit_call_id = function_call_delta.explicit_call_id;
        let name = function_call_delta.name;
        let index = self.function_call_index(part_index, explicit_call_id.as_deref(), &name);
        let args = function_call_delta.arguments;
        let mut events = Vec::new();
        let (call_id, should_add) = {
            let tool_call = self.tool_calls.entry(index).or_default();
            let call_id = gemini_provider_core_stream_tool_call(
                self.request_id,
                index,
                explicit_call_id.as_deref().or(tool_call.call_id.as_deref()),
                Some(&name),
                &args,
                tool_call.thought_signature.as_deref(),
            )
            .call_id;
            tool_call.call_id = Some(call_id.clone());
            if let Some(explicit_call_id) = explicit_call_id {
                self.tool_call_indices_by_id.insert(explicit_call_id, index);
                tool_call.explicit_call_id = true;
            }
            tool_call.name = Some(name.clone());
            tool_call.arguments = args.clone();
            if thought_signature.is_some() {
                tool_call.thought_signature = thought_signature.clone();
            }
            let should_add = !tool_call.added;
            if should_add {
                tool_call.added = true;
            }
            (call_id, should_add)
        };
        if should_add {
            let thought_signature = self
                .tool_calls
                .get(&index)
                .and_then(|tool_call| tool_call.thought_signature.as_deref());
            if let Some(item) =
                gemini_provider_core_stream_tool_call_added_item(&call_id, &name, thought_signature)
            {
                let sequence_number = self.next_sequence_number();
                events.push(self.event(
                    "response.output_item.added",
                    gemini_provider_core_output_item_added_event(sequence_number, &item),
                ));
            }
        }

        if gemini_provider_core_stream_should_emit_function_call_arguments_delta(&name) {
            let sequence_number = self.next_sequence_number();
            let upstream_value = gemini_provider_core_stream_function_call_arguments_delta_source(
                &call_id, &name, &args,
            );
            let delta_event = runtime_provider_stream_function_call_arguments_delta_event(
                RuntimeProviderBridgeKind::Gemini,
                &upstream_value,
                sequence_number,
            )
            .map(|(_, data)| data)
            .unwrap_or_else(|| {
                gemini_provider_core_function_call_arguments_delta_event(
                    sequence_number,
                    &call_id,
                    &args,
                )
            });
            let delta_event =
                gemini_provider_core_function_call_arguments_delta_event_with_thought_signature(
                    delta_event,
                    thought_signature.as_deref(),
                );
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
                let stream_tool_call = gemini_provider_core_stream_tool_call(
                    self.request_id,
                    *index,
                    tool_call.call_id.as_deref(),
                    tool_call.name.as_deref(),
                    &tool_call.arguments,
                    tool_call.thought_signature.as_deref(),
                );
                let call_id = stream_tool_call.call_id;
                let name = stream_tool_call.name;
                let raw_arguments_value = gemini_provider_core_stream_tool_call_arguments_value(
                    &stream_tool_call.arguments,
                );
                if let Some(blocked) = runtime_gemini_blocked_tool_call_message_with_config(
                    &name,
                    &raw_arguments_value,
                    &self.gemini_config,
                ) {
                    return Some((
                        call_id,
                        name,
                        blocked,
                        stream_tool_call.thought_signature,
                        true,
                    ));
                }
                let arguments = gemini_provider_core_stream_completed_tool_call_arguments(
                    &name,
                    &stream_tool_call.arguments,
                );
                tool_call.arguments = arguments.clone();
                Some((
                    call_id,
                    name,
                    arguments,
                    stream_tool_call.thought_signature,
                    false,
                ))
            })
            .collect::<Vec<_>>();
        let mut events = Vec::new();
        for (call_id, name, arguments, thought_signature, blocked) in pending {
            let item = gemini_provider_core_stream_completed_tool_call_item(
                &call_id,
                &name,
                &arguments,
                thought_signature.as_deref(),
                blocked,
            );
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.output_item.done",
                gemini_provider_core_output_item_done_event(sequence_number, None, &item),
            ));
        }
        events
    }
}
