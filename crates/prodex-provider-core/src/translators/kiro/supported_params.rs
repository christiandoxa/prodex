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
                reason: "Kiro ACP does not expose chat token-limit controls".to_string(),
            },
            ProviderUnsupportedReason {
                field: "tool_choice!=auto/function_call".to_string(),
                reason: "Kiro ACP owns tool selection".to_string(),
            },
            ProviderUnsupportedReason {
                field: "logprobs/top_logprobs".to_string(),
                reason: "Kiro ACP does not expose log probabilities".to_string(),
            },
        ],
    }
}

pub(super) fn kiro_responses_supported_params(
    ignores_required_token_limit: bool,
) -> ProviderParamSupport {
    let mut unsupported = vec![
        ProviderUnsupportedReason {
            field: "temperature/top_p".to_string(),
            reason: "Kiro ACP does not expose sampling controls".to_string(),
        },
        ProviderUnsupportedReason {
            field: "stop/stop_sequences".to_string(),
            reason: "Kiro ACP does not expose stop-sequence controls".to_string(),
        },
        ProviderUnsupportedReason {
            field: "logprobs/top_logprobs".to_string(),
            reason: "Kiro ACP does not expose log probabilities".to_string(),
        },
        ProviderUnsupportedReason {
            field: "response_format/text.format[type!=text]".to_string(),
            reason: "Kiro ACP does not guarantee structured output".to_string(),
        },
        ProviderUnsupportedReason {
            field: "tool_choice!=auto".to_string(),
            reason: "Kiro ACP owns tool selection".to_string(),
        },
        ProviderUnsupportedReason {
            field: "tools/web_search_options".to_string(),
            reason: "Kiro ACP owns its tool and web-search inventory".to_string(),
        },
        ProviderUnsupportedReason {
            field: "parallel_tool_calls=false".to_string(),
            reason: "Kiro ACP does not expose parallel tool-call control".to_string(),
        },
        ProviderUnsupportedReason {
            field: "input[*].content[type!=text]".to_string(),
            reason: "Kiro ACP is initialized as a text-only client".to_string(),
        },
    ];
    unsupported.push(ProviderUnsupportedReason {
        field: "max_output_tokens/max_tokens/max_completion_tokens".to_string(),
        reason: if ignores_required_token_limit {
            "Kiro Messages accepts the required token limit for compatibility, but ACP cannot enforce it"
        } else {
            "Kiro ACP does not expose output token-limit controls"
        }
        .to_string(),
    });
    ProviderParamSupport {
        supported: true,
        unsupported,
    }
}
