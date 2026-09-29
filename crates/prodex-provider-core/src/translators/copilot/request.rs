//! Copilot request-shape helpers used by the runtime compatibility shim.

use crate::{
    ProviderId, provider_canonical_model, provider_model_from_request_body,
    provider_request_body_with_model,
};

pub fn copilot_provider_core_request_body_with_canonical_model(body: &[u8]) -> Vec<u8> {
    let Some(model) = provider_model_from_request_body(body) else {
        return body.to_vec();
    };
    let canonical = provider_canonical_model(ProviderId::Copilot, &model);
    provider_request_body_with_model(body, &canonical)
}

pub fn copilot_provider_core_request_body_without_encrypted_content(
    body: &[u8],
) -> (Vec<u8>, bool) {
    match prodex_mojo_core::copilot_request_policy::strip_encrypted_content(body) {
        Ok(Some(stripped)) => (stripped, true),
        Ok(None) | Err(_) => (body.to_vec(), false),
    }
}

pub fn copilot_provider_core_request_has_agent_input(body: &[u8]) -> bool {
    prodex_mojo_core::copilot_request_policy::has_agent_input(body).unwrap_or(false)
}

pub fn copilot_provider_core_request_has_vision_input(body: &[u8]) -> bool {
    prodex_mojo_core::copilot_request_policy::has_vision_input(body).unwrap_or(false)
}
