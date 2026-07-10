use self::events::{
    runtime_kiro_chat_completion_chunk, runtime_kiro_created_at, runtime_kiro_read_stderr,
};
use super::RuntimeKiroProfileAuth;
use super::chat_compat::runtime_kiro_chat_completion_finish_reason;
use super::invoke::runtime_kiro_temp_dir;
use super::streaming_turn::runtime_kiro_stream_prompt_turn;
use crate::profile_commands::{read_kiro_auth_secret, write_kiro_cli_data_dir};
use crate::runtime_kiro_acp::{
    runtime_kiro_acp_chat_assistant_messages_from_prompt_turn,
    runtime_kiro_acp_responses_value_from_prompt_turn,
};
use crate::runtime_launch::proxy_startup::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_store_conversation,
};
use crate::runtime_launch::proxy_startup::local_rewrite::{
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
};
use crate::runtime_launch::proxy_startup::local_rewrite_upstream::RuntimeLocalRewriteStreamingResponse;
use crate::runtime_launch::proxy_startup::provider_sse_events::{
    runtime_provider_sse_event, runtime_provider_sse_output_text_item_done_event,
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[path = "events.rs"]
mod events;

pub(super) fn runtime_kiro_streaming_reader(
    request_id: u64,
    prompt: String,
    prompt_messages: Vec<Value>,
    auth: &RuntimeKiroProfileAuth,
    requested_model: Option<String>,
    chat_completions_route: bool,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeKiroStreamingReader> {
    let secret = read_kiro_auth_secret(&auth.codex_home)?;
    let overlay_root = runtime_kiro_temp_dir("runtime");
    let data_dir = overlay_root.join("kiro-data");
    write_kiro_cli_data_dir(&data_dir, &secret)?;
    let mut extra_env = vec![(OsString::from("Q_CLI_DATA_DIR"), data_dir.into_os_string())];
    if let Some(region) = secret
        .region
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        extra_env.push((OsString::from("AWS_REGION"), OsString::from(region)));
    }
    let cwd = env::current_dir().unwrap_or_else(|_| auth.codex_home.clone());
    let default_command = crate::kiro_bin();
    let command = auth
        .command
        .clone()
        .unwrap_or_else(|| PathBuf::from(default_command));
    let profile_name = auth.profile_name.clone();
    let conversations = shared.deepseek_conversations.clone();
    let (sender, receiver) = mpsc::channel();
    let error_sender = sender.clone();
    thread::spawn(move || {
        let result = runtime_kiro_streaming_worker(
            sender,
            request_id,
            &prompt,
            prompt_messages,
            &cwd,
            &extra_env,
            &command,
            &profile_name,
            requested_model,
            chat_completions_route,
            conversations,
        );
        let _ = fs::remove_dir_all(&overlay_root);
        if let Err(err) = result {
            let _ = error_sender.send(RuntimeKiroStreamingChunk::Error(io::Error::other(
                err.to_string(),
            )));
        }
    });
    Ok(RuntimeKiroStreamingReader {
        receiver,
        pending: Cursor::new(Vec::new()),
        finished: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn runtime_kiro_streaming_worker(
    sender: Sender<RuntimeKiroStreamingChunk>,
    request_id: u64,
    prompt: &str,
    prompt_messages: Vec<Value>,
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
    command: &Path,
    profile_name: &str,
    requested_model: Option<String>,
    chat_completions_route: bool,
    conversations: RuntimeDeepSeekConversationStore,
) -> Result<()> {
    let mut child = Command::new(command)
        .arg("acp")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(extra_env.iter().cloned())
        .spawn()
        .with_context(|| format!("failed to start Kiro ACP agent {}", command.display()))?;
    let result = (|| -> Result<()> {
        let stdin = child
            .stdin
            .take()
            .context("failed to capture Kiro ACP stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("failed to capture Kiro ACP stdout")?;
        let mut stderr = child
            .stderr
            .take()
            .context("failed to capture Kiro ACP stderr")?;
        let turn_result = runtime_kiro_stream_prompt_turn(
            &sender,
            request_id,
            prompt,
            cwd,
            stdin,
            stdout,
            chat_completions_route,
            requested_model.as_deref(),
        )?;
        let response_id = format!("resp_kiro_{request_id}");
        let created_at = turn_result.created_at;

        if turn_result.message_item_open {
            let sequence_number = turn_result.sequence_number + 1;
            sender.send(RuntimeKiroStreamingChunk::Data(
                runtime_provider_sse_output_text_item_done_event(
                    sequence_number,
                    &response_id,
                    &turn_result.message_item_id,
                    &turn_result.assistant_text,
                )
                .into_bytes(),
            ))?;
        }

        let _stderr_text = runtime_kiro_read_stderr(&mut stderr);
        let turn = turn_result.turn;
        let mut response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, request_id);
        prodex_provider_core::kiro_provider_core_apply_response_runtime_metadata(
            &mut response,
            profile_name,
            requested_model.as_deref(),
            Some(created_at),
        );
        if response.get("status").and_then(Value::as_str) != Some("failed")
            && let Some(response_id_value) = response.get("id").and_then(Value::as_str)
        {
            runtime_deepseek_store_conversation(
                &conversations,
                response_id_value,
                prompt_messages,
                runtime_kiro_acp_chat_assistant_messages_from_prompt_turn(&turn),
            );
        }
        if chat_completions_route {
            sender.send(RuntimeKiroStreamingChunk::Data(
                runtime_kiro_chat_completion_chunk(
                    &turn_result.chat_completion_id,
                    None,
                    prodex_provider_core::kiro_provider_core_chat_completion_empty_delta(),
                    Some(runtime_kiro_chat_completion_finish_reason(&response)),
                )?,
            ))?;
            sender.send(RuntimeKiroStreamingChunk::Data(
                b"data: [DONE]\n\n".to_vec(),
            ))?;
        } else {
            let sequence_number =
                turn_result.sequence_number + u64::from(turn_result.message_item_open) + 1;
            sender.send(RuntimeKiroStreamingChunk::Data(
                runtime_provider_sse_event(
                    "response.completed",
                    prodex_provider_core::kiro_provider_core_response_completed_event(
                        sequence_number,
                        created_at,
                        &response,
                    ),
                )
                .into_bytes(),
            ))?;
            sender.send(RuntimeKiroStreamingChunk::Data(
                b"data: [DONE]\r\n\r\n".to_vec(),
            ))?;
        }
        sender.send(RuntimeKiroStreamingChunk::End)?;
        Ok(())
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

pub(super) enum RuntimeKiroStreamingChunk {
    Data(Vec<u8>),
    Error(io::Error),
    End,
}

pub(super) struct RuntimeKiroStreamingReader {
    receiver: Receiver<RuntimeKiroStreamingChunk>,
    pending: Cursor<Vec<u8>>,
    finished: bool,
}

impl Read for RuntimeKiroStreamingReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let read = self.pending.read(buf)?;
            if read > 0 {
                return Ok(read);
            }
            if self.finished {
                return Ok(0);
            }
            match self.receiver.recv() {
                Ok(RuntimeKiroStreamingChunk::Data(bytes)) => {
                    self.pending = Cursor::new(bytes);
                }
                Ok(RuntimeKiroStreamingChunk::Error(err)) => return Err(err),
                Ok(RuntimeKiroStreamingChunk::End) | Err(_) => {
                    self.finished = true;
                    return Ok(0);
                }
            }
        }
    }
}
