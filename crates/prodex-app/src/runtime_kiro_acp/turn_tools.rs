use super::super::turn_state::RuntimeKiroAcpToolCallState;

pub(super) fn runtime_kiro_acp_responses_tool_call_item(
    tool_call: &RuntimeKiroAcpToolCallState,
) -> serde_json::Value {
    prodex_provider_core::kiro_provider_core_acp_responses_tool_call_item(
        &tool_call.tool_call_id,
        tool_call.title.as_deref(),
        tool_call.status.as_deref(),
        tool_call.kind.as_deref(),
        tool_call.raw_input.as_ref(),
        tool_call.raw_output.as_ref(),
        tool_call.content.as_deref(),
        tool_call.locations.as_deref(),
    )
}

pub(super) fn runtime_kiro_acp_chat_tool_call_item(
    tool_call: &RuntimeKiroAcpToolCallState,
) -> serde_json::Value {
    prodex_provider_core::kiro_provider_core_acp_chat_tool_call_item(
        &tool_call.tool_call_id,
        tool_call.title.as_deref(),
        tool_call.kind.as_deref(),
        tool_call.raw_input.as_ref(),
    )
}
