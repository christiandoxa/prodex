//! DeepSeek supported-parameter reporting.

use crate::ProviderEndpoint;
use crate::translator::{ProviderParamSupport, ProviderUnsupportedReason};

pub(super) fn deepseek_supported_params(endpoint: ProviderEndpoint) -> ProviderParamSupport {
    if endpoint == ProviderEndpoint::Models {
        return ProviderParamSupport::full();
    }
    if matches!(
        endpoint,
        ProviderEndpoint::Responses
            | ProviderEndpoint::ChatCompletions
            | ProviderEndpoint::Messages
    ) {
        return ProviderParamSupport {
            supported: true,
            unsupported: vec![
                ProviderUnsupportedReason {
                    field: "parallel_tool_calls=false".to_string(),
                    reason: "DeepSeek does not expose a compatible parallel tool disable control"
                        .to_string(),
                },
                ProviderUnsupportedReason {
                    field: "web_search_options".to_string(),
                    reason: "DeepSeek v1 translator does not map Responses web search options"
                        .to_string(),
                },
                ProviderUnsupportedReason {
                    field: "safety_identifier".to_string(),
                    reason: "DeepSeek v1 translator does not forward Responses safety_identifier"
                        .to_string(),
                },
                ProviderUnsupportedReason {
                    field: "tools[type!=function]".to_string(),
                    reason:
                        "DeepSeek v1 translator only forwards plain function tools on Responses"
                            .to_string(),
                },
                ProviderUnsupportedReason {
                    field: "input[*].content[type!=text]".to_string(),
                    reason: "DeepSeek v1 translator does not translate multimodal Responses input"
                        .to_string(),
                },
            ],
        };
    }
    let mut support = ProviderParamSupport::full();
    if endpoint != ProviderEndpoint::Responses {
        support.supported = false;
        support.unsupported.push(ProviderUnsupportedReason {
            field: endpoint.label().to_string(),
            reason: "v1 translator currently models only responses compatibility".to_string(),
        });
    }
    support
}
