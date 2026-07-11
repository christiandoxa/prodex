use self::turn_tools::{
    runtime_kiro_acp_chat_tool_call_item, runtime_kiro_acp_responses_tool_call_item,
};
use super::turn_state::runtime_kiro_acp_collect_turn_state;
use super::{
    RuntimeKiroAcpEnvelope, RuntimeKiroAcpNewSessionResult, RuntimeKiroAcpPromptTurnResult,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "turn_tools.rs"]
mod turn_tools;

pub(crate) fn runtime_kiro_acp_model_catalog(
    session: &RuntimeKiroAcpNewSessionResult,
) -> Vec<serde_json::Value> {
    session
        .models
        .as_ref()
        .map(|models| {
            models
                .available_models
                .iter()
                .map(|model| {
                    prodex_provider_core::kiro_provider_core_acp_model_value(
                        &model.model_id,
                        &model.name,
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn runtime_kiro_acp_responses_value_from_prompt_turn(
    turn: &RuntimeKiroAcpPromptTurnResult,
    request_id: u64,
) -> serde_json::Value {
    let response_id = format!("resp_kiro_{request_id}");
    let model = turn
        .session
        .models
        .as_ref()
        .map(|models| models.current_model_id.clone())
        .unwrap_or_else(|| "kiro-cli".to_string());
    let turn_state = runtime_kiro_acp_collect_turn_state(turn);

    let mut output = Vec::new();
    if !turn_state.assistant_text.is_empty() {
        output.push(
            prodex_provider_core::kiro_provider_core_acp_assistant_output_message(
                &turn_state.assistant_text,
            ),
        );
    }
    output.extend(
        turn_state
            .tool_calls
            .iter()
            .map(runtime_kiro_acp_responses_tool_call_item),
    );

    let mut response = prodex_provider_core::kiro_provider_core_acp_response_value(
        &response_id,
        runtime_kiro_acp_created_at(),
        &model,
        output,
    );

    if let Some(error) = &turn.prompt_response.error {
        prodex_provider_core::kiro_provider_core_acp_mark_failed_response(
            &mut response,
            error.code,
            &error.message,
        );
    } else if let Some((reason, message)) =
        runtime_kiro_acp_incomplete_details(&turn.prompt_response)
    {
        prodex_provider_core::kiro_provider_core_acp_mark_incomplete_response(
            &mut response,
            reason,
            message,
        );
    }

    let stop_reason = runtime_kiro_acp_prompt_stop_reason(&turn.prompt_response);
    if let Some(metadata) = prodex_provider_core::kiro_provider_core_acp_metadata(
        &turn_state.reasoning_text,
        turn_state.usage_update,
        turn_state.plan_entries,
        turn_state.available_commands,
        turn_state.current_mode_id.as_deref(),
        turn_state.session_title.as_deref(),
        turn_state.session_updated_at.as_deref(),
        stop_reason.as_deref(),
    ) {
        response["metadata"] = metadata;
    }
    response
}

pub(crate) fn runtime_kiro_acp_chat_assistant_messages_from_prompt_turn(
    turn: &RuntimeKiroAcpPromptTurnResult,
) -> Vec<serde_json::Value> {
    let turn_state = runtime_kiro_acp_collect_turn_state(turn);
    prodex_provider_core::kiro_provider_core_acp_chat_assistant_message(
        &turn_state.assistant_text,
        &turn_state.reasoning_text,
        turn_state
            .tool_calls
            .iter()
            .map(runtime_kiro_acp_chat_tool_call_item)
            .collect(),
    )
    .into_iter()
    .collect()
}

fn runtime_kiro_acp_created_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn runtime_kiro_acp_prompt_stop_reason(envelope: &RuntimeKiroAcpEnvelope) -> Option<String> {
    prodex_provider_core::kiro_provider_core_acp_stop_reason(envelope.result.as_ref())
}

fn runtime_kiro_acp_incomplete_details(
    envelope: &RuntimeKiroAcpEnvelope,
) -> Option<(&'static str, &'static str)> {
    prodex_provider_core::kiro_provider_core_acp_incomplete_details(
        runtime_kiro_acp_prompt_stop_reason(envelope).as_deref(),
    )
}
