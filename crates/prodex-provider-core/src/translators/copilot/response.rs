//! Copilot response-shape helpers used by runtime affinity tracking.

use serde_json::Value;

pub fn copilot_provider_core_response_id_from_value(value: &Value) -> Option<String> {
    let body = serde_json::to_vec(value).expect("Serde JSON value serializes");
    prodex_mojo_core::copilot_request_policy::response_id(&body)
        .expect("Mojo Copilot response ID policy returned invalid output")
}
