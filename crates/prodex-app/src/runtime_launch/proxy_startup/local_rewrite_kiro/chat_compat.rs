use serde_json::Value;

pub(super) fn runtime_kiro_chat_completion_value_from_response(
    response: &Value,
    request_id: u64,
) -> Value {
    prodex_provider_core::kiro_provider_core_chat_completion_value_from_response(
        response, request_id,
    )
}

pub(super) fn runtime_kiro_chat_completion_finish_reason(response: &Value) -> &'static str {
    prodex_provider_core::kiro_provider_core_chat_completion_finish_reason_from_response(response)
}
