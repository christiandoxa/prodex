//! Chat-completions SSE to Responses event normalization.

use super::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformResult,
    ProviderWireFormat, Value,
};

#[cfg(feature = "mojo")]
fn stream_event_body(
    kind: prodex_mojo_core::rich::OpenAiCompatStreamKind,
    call_id: Option<&str>,
    name: Option<&str>,
    delta: Option<&str>,
) -> Vec<u8> {
    prodex_mojo_core::rich::openai_compat_stream_event(
        prodex_mojo_core::rich::OpenAiCompatStreamInput {
            kind,
            call_id,
            name,
            delta,
        },
    )
    .unwrap_or_else(|error| panic!("Mojo OpenAI compatibility stream event failed: {error:?}"))
}

pub(crate) fn translate_chat_stream_event_to_responses(
    provider: ProviderId,
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            format!(
                "{} translator only translates responses stream events",
                provider.label()
            ),
        );
    }

    let event = String::from_utf8_lossy(&input.body);
    let Some(data) = event
        .strip_prefix("data: ")
        .and_then(|body| body.strip_suffix("\n\n"))
    else {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            "chat completions SSE event must use data: <json> framing",
        );
    };

    if data == "[DONE]" {
        #[cfg(feature = "mojo")]
        {
            return ProviderTransformResult::lossless(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                stream_event_body(
                    prodex_mojo_core::rich::OpenAiCompatStreamKind::Done,
                    None,
                    None,
                    None,
                ),
            );
        }
        #[cfg(not(feature = "mojo"))]
        {
            return ProviderTransformResult::unsupported(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                "OpenAI chat stream translation requires Mojo support",
            );
        }
    }

    let value: Value = match serde_json::from_str(data) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                format!("failed to parse chat completions SSE JSON: {error}"),
            );
        }
    };

    #[cfg(feature = "mojo")]
    {
        let mut document = crate::mojo_json::Document::default();
        document.openai_chat_context(&value, None);
        let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
        match prodex_mojo_core::json::transform_openai_chat_stream_event(&document.nodes, raw) {
            Ok(Some(body)) => ProviderTransformResult::lossless(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                body,
            ),
            Ok(None) => ProviderTransformResult::unsupported(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                "chat completions SSE event does not contain a supported text delta",
            ),
            Err(error) => panic!("Mojo OpenAI compatibility stream event failed: {error:?}"),
        }
    }

    #[cfg(not(feature = "mojo"))]
    {
        let _ = value;
        ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            "OpenAI chat stream translation requires Mojo support",
        )
    }
}

#[cfg(all(test, feature = "mojo"))]
#[path = "stream/mojo_tests.rs"]
mod mojo_tests;
