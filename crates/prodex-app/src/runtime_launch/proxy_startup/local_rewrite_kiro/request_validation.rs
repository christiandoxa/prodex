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
    runtime_kiro_request_body_for_endpoint_with_tool_ownership(
        endpoint,
        body,
        runtime_kiro_sub_agent_marker_present(),
    )
}

#[cfg(not(test))]
fn runtime_kiro_sub_agent_marker_present() -> bool {
    std::env::var_os("PRODEX_SUB_AGENT").is_some()
}

#[cfg(test)]
fn runtime_kiro_sub_agent_marker_present() -> bool {
    false
}

#[cfg(test)]
pub(super) fn runtime_kiro_request_body_for_endpoint_with_sub_agent(
    endpoint: ProviderEndpoint,
    body: Vec<u8>,
    sub_agent: bool,
) -> std::result::Result<Vec<u8>, RuntimeHeapTrimmedBufferedResponseParts> {
    runtime_kiro_request_body_for_endpoint_with_tool_ownership(endpoint, body, sub_agent)
}

fn runtime_kiro_request_body_for_endpoint_with_tool_ownership(
    endpoint: ProviderEndpoint,
    body: Vec<u8>,
    sub_agent: bool,
) -> std::result::Result<Vec<u8>, RuntimeHeapTrimmedBufferedResponseParts> {
    let body = strip_kiro_external_tool_controls(body, sub_agent)?;
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

fn strip_kiro_external_tool_controls(
    body: Vec<u8>,
    sub_agent: bool,
) -> std::result::Result<Vec<u8>, RuntimeHeapTrimmedBufferedResponseParts> {
    let mut value: Value = serde_json::from_slice(&body)
        .map_err(|_| invalid_request("Kiro request body must be valid JSON", "invalid_json"))?;
    let Some(object) = value.as_object_mut() else {
        return Err(invalid_request(
            "Kiro request body must be a JSON object",
            "invalid_request_body",
        ));
    };
    let fields = if sub_agent {
        [
            "parallel_tool_calls",
            "tool_choice",
            "tools",
            "functions",
            "function_call",
            "web_search_options",
        ]
        .as_slice()
    } else {
        ["tools", "functions"].as_slice()
    };
    for field in fields {
        object.remove(*field);
    }
    serde_json::to_vec(&value).map_err(|_| {
        invalid_request(
            "failed to serialize rewritten Kiro request body",
            "invalid_request_body",
        )
    })
}

fn invalid_request(message: &str, code: &str) -> RuntimeHeapTrimmedBufferedResponseParts {
    runtime_kiro_json_parts(
        400,
        prodex_provider_core::kiro_provider_core_invalid_request_error_value(message, code),
    )
}
