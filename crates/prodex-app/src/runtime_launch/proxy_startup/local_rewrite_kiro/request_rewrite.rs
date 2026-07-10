use super::runtime_kiro_json_parts;
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use crate::runtime_launch::proxy_startup::provider_bridge::{
    RuntimeProviderRouteKind, runtime_provider_route_kind,
};

pub(super) fn runtime_kiro_request_body_for_path(
    path: &str,
    body: Vec<u8>,
) -> std::result::Result<Vec<u8>, RuntimeHeapTrimmedBufferedResponseParts> {
    if !matches!(
        runtime_provider_route_kind(path),
        Some(RuntimeProviderRouteKind::ChatCompletions)
    ) {
        return Ok(body);
    }
    prodex_provider_core::kiro_provider_core_chat_completions_request_body(&body)
        .map_err(|error| runtime_kiro_invalid_request_error(&error.message, &error.code))
}

fn runtime_kiro_invalid_request_error(
    message: &str,
    code: &str,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    runtime_kiro_json_parts(
        400,
        prodex_provider_core::kiro_provider_core_invalid_request_error_value(message, code),
    )
}
