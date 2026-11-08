//! Stream completion, final response assembly, and conversation snapshot persistence.

use super::super::deepseek_rewrite::runtime_deepseek_store_conversation;
use super::super::provider_bridge::runtime_provider_label;
use super::super::provider_sse_events::runtime_provider_sse_failed_event;
use super::RuntimeDeepSeekSseState;
use prodex_provider_core::{
    DeepSeekProviderCoreStreamChatToolCall, deepseek_provider_core_response_completed_event,
    deepseek_provider_core_stream_chat_assistant_message,
    deepseek_provider_core_stream_fallback_tool_call_id,
    deepseek_provider_core_stream_output_text_item,
    deepseek_provider_core_stream_output_text_item_id,
    deepseek_provider_core_stream_response_metadata, deepseek_provider_core_stream_response_value,
    deepseek_provider_core_stream_tool_call_item,
};

impl RuntimeDeepSeekSseState {
    pub(in crate::runtime_launch::proxy_startup) fn complete_event(&mut self) -> Option<String> {
        if self.completed {
            return None;
        }
        if let Err(message) = self.validate_tool_call_arguments() {
            return self.failed_event("invalid_tool_call_arguments", &message);
        }
        let mut events = match self.complete_tool_call_events() {
            Ok(events) => events,
            Err(error) => {
                return self.failed_event(
                    "invalid_tool_call_arguments",
                    &format!("compiled Mojo DeepSeek SSE completion planner failed: {error:?}"),
                );
            }
        };
        events.extend(self.complete_reasoning_events());
        events.extend(self.complete_output_text_item_events());
        let response = deepseek_provider_core_stream_response_value(
            &self.response_id,
            self.output_items(),
            self.model.as_deref(),
            self.usage.clone(),
            deepseek_provider_core_stream_response_metadata(
                runtime_provider_label(self.provider_kind),
                self.logprobs.clone(),
                &self.reasoning_content,
                &self.refusal,
                &self.annotations,
                self.finish_reason.as_deref(),
                self.system_fingerprint.as_deref(),
            ),
            self.response_metadata.clone(),
        );
        let sequence_number = self.next_sequence_number();
        events.push(self.event(
            "response.completed",
            deepseek_provider_core_response_completed_event(
                sequence_number,
                self.created_at,
                &response,
            ),
        ));
        runtime_deepseek_store_conversation(
            &self.conversations,
            &self.response_id,
            self.conversation_messages.clone(),
            self.chat_assistant_messages(),
        );
        self.completed = true;
        Some(events.join(""))
    }

    pub(in crate::runtime_launch::proxy_startup) fn failed_event(
        &mut self,
        code: &str,
        message: &str,
    ) -> Option<String> {
        if self.completed {
            return None;
        }
        let sequence_number = self.next_sequence_number();
        self.completed = true;
        Some(runtime_provider_sse_failed_event(
            sequence_number,
            self.created_at,
            &self.response_id,
            code,
            message,
        ))
    }

    pub(super) fn output_text_item_id(&self) -> String {
        deepseek_provider_core_stream_output_text_item_id(
            runtime_provider_label(self.provider_kind),
            self.request_id,
        )
    }

    pub(super) fn output_text_item_added_event(&mut self) -> Option<String> {
        if self.output_text_item_added {
            return None;
        }
        self.output_text_item_added = true;
        let id = self.output_text_item_id();
        let item = serde_json::json!({"id":id,"type":"message","role":"assistant","status":"in_progress","content":[]});
        Some(self.output_item_event("response.output_item.added", &id, item))
    }

    pub(super) fn complete_output_text_item_events(&mut self) -> Vec<String> {
        if self.output_text.is_empty() || self.output_text_item_done {
            return Vec::new();
        }
        let mut events = Vec::new();
        events.extend(self.output_text_item_added_event());
        self.output_text_item_done = true;
        let id = self.output_text_item_id();
        let mut item = deepseek_provider_core_stream_output_text_item(&self.output_text);
        item["id"] = id.clone().into();
        item["status"] = "completed".into();
        events.push(self.output_item_event("response.output_item.done", &id, item));
        events
    }

    pub(super) fn store_conversation_snapshot(&self) {
        runtime_deepseek_store_conversation(
            &self.conversations,
            &self.response_id,
            self.conversation_messages.clone(),
            self.chat_assistant_messages(),
        );
    }

    fn chat_assistant_messages(&self) -> Vec<serde_json::Value> {
        let tool_calls = self
            .tool_calls
            .iter()
            .map(
                |(index, tool_call)| DeepSeekProviderCoreStreamChatToolCall {
                    call_id: tool_call.call_id.clone().unwrap_or_else(|| {
                        deepseek_provider_core_stream_fallback_tool_call_id(
                            "deepseek",
                            self.request_id,
                            *index,
                        )
                    }),
                    name: tool_call
                        .name
                        .clone()
                        .unwrap_or_else(|| "tool_call".to_string()),
                    arguments: tool_call.arguments.clone(),
                    thought_signature: tool_call.thought_signature.clone(),
                },
            )
            .collect::<Vec<_>>();
        deepseek_provider_core_stream_chat_assistant_message(
            &self.output_text,
            &self.reasoning_content,
            &tool_calls,
        )
        .into_iter()
        .collect()
    }

    fn output_items(&self) -> Vec<serde_json::Value> {
        let mut output = self.external_output_items.clone();
        if !self.reasoning_content.is_empty() {
            output.push(self.reasoning_item());
        }
        if !self.output_text.is_empty() {
            let mut item = deepseek_provider_core_stream_output_text_item(&self.output_text);
            item["id"] = self.output_text_item_id().into();
            item["status"] = "completed".into();
            output.push(item);
        }
        for (index, tool_call) in &self.tool_calls {
            let call_id = tool_call.call_id.clone().unwrap_or_else(|| {
                deepseek_provider_core_stream_fallback_tool_call_id(
                    "deepseek",
                    self.request_id,
                    *index,
                )
            });
            let flat_name = tool_call
                .name
                .clone()
                .unwrap_or_else(|| "tool_call".to_string());
            let mut item = deepseek_provider_core_stream_tool_call_item(
                &call_id,
                &flat_name,
                &tool_call.arguments,
                tool_call.thought_signature.as_deref(),
            );
            item["id"] = call_id.into();
            output.push(item);
        }
        output.sort_by_key(|item| {
            item["id"]
                .as_str()
                .and_then(|id| self.output_indices.get(id))
                .copied()
                .unwrap_or(usize::MAX)
        });
        output
    }

    pub(in crate::runtime_launch::proxy_startup) fn record_external_output_item(
        &mut self,
        item: serde_json::Value,
    ) {
        self.external_output_items.push(item);
    }

    pub(in crate::runtime_launch::proxy_startup) fn event(
        &self,
        event: &str,
        data: serde_json::Value,
    ) -> String {
        let data = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
        format!("event: {event}\r\ndata: {data}\r\n\r\n")
    }

    pub(in crate::runtime_launch::proxy_startup) fn next_sequence_number(&mut self) -> u64 {
        let next = self.sequence_number;
        self.sequence_number = self.sequence_number.saturating_add(1);
        next
    }
}
