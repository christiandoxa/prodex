use super::{
    BTreeSet, Context as _, Result, RuntimeKiroAcpPromptTurnResult,
    RuntimeKiroAcpSessionNotification, RuntimeKiroAcpSessionUpdate, RuntimeKiroStreamingChunk,
    RuntimeKiroStreamingContext, RuntimeKiroStreamingState, SyncSender, Value,
    runtime_deepseek_store_conversation, runtime_kiro_acp_chat_assistant_messages_from_prompt_turn,
    runtime_kiro_acp_responses_value_from_prompt_turn, runtime_kiro_chat_completion_chunk,
    runtime_kiro_chat_completion_finish_reason, runtime_kiro_content_text,
    runtime_kiro_stream_tool_call_item, runtime_provider_sse_event,
    runtime_provider_sse_output_text_item_added_event,
    runtime_provider_sse_output_text_item_done_event,
    runtime_provider_stream_function_call_arguments_delta_event,
};

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

fn runtime_kiro_send_final_stream(
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
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_provider_sse_event(
                "response.completed",
                prodex_provider_core::kiro_provider_core_response_completed_event(
                    state.sequence_number,
                    state.created_at,
                    response,
                ),
            )
            .into_bytes(),
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
    added_tool_calls: &mut BTreeSet<String>,
    delta_tool_calls: &mut BTreeSet<String>,
    done_tool_calls: &mut BTreeSet<String>,
    chat_completions_route: bool,
    chat_delta_started: &mut bool,
) -> Result<()> {
    match &notification.update {
        RuntimeKiroAcpSessionUpdate::AgentMessageChunk { content, .. } => {
            let Some(text) = runtime_kiro_content_text(content) else {
                return Ok(());
            };
            if chat_completions_route {
                let include_model = !*chat_delta_started;
                if include_model {
                    *chat_delta_started = true;
                }
                let delta = prodex_provider_core::kiro_provider_core_chat_completion_text_delta(
                    &text,
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
            } else {
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
                assistant_text.push_str(&text);
                *sequence_number += 1;
                sender.send(RuntimeKiroStreamingChunk::Data(
                    runtime_provider_sse_event(
                        "response.output_text.delta",
                        prodex_provider_core::kiro_provider_core_output_text_delta_event(
                            *sequence_number,
                            created_at,
                            response_id,
                            &text,
                        ),
                    )
                    .into_bytes(),
                ))?;
            }
        }
        RuntimeKiroAcpSessionUpdate::ToolCall {
            tool_call_id,
            title,
            status,
            kind,
            raw_input,
            ..
        } => {
            runtime_kiro_stream_tool_call(
                sender,
                response_id,
                chat_completion_id,
                stream_model,
                created_at,
                sequence_number,
                added_tool_calls,
                delta_tool_calls,
                done_tool_calls,
                tool_call_id,
                Some(title.as_str()),
                Some(status.as_str()),
                kind.as_deref(),
                raw_input.as_ref(),
                chat_completions_route,
                chat_delta_started,
            )?;
        }
        RuntimeKiroAcpSessionUpdate::ToolCallUpdate {
            tool_call_id,
            title,
            status,
            kind,
            raw_input,
            ..
        } => {
            runtime_kiro_stream_tool_call(
                sender,
                response_id,
                chat_completion_id,
                stream_model,
                created_at,
                sequence_number,
                added_tool_calls,
                delta_tool_calls,
                done_tool_calls,
                tool_call_id,
                title.as_deref(),
                status.as_deref(),
                kind.as_deref(),
                raw_input.as_ref(),
                chat_completions_route,
                chat_delta_started,
            )?;
        }
        _ => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn runtime_kiro_stream_tool_call(
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    response_id: &str,
    chat_completion_id: &str,
    stream_model: &str,
    _created_at: u64,
    sequence_number: &mut u64,
    added_tool_calls: &mut BTreeSet<String>,
    delta_tool_calls: &mut BTreeSet<String>,
    done_tool_calls: &mut BTreeSet<String>,
    tool_call_id: &str,
    title: Option<&str>,
    status: Option<&str>,
    kind: Option<&str>,
    raw_input: Option<&Value>,
    chat_completions_route: bool,
    chat_delta_started: &mut bool,
) -> Result<()> {
    let item = runtime_kiro_stream_tool_call_item(tool_call_id, title, status, kind, raw_input);
    let arguments = prodex_provider_core::kiro_provider_core_stream_tool_arguments(raw_input);
    if chat_completions_route {
        let should_emit = added_tool_calls.insert(tool_call_id.to_string())
            || (raw_input.is_some() && delta_tool_calls.insert(tool_call_id.to_string()));
        if should_emit {
            let include_model = !*chat_delta_started;
            if include_model {
                *chat_delta_started = true;
            }
            let delta = prodex_provider_core::kiro_provider_core_chat_completion_tool_call_delta(
                tool_call_id,
                item.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool_call"),
                &arguments,
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
        }
        return Ok(());
    }
    if added_tool_calls.insert(tool_call_id.to_string()) {
        *sequence_number += 1;
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_provider_sse_event(
                "response.output_item.added",
                prodex_provider_core::kiro_provider_core_output_item_added_event(
                    *sequence_number,
                    &item,
                ),
            )
            .into_bytes(),
        ))?;
    }
    if delta_tool_calls.insert(tool_call_id.to_string()) {
        *sequence_number += 1;
        let upstream_value =
            prodex_provider_core::kiro_provider_core_tool_call_arguments_delta_chat_value(
                tool_call_id,
                &arguments,
            );
        if let Some((event_name, data)) =
            runtime_provider_stream_function_call_arguments_delta_event(
                super::super::provider_bridge::RuntimeProviderBridgeKind::DeepSeek,
                &upstream_value,
                *sequence_number,
            )
        {
            sender.send(RuntimeKiroStreamingChunk::Data(
                runtime_provider_sse_event(&event_name, data).into_bytes(),
            ))?;
        }
    }
    let terminal = matches!(status, Some("completed" | "failed" | "cancelled"));
    if terminal && done_tool_calls.insert(tool_call_id.to_string()) {
        *sequence_number += 1;
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_provider_sse_event(
                "response.output_item.done",
                prodex_provider_core::kiro_provider_core_output_item_done_event(
                    *sequence_number,
                    response_id,
                    &item,
                ),
            )
            .into_bytes(),
        ))?;
    }
    Ok(())
}
