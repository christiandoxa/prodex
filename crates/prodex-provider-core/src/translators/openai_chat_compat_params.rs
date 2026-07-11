//! Supported-parameter reporting for the shared Responses-to-chat bridge.

use crate::ProviderId;
use crate::translator::{ProviderParamSupport, ProviderUnsupportedReason};

pub(crate) fn responses_chat_compat_supported_params(provider: ProviderId) -> ProviderParamSupport {
    ProviderParamSupport {
        supported: true,
        unsupported: vec![
            ProviderUnsupportedReason {
                field: "input[*].content[type!=text]".to_string(),
                reason: format!(
                    "{} Responses chat-compat currently translates only text input content",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "response_format.type".to_string(),
                reason: format!(
                    "{} Responses chat-compat does not translate response_format controls",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "reasoning".to_string(),
                reason: format!(
                    "{} Responses chat-compat does not map Responses reasoning controls",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "text.format".to_string(),
                reason: format!(
                    "{} Responses chat-compat does not translate text.format controls",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "n>1".to_string(),
                reason: format!(
                    "{} Responses chat-compat returns only the first choice and does not support n>1",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "metadata".to_string(),
                reason: format!(
                    "{} Responses chat-compat does not translate request metadata",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "safety_identifier".to_string(),
                reason: format!(
                    "{} Responses chat-compat does not translate safety_identifier",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "web_search_options".to_string(),
                reason: format!(
                    "{} Responses chat-compat does not translate web_search_options",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "input[type=custom_tool_call|tool_search_call]".to_string(),
                reason: format!(
                    "{} Responses chat-compat only translates message/function-call history items",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "messages".to_string(),
                reason: format!(
                    "{} Responses chat-compat expects Responses input, not raw chat-completions messages",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "tools[type!=function]".to_string(),
                reason: format!(
                    "{} Responses chat-compat only forwards function tools",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "tool_choice[type!=function]".to_string(),
                reason: format!(
                    "{} Responses chat-compat only forwards function tool_choice controls",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "parallel_tool_calls=false".to_string(),
                reason: format!(
                    "{} Responses chat-compat does not prove a compatible parallel_tool_calls=false control",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "logprobs/top_logprobs".to_string(),
                reason: format!(
                    "{} Responses chat-compat does not translate logprobs controls",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "stop_sequences".to_string(),
                reason: format!(
                    "{} Responses chat-compat does not translate stop_sequences",
                    provider.label()
                ),
            },
            ProviderUnsupportedReason {
                field: "previous_response_id".to_string(),
                reason: format!(
                    "{} Responses chat-compat does not map previous_response_id continuation state",
                    provider.label()
                ),
            },
        ],
    }
}
