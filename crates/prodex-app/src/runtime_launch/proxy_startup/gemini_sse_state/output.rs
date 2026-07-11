//! Gemini SSE output/conversation assembly helpers.

use super::*;

impl RuntimeGeminiSseState {
    pub(super) fn store_conversation_snapshot(&self) {
        runtime_deepseek_store_conversation(
            &self.conversations,
            &self.response_id,
            self.conversation_messages.clone(),
            self.chat_assistant_messages(),
        );
    }

    fn stream_tool_calls(&self) -> Vec<GeminiProviderCoreStreamToolCall> {
        self.tool_calls
            .iter()
            .map(|(index, tool_call)| {
                gemini_provider_core_stream_tool_call(
                    self.request_id,
                    *index,
                    tool_call.call_id.as_deref(),
                    tool_call.name.as_deref(),
                    &tool_call.arguments,
                    tool_call.thought_signature.as_deref(),
                )
            })
            .collect()
    }

    pub(super) fn chat_assistant_messages(&self) -> Vec<serde_json::Value> {
        let tool_calls = self.stream_tool_calls();
        gemini_provider_core_stream_chat_assistant_message(
            &self.output_text,
            &self.reasoning_content,
            &self.media_content_items,
            &self.native_parts,
            &self.image_generation_items,
            self.metadata.as_ref(),
            &tool_calls,
        )
        .into_iter()
        .collect()
    }

    pub(super) fn output_items(&self) -> Vec<serde_json::Value> {
        let tool_calls = self.stream_tool_calls();
        gemini_provider_core_stream_output_items(
            self.web_search_call.as_ref(),
            &self.image_generation_items,
            &self.output_text,
            &self.media_content_items,
            self.citation_text.as_deref(),
            &tool_calls,
            runtime_gemini_blocked_tool_call_message,
        )
    }

    pub(super) fn tool_call_ids(&self) -> Vec<String> {
        gemini_provider_core_stream_tool_call_ids(&self.stream_tool_calls())
    }
}
