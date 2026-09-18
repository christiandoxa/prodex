//! Pure DeepSeek simple-request eligibility probe.

pub fn deepseek_provider_core_simple_request(
    body: &[u8],
    has_stored_previous_response_id: impl FnMut(&str) -> bool,
) -> bool {
    let mut has_stored_previous_response_id = has_stored_previous_response_id;
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    let previous_response_bound = value
        .get("previous_response_id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .is_some_and(&mut has_stored_previous_response_id);
    super::request_policy::plan_bytes(
        body,
        prodex_mojo_core::rich::DeepSeekRequestPolicyOperation::SimpleRequest,
        previous_response_bound,
    )
    .is_some_and(|plan| plan.tag == 0)
}
