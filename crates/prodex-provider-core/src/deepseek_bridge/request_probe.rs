//! DeepSeek simple-request ABI adapter.

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
    // Normalize escaped keys and duplicate members using Serde before the Mojo JSON scan.
    let Ok(source) = serde_json::to_string(&value) else {
        return false;
    };
    prodex_mojo_core::rich::deepseek_request_policy(
        prodex_mojo_core::rich::DeepSeekRequestPolicyOperation::SimpleRequest,
        &source,
        previous_response_bound,
        0,
    )
    .is_ok_and(|plan| plan.tag == 0)
}
