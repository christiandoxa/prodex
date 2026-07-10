//! Gemini buffered response transform orchestration.

use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::Value;

use super::response::{
    gemini_normalized_response_value, gemini_responses_value_from_generate_value,
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
    let response = gemini_responses_value_from_generate_value(&value);
    ProviderTransformResult::lossless(
        ProviderId::Gemini,
        input.endpoint,
        ProviderWireFormat::GeminiGenerateContent,
        ProviderWireFormat::OpenAiResponses,
        serde_json::to_vec(&response).expect("gemini response serializes"),
    )
}
