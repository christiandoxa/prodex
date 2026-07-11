use crate::runtime_kiro_acp::{RuntimeKiroAcpSessionNotification, RuntimeKiroAcpSessionUpdate};
use crate::runtime_launch::proxy_startup::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_stream_function_call_arguments_delta_event,
};
use crate::runtime_launch::proxy_startup::provider_sse_events::{
    runtime_provider_sse_event, runtime_provider_sse_output_text_item_added_event,
    runtime_provider_sse_output_text_item_done_event,
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::{self, Read};
use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};

use super::RuntimeKiroStreamingChunk;

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_kiro_stream_notification(
    sender: &Sender<RuntimeKiroStreamingChunk>,
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
                let delta = runtime_kiro_chat_completion_text_delta(&text, include_model);
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
    sender: &Sender<RuntimeKiroStreamingChunk>,
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
            let delta = runtime_kiro_chat_completion_tool_call_delta(
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
                RuntimeProviderBridgeKind::DeepSeek,
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

fn runtime_kiro_stream_tool_call_item(
    tool_call_id: &str,
    title: Option<&str>,
    status: Option<&str>,
    kind: Option<&str>,
    raw_input: Option<&Value>,
) -> Value {
    prodex_provider_core::kiro_provider_core_stream_tool_call_item(
        tool_call_id,
        title,
        status,
        kind,
        raw_input,
    )
}

pub(super) fn runtime_kiro_chat_completion_chunk(
    chat_completion_id: &str,
    model: Option<&str>,
    delta: Value,
    finish_reason: Option<&str>,
) -> Result<Vec<u8>> {
    prodex_provider_core::kiro_provider_core_chat_completion_chunk(
        chat_completion_id,
        model,
        delta,
        finish_reason,
    )
    .context("failed to serialize Kiro chat completion chunk")
}

pub(super) fn runtime_kiro_chat_completion_role_delta() -> Value {
    prodex_provider_core::kiro_provider_core_chat_completion_role_delta()
}

pub(super) fn runtime_kiro_chat_completion_text_delta(text: &str, include_role: bool) -> Value {
    prodex_provider_core::kiro_provider_core_chat_completion_text_delta(text, include_role)
}

pub(super) fn runtime_kiro_chat_completion_tool_call_delta(
    tool_call_id: &str,
    name: &str,
    arguments: &str,
    include_role: bool,
) -> Value {
    prodex_provider_core::kiro_provider_core_chat_completion_tool_call_delta(
        tool_call_id,
        name,
        arguments,
        include_role,
    )
}

pub(super) fn runtime_kiro_content_text(value: &Value) -> Option<String> {
    prodex_provider_core::kiro_provider_core_stream_content_text(value)
}

pub(super) fn runtime_kiro_created_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub(super) fn runtime_kiro_read_stderr(stderr: &mut impl Read) -> String {
    let mut stderr_text = String::new();
    let _ = stderr.read_to_string(&mut stderr_text);
    stderr_text
}

pub(super) fn runtime_kiro_stderr_suffix(stderr: &str) -> String {
    let stderr = stderr.trim();
    if stderr.is_empty() {
        String::new()
    } else {
        format!("; stderr: {stderr}")
    }
}
