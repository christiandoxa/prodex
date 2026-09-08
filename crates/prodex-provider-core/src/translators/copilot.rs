mod request;
mod response;

pub use self::request::{
    copilot_provider_core_request_body_with_canonical_model,
    copilot_provider_core_request_body_without_encrypted_content,
    copilot_provider_core_request_has_agent_input, copilot_provider_core_request_has_vision_input,
};
pub use self::response::copilot_provider_core_response_id_from_value;

#[cfg(test)]
#[path = "copilot/tests.rs"]
mod tests;
