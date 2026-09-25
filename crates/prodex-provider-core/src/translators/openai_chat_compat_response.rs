#[path = "openai_chat_compat_response/stream.rs"]
mod stream;

pub(crate) use self::stream::translate_chat_stream_event_to_responses;
use super::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformResult,
    ProviderWireFormat, Value,
};
#[cfg(feature = "mojo")]
use crate::mojo_json::Document;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn translate_chat_response_to_responses(
    provider: ProviderId,
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    translate_chat_response_to_responses_at(provider, input, unix_now_secs())
}

fn translate_chat_response_to_responses_at(
    provider: ProviderId,
    input: ProviderTransformInput,
    now_secs: u64,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            format!(
                "{} translator only translates responses responses",
                provider.label()
            ),
        );
    }

    let value: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                format!("failed to parse chat completions response JSON: {error}"),
            );
        }
    };

    #[cfg(feature = "mojo")]
    {
        let mut document = Document::default();
        document.openai_chat_context(&value, Some(now_secs));
        let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
        let body = prodex_mojo_core::json::transform_openai_chat_response(&document.nodes, raw)
            .unwrap_or_else(|error| {
                panic!("Mojo OpenAI chat response transform failed: {error:?}")
            });
        ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            body,
        )
    }
    #[cfg(not(feature = "mojo"))]
    {
        let _ = (value, now_secs);
        ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            "OpenAI chat response translation requires Mojo support",
        )
    }
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(all(test, feature = "mojo"))]
#[path = "openai_chat_compat_response/mojo_tests.rs"]
mod mojo_tests;

#[cfg(test)]
mod tests {
    use super::{
        ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformResult,
        ProviderWireFormat, translate_chat_response_to_responses,
        translate_chat_stream_event_to_responses,
    };
    use crate::ProviderTransformLoss;

    fn response(endpoint: ProviderEndpoint, body: &[u8]) -> ProviderTransformResult {
        translate_chat_response_to_responses(
            ProviderId::Anthropic,
            ProviderTransformInput::new(endpoint, body),
        )
    }

    fn stream(endpoint: ProviderEndpoint, event: &str) -> ProviderTransformResult {
        translate_chat_stream_event_to_responses(
            ProviderId::Anthropic,
            ProviderTransformInput::new(endpoint, event.as_bytes()),
        )
    }

    #[test]
    fn response_endpoint_and_json_validation_precede_feature_support() {
        let wrong_endpoint = response(ProviderEndpoint::ChatCompletions, b"{bad}");
        assert!(matches!(
            wrong_endpoint.loss,
            ProviderTransformLoss::UnsupportedUpstream { .. }
        ));
        assert_eq!(wrong_endpoint.endpoint, ProviderEndpoint::ChatCompletions);
        assert_eq!(
            wrong_endpoint.from_format,
            ProviderWireFormat::OpenAiChatCompletions
        );
        assert_eq!(
            wrong_endpoint.to_format,
            ProviderWireFormat::OpenAiResponses
        );

        let malformed = response(ProviderEndpoint::Responses, b"{bad}");
        assert!(matches!(
            malformed.loss,
            ProviderTransformLoss::Rejected { .. }
        ));
        assert_eq!(malformed.endpoint, ProviderEndpoint::Responses);
        assert_eq!(malformed.body, None);
    }

    #[test]
    fn stream_framing_and_json_errors_keep_their_precedence() {
        let wrong_endpoint = stream(ProviderEndpoint::ChatCompletions, "data: {bad}\n\n");
        assert!(matches!(
            wrong_endpoint.loss,
            ProviderTransformLoss::UnsupportedUpstream { .. }
        ));

        let malformed_frame = stream(ProviderEndpoint::Responses, "data: {bad}");
        assert_eq!(
            malformed_frame.loss,
            ProviderTransformLoss::UnsupportedUpstream {
                reason: "chat completions SSE event must use data: <json> framing".into(),
            }
        );

        let malformed_json = stream(ProviderEndpoint::Responses, "data: {bad}\n\n");
        assert!(matches!(
            malformed_json.loss,
            ProviderTransformLoss::Rejected { .. }
        ));
        assert_eq!(malformed_json.body, None);
    }

    #[cfg(not(feature = "mojo"))]
    #[test]
    fn valid_response_and_stream_translation_require_mojo() {
        let response = response(ProviderEndpoint::Responses, br#"{"choices":[]}"#);
        assert_eq!(
            response.loss,
            ProviderTransformLoss::UnsupportedUpstream {
                reason: "OpenAI chat response translation requires Mojo support".into(),
            }
        );
        assert_eq!(response.endpoint, ProviderEndpoint::Responses);
        assert_eq!(
            response.from_format,
            ProviderWireFormat::OpenAiChatCompletions
        );
        assert_eq!(response.to_format, ProviderWireFormat::OpenAiResponses);
        assert_eq!(response.body, None);

        for event in [
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n",
            "data: [DONE]\n\n",
        ] {
            let result = stream(ProviderEndpoint::Responses, event);
            assert_eq!(
                result.loss,
                ProviderTransformLoss::UnsupportedUpstream {
                    reason: "OpenAI chat stream translation requires Mojo support".into(),
                }
            );
            assert_eq!(result.endpoint, ProviderEndpoint::Responses);
            assert_eq!(
                result.from_format,
                ProviderWireFormat::OpenAiChatCompletions
            );
            assert_eq!(result.to_format, ProviderWireFormat::OpenAiResponses);
            assert_eq!(result.body, None);
        }
    }

    #[cfg(feature = "mojo")]
    #[test]
    fn valid_stream_without_supported_delta_remains_unsupported() {
        let result = stream(
            ProviderEndpoint::Responses,
            "data: {\"choices\":[{\"delta\":{}}]}\n\n",
        );
        assert_eq!(
            result.loss,
            ProviderTransformLoss::UnsupportedUpstream {
                reason: "chat completions SSE event does not contain a supported text delta".into(),
            }
        );
    }
}
