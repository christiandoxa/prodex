use super::runtime_kiro_json_parts;
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use prodex_provider_core::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformLoss,
    provider_translator,
};
use serde_json::Value;

pub(super) fn runtime_kiro_request_body_for_endpoint(
    endpoint: ProviderEndpoint,
    body: Vec<u8>,
) -> std::result::Result<Vec<u8>, RuntimeHeapTrimmedBufferedResponseParts> {
    if endpoint == ProviderEndpoint::ChatCompletions {
        return prodex_provider_core::kiro_provider_core_chat_completions_request_body(&body)
            .map_err(|error| invalid_request(&error.message, &error.code));
    }
    let result = provider_translator(ProviderId::Kiro)
        .transform_request(ProviderTransformInput::new(endpoint, body));
    let code = result
        .metadata
        .get("error_code")
        .and_then(Value::as_str)
        .unwrap_or("invalid_request")
        .to_string();
    match result.loss {
        ProviderTransformLoss::Rejected { reason }
        | ProviderTransformLoss::UnsupportedUpstream { reason } => {
            Err(invalid_request(&reason, &code))
        }
        ProviderTransformLoss::Lossless | ProviderTransformLoss::DegradedButSafe { .. } => result
            .body
            .ok_or_else(|| invalid_request("Kiro request body is missing", &code)),
    }
}

fn invalid_request(message: &str, code: &str) -> RuntimeHeapTrimmedBufferedResponseParts {
    runtime_kiro_json_parts(
        400,
        prodex_provider_core::kiro_provider_core_invalid_request_error_value(message, code),
    )
}
