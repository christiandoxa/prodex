//! Kiro supported-parameter reporting.

use crate::translator::{ProviderParamSupport, ProviderUnsupportedReason};

pub(super) fn kiro_chat_completions_supported_params() -> ProviderParamSupport {
    ProviderParamSupport {
        supported: true,
        unsupported: vec![
            ProviderUnsupportedReason {
                field: "response_format(json_schema/json_object)".to_string(),
                reason: "Kiro currently supports only text chat response_format".to_string(),
            },
            ProviderUnsupportedReason {
                field: "n>1".to_string(),
                reason: "Kiro currently supports only one chat completion choice".to_string(),
            },
            ProviderUnsupportedReason {
                field: "stop".to_string(),
                reason: "Kiro does not currently support non-empty chat stop sequences".to_string(),
            },
            ProviderUnsupportedReason {
                field: "temperature".to_string(),
                reason: "Kiro does not currently support non-default chat temperature".to_string(),
            },
            ProviderUnsupportedReason {
                field: "top_p".to_string(),
                reason: "Kiro does not currently support non-default chat top_p".to_string(),
            },
            ProviderUnsupportedReason {
                field: "presence_penalty".to_string(),
                reason: "Kiro does not currently support non-default chat presence_penalty"
                    .to_string(),
            },
            ProviderUnsupportedReason {
                field: "frequency_penalty".to_string(),
                reason: "Kiro does not currently support non-default chat frequency_penalty"
                    .to_string(),
            },
            ProviderUnsupportedReason {
                field: "seed".to_string(),
                reason: "Kiro does not currently support chat seed".to_string(),
            },
            ProviderUnsupportedReason {
                field: "parallel_tool_calls".to_string(),
                reason: "Kiro does not currently support chat parallel_tool_calls=false"
                    .to_string(),
            },
            ProviderUnsupportedReason {
                field: "user".to_string(),
                reason: "Kiro ignores chat user metadata".to_string(),
            },
            ProviderUnsupportedReason {
                field: "max_output_tokens/max_tokens/max_completion_tokens".to_string(),
                reason:
                    "Kiro ignores valid chat token-limit controls and rejects non-positive values"
                        .to_string(),
            },
        ],
    }
}
