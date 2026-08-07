use super::{
    Context as _, Result, RuntimeKiroAcpPromptTurnResult, RuntimeKiroAcpSessionNotification,
    RuntimeKiroAcpSessionUpdate, RuntimeKiroStreamingChunk, RuntimeKiroStreamingContext,
    RuntimeKiroStreamingState, SyncSender, Value, runtime_deepseek_store_conversation,
    runtime_kiro_acp_chat_assistant_messages_from_prompt_turn,
    runtime_kiro_acp_responses_value_from_prompt_turn, runtime_kiro_chat_completion_chunk,
    runtime_kiro_chat_completion_finish_reason, runtime_kiro_content_text,
    runtime_provider_sse_event, runtime_provider_sse_output_text_item_added_event,
    runtime_provider_sse_output_text_item_done_event,
};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct RuntimeKiroStreamingActivityState {
    events: usize,
    truncated: bool,
    labels: BTreeMap<String, (String, Option<String>)>,
}

impl RuntimeKiroStreamingActivityState {
    fn next(
        &mut self,
        tool_call_id: &str,
        title: Option<&str>,
        status: Option<&str>,
        kind: Option<&str>,
        initial: bool,
        details_omitted: bool,
    ) -> Option<Value> {
        if self.truncated {
            return None;
        }
        if self.events
            >= prodex_provider_core::KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_EVENTS.saturating_sub(1)
        {
            self.truncated = true;
            self.events += 1;
            return Some(prodex_provider_core::kiro_provider_core_truncated_tool_activity_item());
        }
        let bounded_id = (tool_call_id.len()
            <= prodex_provider_core::KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_ID_BYTES)
            .then_some(tool_call_id);
        let previous = bounded_id
            .and_then(|tool_call_id| self.labels.get(tool_call_id))
            .cloned();
        let title = title
            .map(str::to_string)
            .or_else(|| previous.as_ref().map(|(name, _)| name.clone()));
        let kind = kind
            .map(str::to_string)
            .or_else(|| previous.and_then(|(_, kind)| kind));
        let activity = prodex_provider_core::kiro_provider_core_tool_activity_item(
            title.as_deref(),
            status,
            kind.as_deref(),
            initial,
            details_omitted,
        );
        self.events += 1;
        if let Some(tool_call_id) = bounded_id {
            self.labels.insert(
                tool_call_id.to_string(),
                (
                    activity["name"]
                        .as_str()
                        .unwrap_or("Kiro internal activity")
                        .to_string(),
                    activity["kind"].as_str().map(str::to_string),
                ),
            );
        }
        Some(activity)
    }
}

pub(super) fn runtime_kiro_finish_stream(
    sender: SyncSender<RuntimeKiroStreamingChunk>,
    context: RuntimeKiroStreamingContext<'_>,
    mut state: RuntimeKiroStreamingState,
) -> Result<()> {
    let RuntimeKiroStreamingContext {
        request_id,
        prompt_messages,
        profile_name,
        requested_model,
        chat_completions_route,
        conversations,
        ..
    } = context;
    if state.message_item_open {
        state.sequence_number += 1;
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_provider_sse_output_text_item_done_event(
                state.sequence_number,
                &state.response_id,
                &state.message_item_id,
                &state.assistant_text,
            )
            .into_bytes(),
        ))?;
    }
    let turn = RuntimeKiroAcpPromptTurnResult {
        initialize: state
            .initialize
            .take()
            .context("Kiro ACP streaming turn did not return initialize result")?,
        session: state
            .session
            .take()
            .context("Kiro ACP streaming turn did not return session/new result")?,
        prompt_response: state
            .prompt_response
            .take()
            .context("Kiro ACP streaming turn did not return session/prompt response")?,
        notifications: std::mem::take(&mut state.notifications),
    };
    let mut response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, request_id);
    prodex_provider_core::kiro_provider_core_apply_response_runtime_metadata(
        &mut response,
        profile_name,
        requested_model.as_deref(),
        Some(state.created_at),
    );
    if response.get("status").and_then(Value::as_str) != Some("failed")
        && let Some(response_id) = response.get("id").and_then(Value::as_str)
    {
        runtime_deepseek_store_conversation(
            &conversations,
            response_id,
            prompt_messages,
            runtime_kiro_acp_chat_assistant_messages_from_prompt_turn(&turn),
        );
    }
    runtime_kiro_send_final_stream(&sender, &response, &mut state, chat_completions_route)?;
    sender.send(RuntimeKiroStreamingChunk::End)?;
    Ok(())
}

pub(super) fn runtime_kiro_send_final_stream(
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    response: &Value,
    state: &mut RuntimeKiroStreamingState,
    chat_completions_route: bool,
) -> Result<()> {
    if chat_completions_route {
        let has_tool_calls =
            prodex_provider_core::kiro_provider_core_response_has_tool_calls(response);
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_kiro_chat_completion_chunk(
                &state.chat_completion_id,
                None,
                prodex_provider_core::kiro_provider_core_chat_completion_empty_delta(),
                Some(runtime_kiro_chat_completion_finish_reason(
                    response,
                    has_tool_calls,
                )),
            )?,
        ))?;
        sender.send(RuntimeKiroStreamingChunk::Data(
            b"data: [DONE]\n\n".to_vec(),
        ))?;
    } else {
        state.sequence_number += 1;
        let (event_type, event) = match response.get("status").and_then(Value::as_str) {
            Some("failed") => (
                "response.failed",
                prodex_provider_core::kiro_provider_core_response_failed_event(
                    state.sequence_number,
                    state.created_at,
                    response,
                ),
            ),
            Some("incomplete") => (
                "response.incomplete",
                prodex_provider_core::kiro_provider_core_response_incomplete_event(
                    state.sequence_number,
                    state.created_at,
                    response,
                ),
            ),
            _ => (
                "response.completed",
                prodex_provider_core::kiro_provider_core_response_completed_event(
                    state.sequence_number,
                    state.created_at,
                    response,
                ),
            ),
        };
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_provider_sse_event(event_type, event).into_bytes(),
        ))?;
        sender.send(RuntimeKiroStreamingChunk::Data(
            b"data: [DONE]\r\n\r\n".to_vec(),
        ))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_kiro_stream_notification(
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    notification: &RuntimeKiroAcpSessionNotification,
    response_id: &str,
    chat_completion_id: &str,
    stream_model: &str,
    created_at: u64,
    message_item_id: &str,
    sequence_number: &mut u64,
    message_item_open: &mut bool,
    assistant_text: &mut String,
    tool_activities: &mut RuntimeKiroStreamingActivityState,
    chat_completions_route: bool,
    chat_delta_started: &mut bool,
) -> Result<()> {
    match &notification.update {
        RuntimeKiroAcpSessionUpdate::AgentMessageChunk { content, .. } => {
            let Some(text) = runtime_kiro_content_text(content) else {
                return Ok(());
            };
            runtime_kiro_stream_text(
                sender,
                response_id,
                chat_completion_id,
                stream_model,
                created_at,
                message_item_id,
                sequence_number,
                message_item_open,
                assistant_text,
                &text,
                chat_completions_route,
                chat_delta_started,
            )?;
        }
        RuntimeKiroAcpSessionUpdate::AgentThoughtChunk { content, .. }
            if chat_completions_route =>
        {
            let Some(text) = runtime_kiro_content_text(content) else {
                return Ok(());
            };
            sender.send(RuntimeKiroStreamingChunk::Data(
                runtime_kiro_chat_completion_chunk(
                    chat_completion_id,
                    Some(stream_model),
                    prodex_provider_core::kiro_provider_core_chat_completion_reasoning_delta(
                        &text,
                        !*chat_delta_started,
                    ),
                    None,
                )?,
            ))?;
            *chat_delta_started = true;
        }
        RuntimeKiroAcpSessionUpdate::ToolCall {
            tool_call_id,
            title,
            status,
            content,
            kind,
            raw_input,
            raw_output,
            locations,
            ..
        } => {
            runtime_kiro_stream_tool_activity(
                sender,
                response_id,
                chat_completion_id,
                stream_model,
                created_at,
                message_item_id,
                sequence_number,
                message_item_open,
                assistant_text,
                tool_activities,
                tool_call_id,
                Some(title.as_str()),
                status.as_deref(),
                kind.as_deref(),
                true,
                raw_input.is_some()
                    || raw_output.is_some()
                    || content.as_ref().is_some_and(|items| !items.is_empty())
                    || locations.as_ref().is_some_and(|items| !items.is_empty()),
                chat_completions_route,
                chat_delta_started,
            )?;
        }
        RuntimeKiroAcpSessionUpdate::ToolCallUpdate {
            tool_call_id,
            title,
            status,
            content,
            kind,
            raw_input,
            raw_output,
            locations,
            ..
        } => {
            runtime_kiro_stream_tool_activity(
                sender,
                response_id,
                chat_completion_id,
                stream_model,
                created_at,
                message_item_id,
                sequence_number,
                message_item_open,
                assistant_text,
                tool_activities,
                tool_call_id,
                title.as_deref(),
                status.as_deref(),
                kind.as_deref(),
                false,
                raw_input.is_some()
                    || raw_output.is_some()
                    || content.as_ref().is_some_and(|items| !items.is_empty())
                    || locations.as_ref().is_some_and(|items| !items.is_empty()),
                chat_completions_route,
                chat_delta_started,
            )?;
        }
        _ => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn runtime_kiro_stream_tool_activity(
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    response_id: &str,
    chat_completion_id: &str,
    stream_model: &str,
    created_at: u64,
    message_item_id: &str,
    sequence_number: &mut u64,
    message_item_open: &mut bool,
    assistant_text: &mut String,
    tool_activities: &mut RuntimeKiroStreamingActivityState,
    tool_call_id: &str,
    title: Option<&str>,
    status: Option<&str>,
    kind: Option<&str>,
    initial: bool,
    details_omitted: bool,
    chat_completions_route: bool,
    chat_delta_started: &mut bool,
) -> Result<()> {
    let Some(activity) =
        tool_activities.next(tool_call_id, title, status, kind, initial, details_omitted)
    else {
        return Ok(());
    };
    runtime_kiro_stream_text(
        sender,
        response_id,
        chat_completion_id,
        stream_model,
        created_at,
        message_item_id,
        sequence_number,
        message_item_open,
        assistant_text,
        &prodex_provider_core::kiro_provider_core_tool_activity_text(&activity),
        chat_completions_route,
        chat_delta_started,
    )
}

#[allow(clippy::too_many_arguments)]
fn runtime_kiro_stream_text(
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    response_id: &str,
    chat_completion_id: &str,
    stream_model: &str,
    created_at: u64,
    message_item_id: &str,
    sequence_number: &mut u64,
    message_item_open: &mut bool,
    assistant_text: &mut String,
    text: &str,
    chat_completions_route: bool,
    chat_delta_started: &mut bool,
) -> Result<()> {
    assistant_text.push_str(text);
    if chat_completions_route {
        let include_model = !*chat_delta_started;
        if include_model {
            *chat_delta_started = true;
        }
        let delta = prodex_provider_core::kiro_provider_core_chat_completion_text_delta(
            text,
            include_model,
        );
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_kiro_chat_completion_chunk(
                chat_completion_id,
                include_model.then_some(stream_model),
                delta,
                None,
            )?,
        ))?;
        return Ok(());
    }
    if !*message_item_open {
        *sequence_number += 1;
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_provider_sse_output_text_item_added_event(
                *sequence_number,
                response_id,
                message_item_id,
            )
            .into_bytes(),
        ))?;
        *message_item_open = true;
    }
    *sequence_number += 1;
    sender.send(RuntimeKiroStreamingChunk::Data(
        runtime_provider_sse_event(
            "response.output_text.delta",
            prodex_provider_core::kiro_provider_core_output_text_delta_event(
                *sequence_number,
                created_at,
                response_id,
                text,
            ),
        )
        .into_bytes(),
    ))?;
    Ok(())
}
