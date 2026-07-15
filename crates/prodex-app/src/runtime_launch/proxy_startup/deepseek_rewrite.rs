pub(super) use self::input_items::runtime_deepseek_messages_from_responses_request;
pub(super) use self::request_validation::{
    runtime_deepseek_apply_web_search_mode, runtime_deepseek_dedup_and_validate_function_tools,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use self::request_validation::{
    runtime_deepseek_ensure_json_prompt_instruction, runtime_deepseek_function_tool_name,
    runtime_deepseek_insert_primitive_request_fields,
    runtime_deepseek_note_thinking_tool_choice_omission,
    runtime_deepseek_reject_beta_completion_fields,
    runtime_deepseek_reject_unsupported_request_fields,
    runtime_deepseek_response_format_from_responses_request,
    runtime_deepseek_response_metadata_from_responses_request,
    runtime_deepseek_stop_from_responses_request,
    runtime_deepseek_top_logprobs_from_responses_request,
    runtime_deepseek_user_id_from_responses_request, runtime_deepseek_validate_tool_choice_name,
    runtime_deepseek_validate_tool_choice_shape, runtime_deepseek_validate_tool_choice_target,
    runtime_deepseek_validate_tools_shape,
};
#[cfg(test)]
pub(in crate::runtime_launch::proxy_startup) use self::response::runtime_deepseek_chat_assistant_messages_from_response_value;
pub(in crate::runtime_launch::proxy_startup) use self::response::runtime_deepseek_merge_response_metadata;
pub(in crate::runtime_launch::proxy_startup) use self::response::{
    runtime_deepseek_chat_buffered_response_parts, runtime_deepseek_store_conversation,
    runtime_deepseek_take_pending_messages,
};
pub(super) use self::tools::{
    runtime_deepseek_tool_choice_from_responses_request,
    runtime_deepseek_tools_from_responses_request,
    runtime_deepseek_web_search_options_from_responses_request,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use super::deepseek_reasoning::{
    runtime_deepseek_apply_reasoning_from_responses_request, runtime_deepseek_thinking_enabled,
    runtime_deepseek_validate_reasoning_shape_for_provider,
};
#[cfg(test)]
pub(super) use super::deepseek_sse_reader::RuntimeDeepSeekChatSseReader;
use super::provider_bridge::RuntimeProviderBridgeKind;
use anyhow::Result;
use prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

mod conversation_store;
mod input_items;
mod request_validation;
mod response;
mod tools;

pub(super) use conversation_store::RuntimeDeepSeekConversationStore;
pub(super) type RuntimeDeepSeekPendingMessages =
    Arc<Mutex<BTreeMap<u64, RuntimeDeepSeekPendingRequest>>>;

#[derive(Clone, Default)]
pub(super) struct RuntimeDeepSeekPendingRequest {
    pub(super) messages: Vec<serde_json::Value>,
    pub(super) response_metadata: Option<serde_json::Value>,
}

pub(crate) struct RuntimeDeepSeekTranslatedRequest {
    pub(crate) body: Vec<u8>,
    pub(crate) messages: Vec<serde_json::Value>,
    pub(crate) response_metadata: Option<serde_json::Value>,
}

impl fmt::Debug for RuntimeDeepSeekPendingRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeDeepSeekPendingRequest")
            .field("messages", &redacted_len(self.messages.len()))
            .field(
                "response_metadata",
                &self.response_metadata.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl fmt::Debug for RuntimeDeepSeekTranslatedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeDeepSeekTranslatedRequest")
            .field("body", &"<redacted>")
            .field("messages", &redacted_len(self.messages.len()))
            .field(
                "response_metadata",
                &self.response_metadata.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

fn redacted_len(len: usize) -> String {
    format!("<redacted:{len}>")
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RuntimeDeepSeekRewriteOptions {
    pub(crate) strict_tools: bool,
    pub(crate) web_search_mode: RuntimeDeepSeekWebSearchMode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum RuntimeDeepSeekWebSearchMode {
    #[default]
    Auto,
    Off,
    OpenAiChat,
    Anthropic,
    FunctionProxy,
}

#[cfg(test)]
pub(super) fn runtime_deepseek_chat_request_body(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    runtime_deepseek_chat_request_body_with_options(
        body,
        conversations,
        RuntimeDeepSeekRewriteOptions::default(),
    )
}

pub(super) fn runtime_deepseek_chat_request_body_with_options(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
    options: RuntimeDeepSeekRewriteOptions,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    super::chat_compatible_request::runtime_provider_chat_compatible_request_body(
        body,
        conversations,
        RuntimeProviderBridgeKind::DeepSeek,
        SUPER_DEEPSEEK_DEFAULT_MODEL,
        true,
        options,
    )
}

#[cfg(test)]
#[path = "deepseek_optional_tools_tests.rs"]
mod deepseek_optional_tools_tests;

#[cfg(test)]
#[path = "deepseek_provider_tool_tests.rs"]
mod deepseek_provider_tool_tests;

#[cfg(test)]
#[path = "deepseek_rewrite_rtk_tests.rs"]
mod deepseek_rewrite_rtk_tests;

#[cfg(test)]
#[path = "deepseek_rewrite_tests.rs"]
mod deepseek_rewrite_tests;

#[cfg(test)]
mod debug_tests {
    use super::*;

    #[test]
    fn deepseek_request_debug_output_redacts_payloads() {
        let pending = RuntimeDeepSeekPendingRequest {
            messages: vec![serde_json::json!({"content": "pending prompt secret"})],
            response_metadata: Some(serde_json::json!({"secret": "pending metadata secret"})),
        };
        let translated = RuntimeDeepSeekTranslatedRequest {
            body: br#"{"content":"translated body secret"}"#.to_vec(),
            messages: vec![serde_json::json!({"content": "translated prompt secret"})],
            response_metadata: Some(serde_json::json!({"secret": "translated metadata secret"})),
        };
        let rendered = format!("{pending:?}\n{translated:?}");

        for raw in [
            "pending prompt secret",
            "pending metadata secret",
            "translated body secret",
            "translated prompt secret",
            "translated metadata secret",
        ] {
            assert!(!rendered.contains(raw), "{rendered}");
        }
    }
}
