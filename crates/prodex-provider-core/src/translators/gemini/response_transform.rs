//! Gemini buffered response transform orchestration.

use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::Value;

use super::response::{
    GeminiResponseBuildError, gemini_normalized_response_value,
    gemini_responses_value_from_generate_value,
};

pub(super) fn gemini_transform_response(input: ProviderTransformInput) -> ProviderTransformResult {
    if super::gemini_passthrough_endpoint(input.endpoint) {
        return ProviderTransformResult::lossless(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::GeminiGenerateContent,
            ProviderWireFormat::OpenAiResponses,
            input.body,
        );
    }
    if input.endpoint != ProviderEndpoint::Responses {
        return ProviderTransformResult::unsupported(
            ProviderId::Gemini,
            input.endpoint,
            ProviderWireFormat::GeminiGenerateContent,
            ProviderWireFormat::OpenAiResponses,
            format!(
                "Gemini translator does not support {}",
                input.endpoint.label()
            ),
        );
    }
    let value: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                ProviderId::Gemini,
                input.endpoint,
                ProviderWireFormat::GeminiGenerateContent,
                ProviderWireFormat::OpenAiResponses,
                format!("failed to parse Gemini response JSON: {error}"),
            );
        }
    };
    let value = gemini_normalized_response_value(&value);
    let response = match gemini_responses_value_from_generate_value(&value) {
        Ok(response) => response,
        Err(GeminiResponseBuildError::InputTooLarge) => {
            return ProviderTransformResult::rejected(
                ProviderId::Gemini,
                input.endpoint,
                ProviderWireFormat::GeminiGenerateContent,
                ProviderWireFormat::OpenAiResponses,
                "Gemini response exceeds the buffered response size limit",
            );
        }
        Err(GeminiResponseBuildError::Kernel) => {
            return ProviderTransformResult::rejected(
                ProviderId::Gemini,
                input.endpoint,
                ProviderWireFormat::GeminiGenerateContent,
                ProviderWireFormat::OpenAiResponses,
                "Gemini response normalization failed",
            );
        }
    };
    ProviderTransformResult::lossless(
        ProviderId::Gemini,
        input.endpoint,
        ProviderWireFormat::GeminiGenerateContent,
        ProviderWireFormat::OpenAiResponses,
        serde_json::to_vec(&response).expect("gemini response serializes"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translator::ProviderTransformLoss;

    #[test]
    fn gemini_provider_core_buffered_response_transform_matches_expected_value() {
        let body = serde_json::to_vec(&serde_json::json!({
            "candidates": [{"content": {"parts": [{"text": "hello"}]}}]
        }))
        .expect("Gemini test response serializes");
        let result = gemini_transform_response(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            body,
        ));

        assert_eq!(result.loss, ProviderTransformLoss::Lossless);
        assert_eq!(
            serde_json::from_slice::<Value>(result.body.as_deref().unwrap())
                .expect("translated Gemini response parses"),
            serde_json::json!({
                "id": "gemini_resp_prodex",
                "object": "response",
                "model": "gemini-2.5-pro",
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "hello"}]
                }],
                "usage": {},
                "metadata": {}
            })
        );
    }

    #[test]
    fn gemini_provider_core_buffered_response_transform_rejects_oversized_input() {
        let text = "x".repeat(prodex_mojo_core::rich::GEMINI_BUFFERED_RESPONSE_MAX_INPUT_BYTES + 1);
        let body = serde_json::to_vec(&serde_json::json!({
            "candidates": [{"content": {"parts": [{"text": text}]}}]
        }))
        .expect("Gemini test response serializes");
        let result = gemini_transform_response(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            body,
        ));

        assert_eq!(
            result.loss,
            ProviderTransformLoss::Rejected {
                reason: "Gemini response exceeds the buffered response size limit".to_string(),
            }
        );
        assert!(result.body.is_none());
    }
}
