use super::streaming::RuntimeKiroStreamingChunk;
use crate::runtime_kiro_acp::{
    RuntimeKiroAcpClientInfo, RuntimeKiroAcpEnvelope, RuntimeKiroAcpPromptTurnResult,
    runtime_kiro_acp_initialize_request, runtime_kiro_acp_session_new_request,
    runtime_kiro_acp_session_prompt_request,
};
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{ChildStdin, ChildStdout};
use std::sync::mpsc::Sender;

#[path = "events.rs"]
mod events;

use self::events::{
    runtime_kiro_chat_completion_chunk, runtime_kiro_chat_completion_role_delta,
    runtime_kiro_created_at, runtime_kiro_stream_notification,
};
use crate::runtime_launch::proxy_startup::provider_sse_events::runtime_provider_sse_event;

pub(super) struct RuntimeKiroStreamingTurnState {
    pub(super) turn: RuntimeKiroAcpPromptTurnResult,
    pub(super) created_at: u64,
    pub(super) sequence_number: u64,
    pub(super) message_item_id: String,
    pub(super) message_item_open: bool,
    pub(super) assistant_text: String,
    pub(super) chat_completion_id: String,
}

pub(super) fn runtime_kiro_stream_prompt_turn(
    sender: &Sender<RuntimeKiroStreamingChunk>,
    request_id: u64,
    prompt: &str,
    cwd: &std::path::Path,
    stdin: ChildStdin,
    stdout: ChildStdout,
    chat_completions_route: bool,
    requested_model: Option<&str>,
) -> Result<RuntimeKiroStreamingTurnState> {
    let mut stdin = BufWriter::new(stdin);
    writeln!(
        stdin,
        "{}",
        runtime_kiro_acp_initialize_request(
            0,
            RuntimeKiroAcpClientInfo {
                name: "prodex",
                title: "Prodex",
                version: env!("CARGO_PKG_VERSION"),
            },
        )
    )
    .context("failed to write Kiro ACP initialize request")?;
    writeln!(stdin, "{}", runtime_kiro_acp_session_new_request(1, cwd))
        .context("failed to write Kiro ACP session/new request")?;
    stdin
        .flush()
        .context("failed to flush initial Kiro ACP streaming requests")?;

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let mut initialize = None;
    let mut session = None;
    let mut prompt_response = None;
    let mut notifications = Vec::new();
    let response_id = format!("resp_kiro_{request_id}");
    let created_at = runtime_kiro_created_at();
    let mut sequence_number = 0_u64;
    let message_item_id = format!("msg_kiro_{request_id}_0");
    let mut message_item_open = false;
    let mut assistant_text = String::new();
    let mut added_tool_calls = BTreeSet::new();
    let mut delta_tool_calls = BTreeSet::new();
    let mut done_tool_calls = BTreeSet::new();
    let mut prompt_sent = false;
    let mut chat_delta_started = false;
    let chat_completion_id = format!("chatcmpl_{response_id}");
    let stream_model = requested_model
        .filter(|s| !s.is_empty())
        .unwrap_or("kiro-cli")
        .to_string();

    while reader
        .read_line(&mut line)
        .context("failed to read Kiro ACP stdout")?
        > 0
    {
        let current = line.trim();
        if current.is_empty() {
            line.clear();
            continue;
        }
        let envelope = RuntimeKiroAcpEnvelope::parse(current)?;
        if !prompt_sent && matches!(envelope.id, Some(1)) && envelope.error.is_none() {
            let parsed_session = envelope.parse_session_new_result()?;
            writeln!(
                stdin,
                "{}",
                runtime_kiro_acp_session_prompt_request(2, &parsed_session.session_id, prompt)
            )
            .context("failed to write Kiro ACP session/prompt request")?;
            stdin
                .flush()
                .context("failed to flush Kiro ACP session/prompt request")?;
            prompt_sent = true;
            session = Some(parsed_session);
            if chat_completions_route {
                sender.send(RuntimeKiroStreamingChunk::Data(
                    runtime_kiro_chat_completion_chunk(
                        &chat_completion_id,
                        Some(&stream_model),
                        runtime_kiro_chat_completion_role_delta(),
                        None,
                    )?,
                ))?;
                chat_delta_started = true;
            } else {
                sender.send(RuntimeKiroStreamingChunk::Data(
                    runtime_provider_sse_event(
                        "response.created",
                        prodex_provider_core::kiro_provider_core_response_created_event(
                            sequence_number,
                            created_at,
                            &response_id,
                        ),
                    )
                    .into_bytes(),
                ))?;
            }
            line.clear();
            continue;
        }
        match envelope.id {
            Some(0) if envelope.error.is_none() => {
                initialize = Some(envelope.parse_initialize_result()?);
            }
            Some(1) if envelope.error.is_none() => {
                session = Some(envelope.parse_session_new_result()?);
            }
            Some(2) => {
                prompt_response = Some(envelope);
                break;
            }
            _ => {
                if let Ok(notification) = envelope.parse_session_notification() {
                    runtime_kiro_stream_notification(
                        sender,
                        &notification,
                        &response_id,
                        &chat_completion_id,
                        &stream_model,
                        created_at,
                        &message_item_id,
                        &mut sequence_number,
                        &mut message_item_open,
                        &mut assistant_text,
                        &mut added_tool_calls,
                        &mut delta_tool_calls,
                        &mut done_tool_calls,
                        chat_completions_route,
                        &mut chat_delta_started,
                    )?;
                }
                notifications.push(envelope);
            }
        }
        line.clear();
    }
    drop(stdin);

    Ok(RuntimeKiroStreamingTurnState {
        turn: RuntimeKiroAcpPromptTurnResult {
            initialize: initialize
                .context("Kiro ACP streaming turn did not return initialize result")?,
            session: session
                .context("Kiro ACP streaming turn did not return session/new result")?,
            prompt_response: prompt_response
                .context("Kiro ACP streaming turn did not return session/prompt response")?,
            notifications,
        },
        created_at,
        sequence_number,
        message_item_id,
        message_item_open,
        assistant_text,
        chat_completion_id,
    })
}
