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
            | ProviderEndpoint::ResponsesCompact
            | ProviderEndpoint::ChatCompletions
            | ProviderEndpoint::Messages
    ) {
        let unsupported = prodex_mojo_core::rich::openai_compat_supported_params("DeepSeek")
            .unwrap_or_else(|error| {
                panic!("Mojo DeepSeek supported-parameter report failed: {error:?}")
            })
            .into_iter()
            .map(|parameter| ProviderUnsupportedReason {
                field: parameter.field,
                reason: parameter.reason,
            })
            .collect();
        return ProviderParamSupport {
            supported: true,
            unsupported,
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

#[cfg(test)]
mod tests {
    use super::deepseek_supported_params;
    use crate::ProviderEndpoint;

    #[test]
    fn deepseek_supported_parameter_report_comes_from_mojo() {
        let report = deepseek_supported_params(ProviderEndpoint::Responses);
        assert!(report.supported);
        assert_eq!(
            report
                .unsupported
                .iter()
                .map(|parameter| (parameter.field.as_str(), parameter.reason.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (
                    "parallel_tool_calls=false",
                    "DeepSeek does not expose a compatible parallel tool disable control",
                ),
                (
                    "web_search_options",
                    "DeepSeek v1 translator does not map Responses web search options",
                ),
                (
                    "safety_identifier",
                    "DeepSeek v1 translator does not forward Responses safety_identifier",
                ),
                (
                    "tools[type!=function]",
                    "DeepSeek v1 translator only forwards plain function tools on Responses",
                ),
                (
                    "input[*].content[type!=text]",
                    "DeepSeek v1 translator does not translate multimodal Responses input",
                ),
            ]
        );
    }
}
