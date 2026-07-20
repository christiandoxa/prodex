use super::chat_compatible_request::runtime_provider_chat_compatible_request_body;
use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekRewriteOptions, RuntimeDeepSeekWebSearchMode,
    runtime_deepseek_store_conversation,
};
use super::local_rewrite::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
};
use super::local_rewrite_gemini_compact::{
    runtime_gemini_compact_response_parts, runtime_gemini_local_compact_response_parts,
};
use super::local_rewrite_upstream::RuntimeLocalRewriteStreamingResponse;
use super::provider_bridge::runtime_provider_stream_function_call_arguments_delta_event;
use super::provider_bridge::{RuntimeProviderBridgeKind, runtime_provider_canonical_model};
use super::provider_sse_events::{
    runtime_provider_sse_event, runtime_provider_sse_output_text_item_added_event,
    runtime_provider_sse_output_text_item_done_event,
};
use crate::profile_commands::{
    create_private_kiro_temp_root, read_kiro_auth_secret, write_kiro_cli_data_dir,
};
use crate::runtime_anthropic::{
    RuntimeAnthropicMessagesRequest, build_runtime_anthropic_error_parts,
    translate_runtime_anthropic_messages_request, translate_runtime_responses_reply_to_anthropic,
};
use crate::runtime_kiro_acp::{
    RuntimeKiroAcpClientInfo, RuntimeKiroAcpEnvelope, RuntimeKiroAcpPromptTurnResult,
    RuntimeKiroAcpSessionNotification, RuntimeKiroAcpSessionUpdate,
    runtime_kiro_acp_chat_assistant_messages_from_prompt_turn, runtime_kiro_acp_initialize_request,
    runtime_kiro_acp_prompt_turn_with_command_and_options,
    runtime_kiro_acp_responses_value_from_prompt_turn, runtime_kiro_acp_session_new_request,
    runtime_kiro_acp_session_prompt_request,
};
use crate::runtime_proxy_shared::{RuntimeResponsesReply, RuntimeStreamingResponse};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest};
use anyhow::{Context, Result};
#[cfg(test)]
use prodex_provider_core::kiro_provider_core_responses_items_from_chat_message as runtime_kiro_responses_items_from_chat_message;
use prodex_provider_core::{
    ProviderEndpoint,
    kiro_provider_core_chat_completion_finish_reason as runtime_kiro_chat_completion_finish_reason,
    kiro_provider_core_chat_completion_value_from_response as runtime_kiro_chat_completion_value_from_response,
    kiro_provider_core_prompt_from_chat_messages as runtime_kiro_prompt_from_messages,
    kiro_provider_core_stream_content_text as runtime_kiro_content_text,
    kiro_provider_core_stream_tool_call_item as runtime_kiro_stream_tool_call_item,
};
use prodex_provider_spi::{ProviderStreamMode, ProviderStreamMode::Streaming};
use runtime_proxy_crate::path_without_query;
use serde_json::Value;
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime as TokioRuntime;

fn runtime_kiro_rewrite_options() -> RuntimeDeepSeekRewriteOptions {
    RuntimeDeepSeekRewriteOptions {
        web_search_mode: RuntimeDeepSeekWebSearchMode::OpenAiChat,
        ..Default::default()
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeKiroProfileAuth {
    pub(crate) profile_name: String,
    pub(crate) codex_home: PathBuf,
    pub(crate) model_catalog: Vec<serde_json::Value>,
    pub(crate) command: Option<PathBuf>,
}

pub(super) fn runtime_kiro_model_catalog_from_provider(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Vec<serde_json::Value> {
    let RuntimeLocalRewriteProviderOptions::Kiro { auth } = provider else {
        return Vec::new();
    };
    auth.model_catalog.clone()
}

pub(super) fn runtime_kiro_models_buffered_response(
    auth: &RuntimeKiroProfileAuth,
    method: &str,
    path_and_query: &str,
) -> Option<RuntimeHeapTrimmedBufferedResponseParts> {
    if !method.eq_ignore_ascii_case("GET") {
        return None;
    }
    let path = path_without_query(path_and_query);
    if path.ends_with("/models") {
        return Some(runtime_kiro_json_parts(
            200,
            prodex_provider_core::kiro_provider_core_model_list_value(&auth.model_catalog),
        ));
    }
    let model_id = path.split("/models/").nth(1)?.trim();
    if model_id.is_empty() {
        return None;
    }
    let (status, body) = prodex_provider_core::kiro_provider_core_model_value_or_not_found(
        &auth.model_catalog,
        model_id,
    );
    Some(runtime_kiro_json_parts(status, body))
}

pub(super) fn send_runtime_kiro_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeKiroProfileAuth,
    endpoint: ProviderEndpoint,
    stream_mode: ProviderStreamMode,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let path = path_without_query(&request.path_and_query);
    let chat_completions_route = endpoint == ProviderEndpoint::ChatCompletions;
    let messages_route = endpoint == ProviderEndpoint::Messages;
    if !(endpoint == ProviderEndpoint::Responses || chat_completions_route || messages_route) {
        return Ok(RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Buffered(runtime_kiro_json_parts(
                501,
                prodex_provider_core::kiro_provider_core_unsupported_path_error_value(path),
            )),
            gemini_context: None,
            copilot_context: None,
        });
    }
    let conversations = shared.deepseek_conversations_for_request(request);
    let anthropic_request = if messages_route {
        match translate_runtime_anthropic_messages_request(request) {
            Ok(translated) => Some(translated),
            Err(err) => {
                return Ok(RuntimeLocalRewriteUpstreamResult {
                    response: RuntimeLocalRewriteUpstreamResponse::Buffered(
                        build_runtime_anthropic_error_parts(
                            400,
                            "invalid_request_error",
                            &err.to_string(),
                        ),
                    ),
                    gemini_context: None,
                    copilot_context: None,
                });
            }
        }
    } else {
        None
    };
    let body = anthropic_request
        .as_ref()
        .map(|translated| translated.translated_request.body.clone())
        .unwrap_or(body);
    let body = match runtime_kiro_request_body_for_endpoint(endpoint, body) {
        Ok(body) => body,
        Err(parts) => {
            return Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                gemini_context: None,
                copilot_context: None,
            });
        }
    };

    let value: Value =
        serde_json::from_slice(&body).context("failed to parse Codex Responses request JSON")?;
    let stream = stream_mode == Streaming;

    let translated = runtime_provider_chat_compatible_request_body(
        &body,
        &conversations,
        RuntimeProviderBridgeKind::Kiro,
        "",
        false,
        runtime_kiro_rewrite_options(),
    )?;
    let prompt_messages = translated.messages.clone();
    let prompt = runtime_kiro_prompt_from_messages(&translated.messages);
    let requested_model = value
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(|model| runtime_provider_canonical_model(RuntimeProviderBridgeKind::Kiro, model));
    let requested_effort = value
        .pointer("/reasoning/effort")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|effort| !effort.is_empty())
        .map(str::to_string);
    if stream {
        let response =
            RuntimeLocalRewriteUpstreamResponse::Streaming(RuntimeLocalRewriteStreamingResponse {
                status: 200,
                headers: vec![(
                    "content-type".to_string(),
                    "text/event-stream; charset=utf-8".to_string(),
                )],
                body: Box::new(runtime_kiro_streaming_reader(
                    request_id,
                    prompt,
                    prompt_messages,
                    auth,
                    requested_model,
                    requested_effort,
                    chat_completions_route,
                    shared,
                    conversations.clone(),
                )?),
                profile_name: auth.profile_name.clone(),
            });
        let response = if let Some(anthropic_request) = anthropic_request.as_ref() {
            runtime_kiro_anthropic_streaming_local_response(
                response,
                anthropic_request,
                request_id,
                &shared.runtime_shared,
            )?
        } else {
            response
        };
        return Ok(RuntimeLocalRewriteUpstreamResult {
            response,
            gemini_context: None,
            copilot_context: None,
        });
    }
    let overlay_root = create_private_kiro_temp_root("runtime")?;
    let result = (|| {
        let secret = read_kiro_auth_secret(&auth.codex_home)?;
        let data_dir = overlay_root.join("kiro-data");
        write_kiro_cli_data_dir(&data_dir, &secret)?;
        let mut extra_env = crate::kiro_cli_data_dir_env(&data_dir);
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
            .as_deref()
            .map(Path::as_os_str)
            .unwrap_or(default_command.as_os_str());
        let turn = runtime_kiro_acp_prompt_turn_with_command_and_options(
            command,
            &cwd,
            &extra_env,
            requested_model.as_deref(),
            requested_effort.as_deref(),
            &prompt,
        )?;
        let mut response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, request_id);
        prodex_provider_core::kiro_provider_core_apply_response_runtime_metadata(
            &mut response,
            &auth.profile_name,
            requested_model.as_deref(),
            None,
        );
        if response.get("status").and_then(Value::as_str) != Some("failed")
            && let Some(response_id) = response.get("id").and_then(Value::as_str)
        {
            runtime_deepseek_store_conversation(
                &conversations,
                response_id,
                translated.messages,
                runtime_kiro_acp_chat_assistant_messages_from_prompt_turn(&turn),
            );
        }
        let body = if chat_completions_route {
            serde_json::to_vec(&runtime_kiro_chat_completion_value_from_response(
                &response, request_id,
            ))
            .context("failed to serialize Kiro chat completion JSON")?
        } else {
            serde_json::to_vec(&response).context("failed to serialize Kiro response JSON")?
        };
        let response = RuntimeLocalRewriteUpstreamResponse::Buffered(
            RuntimeHeapTrimmedBufferedResponseParts {
                status: 200,
                headers: vec![(
                    "content-type".to_string(),
                    b"application/json; charset=utf-8".to_vec(),
                )],
                body: body.into(),
            },
        );
        let response = if let Some(anthropic_request) = anthropic_request.as_ref() {
            RuntimeLocalRewriteUpstreamResponse::Buffered(
                runtime_kiro_anthropic_message_parts_from_response(&response, anthropic_request),
            )
        } else {
            response
        };
        Ok(RuntimeLocalRewriteUpstreamResult {
            response,
            gemini_context: None,
            copilot_context: None,
        })
    })();
    schedule_runtime_kiro_overlay_cleanup(&shared.runtime_shared.async_runtime, overlay_root);
    result
}

pub(super) fn runtime_kiro_compact_response_parts(
    request_id: u64,
    body: &[u8],
    async_runtime: &Arc<TokioRuntime>,
    auth: &RuntimeKiroProfileAuth,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    match runtime_kiro_semantic_compact_summary(request_id, body, async_runtime, auth) {
        Ok(summary) => runtime_gemini_compact_response_parts(&summary),
        Err(_) => runtime_gemini_local_compact_response_parts(body),
    }
}

fn runtime_kiro_semantic_compact_summary(
    request_id: u64,
    body: &[u8],
    async_runtime: &Arc<TokioRuntime>,
    auth: &RuntimeKiroProfileAuth,
) -> Result<String> {
    let rewritten = prodex_provider_core::kiro_provider_core_semantic_compact_request_body(body)
        .map_err(anyhow::Error::msg)?;
    let value: Value = serde_json::from_slice(&rewritten)
        .context("failed to parse rewritten Kiro compact request JSON")?;

    let translated = runtime_provider_chat_compatible_request_body(
        &rewritten,
        &RuntimeDeepSeekConversationStore::default(),
        RuntimeProviderBridgeKind::Kiro,
        "",
        false,
        runtime_kiro_rewrite_options(),
    )?;
    let prompt = runtime_kiro_prompt_from_messages(&translated.messages);
    let overlay_root = create_private_kiro_temp_root("compact")?;
    let result = (|| {
        let secret = read_kiro_auth_secret(&auth.codex_home)?;
        let data_dir = overlay_root.join("kiro-data");
        write_kiro_cli_data_dir(&data_dir, &secret)?;
        let mut extra_env = crate::kiro_cli_data_dir_env(&data_dir);
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
            .as_deref()
            .map(Path::as_os_str)
            .unwrap_or(default_command.as_os_str());
        let requested_model = value
            .get("model")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .map(|model| runtime_provider_canonical_model(RuntimeProviderBridgeKind::Kiro, model));
        let requested_effort = value
            .pointer("/reasoning/effort")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|effort| !effort.is_empty());
        let turn = runtime_kiro_acp_prompt_turn_with_command_and_options(
            command,
            &cwd,
            &extra_env,
            requested_model.as_deref(),
            requested_effort,
            &prompt,
        )?;
        let response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, request_id);
        prodex_provider_core::kiro_provider_core_compact_summary_from_response(&response)
            .map_err(anyhow::Error::msg)
    })();
    schedule_runtime_kiro_overlay_cleanup(async_runtime, overlay_root);
    result
}

fn schedule_runtime_kiro_overlay_cleanup(async_runtime: &Arc<TokioRuntime>, overlay_root: PathBuf) {
    schedule_runtime_kiro_blocking_work(async_runtime, move || {
        runtime_kiro_remove_overlay(overlay_root);
    });
}

fn runtime_kiro_remove_overlay(overlay_root: PathBuf) {
    let _ = fs::remove_dir_all(overlay_root);
}

fn schedule_runtime_kiro_blocking_work(
    async_runtime: &Arc<TokioRuntime>,
    work: impl FnOnce() + Send + 'static,
) {
    drop(async_runtime.spawn_blocking(work));
}

fn runtime_kiro_request_body_for_endpoint(
    endpoint: ProviderEndpoint,
    body: Vec<u8>,
) -> std::result::Result<Vec<u8>, RuntimeHeapTrimmedBufferedResponseParts> {
    if endpoint != ProviderEndpoint::ChatCompletions {
        return Ok(body);
    }
    prodex_provider_core::kiro_provider_core_chat_completions_request_body(&body)
        .map_err(|error| runtime_kiro_invalid_request_error(&error.message, &error.code))
}

fn runtime_kiro_invalid_request_error(
    message: &str,
    code: &str,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    runtime_kiro_json_parts(
        400,
        prodex_provider_core::kiro_provider_core_invalid_request_error_value(message, code),
    )
}

fn runtime_kiro_json_parts(status: u16, body: Value) -> RuntimeHeapTrimmedBufferedResponseParts {
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    }
}

fn runtime_kiro_anthropic_message_parts_from_response(
    response: &RuntimeLocalRewriteUpstreamResponse,
    anthropic_request: &RuntimeAnthropicMessagesRequest,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let RuntimeLocalRewriteUpstreamResponse::Buffered(parts) = response else {
        return build_runtime_anthropic_error_parts(
            500,
            "api_error",
            "Kiro Anthropic messages translation expected a buffered response",
        );
    };
    let value: Value = match serde_json::from_slice(&parts.body) {
        Ok(value) => value,
        Err(_) => {
            return build_runtime_anthropic_error_parts(
                502,
                "api_error",
                "Kiro provider returned an invalid JSON response",
            );
        }
    };
    runtime_kiro_json_parts(
        200,
        prodex_provider_core::kiro_provider_core_anthropic_message_value_from_response(
            &value,
            &anthropic_request.requested_model,
        ),
    )
}

fn runtime_kiro_anthropic_streaming_local_response(
    response: RuntimeLocalRewriteUpstreamResponse,
    anthropic_request: &RuntimeAnthropicMessagesRequest,
    request_id: u64,
    runtime_shared: &crate::RuntimeRotationProxyShared,
) -> Result<RuntimeLocalRewriteUpstreamResponse> {
    let RuntimeLocalRewriteUpstreamResponse::Streaming(streaming) = response else {
        return Ok(RuntimeLocalRewriteUpstreamResponse::Buffered(
            build_runtime_anthropic_error_parts(
                500,
                "api_error",
                "Kiro Anthropic messages streaming translation expected a streaming response",
            ),
        ));
    };
    let translated = translate_runtime_responses_reply_to_anthropic(
        RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
            status: streaming.status,
            headers: streaming.headers,
            body: streaming.body,
            request_id,
            profile_name: streaming.profile_name,
            log_path: runtime_shared.log_path.clone(),
            shared: runtime_shared.clone(),
            _inflight_guard: None,
        }),
        anthropic_request,
        request_id,
        runtime_shared,
    )?;
    Ok(match translated {
        RuntimeResponsesReply::Buffered(parts) => {
            RuntimeLocalRewriteUpstreamResponse::Buffered(parts)
        }
        RuntimeResponsesReply::Streaming(streaming) => {
            RuntimeLocalRewriteUpstreamResponse::Streaming(RuntimeLocalRewriteStreamingResponse {
                status: streaming.status,
                headers: streaming.headers,
                body: streaming.body,
                profile_name: streaming.profile_name,
            })
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn runtime_kiro_streaming_reader(
    request_id: u64,
    prompt: String,
    prompt_messages: Vec<Value>,
    auth: &RuntimeKiroProfileAuth,
    requested_model: Option<String>,
    requested_effort: Option<String>,
    chat_completions_route: bool,
    shared: &RuntimeLocalRewriteProxyShared,
    conversations: RuntimeDeepSeekConversationStore,
) -> Result<RuntimeKiroStreamingReader> {
    let secret = read_kiro_auth_secret(&auth.codex_home)?;
    let overlay_root = create_private_kiro_temp_root("runtime")?;
    let data_dir = overlay_root.join("kiro-data");
    write_kiro_cli_data_dir(&data_dir, &secret)?;
    let mut extra_env = crate::kiro_cli_data_dir_env(&data_dir);
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
    let async_runtime = shared.runtime_shared.async_runtime.clone();
    let (sender, receiver) = mpsc::channel();
    let error_sender = sender.clone();
    schedule_runtime_kiro_blocking_work(&async_runtime, move || {
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
            requested_effort,
            chat_completions_route,
            conversations,
        );
        runtime_kiro_remove_overlay(overlay_root);
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
    requested_effort: Option<String>,
    chat_completions_route: bool,
    conversations: RuntimeDeepSeekConversationStore,
) -> Result<()> {
    let mut acp_command = Command::new(command);
    acp_command.arg("acp");
    if let Some(model) = requested_model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        acp_command.arg("--model").arg(model);
    }
    if let Some(effort) = requested_effort
        .as_deref()
        .map(str::trim)
        .filter(|effort| !effort.is_empty())
    {
        acp_command.arg("--effort").arg(effort);
    }
    let mut child = acp_command
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(extra_env.iter().cloned())
        .spawn()
        .with_context(|| format!("failed to start Kiro ACP agent {}", command.display()))?;
    let result = (|| -> Result<()> {
        let mut stdin = BufWriter::new(
            child
                .stdin
                .take()
                .context("failed to capture Kiro ACP stdin")?,
        );
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

        let stdout = child
            .stdout
            .take()
            .context("failed to capture Kiro ACP stdout")?;
        let mut stderr = child
            .stderr
            .take()
            .context("failed to capture Kiro ACP stderr")?;
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
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "kiro-cli".to_string());

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
                            prodex_provider_core::kiro_provider_core_chat_completion_role_delta(),
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
                            &sender,
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

        if message_item_open {
            sequence_number += 1;
            sender.send(RuntimeKiroStreamingChunk::Data(
                runtime_provider_sse_output_text_item_done_event(
                    sequence_number,
                    &response_id,
                    &message_item_id,
                    &assistant_text,
                )
                .into_bytes(),
            ))?;
        }

        let stderr_text = runtime_kiro_read_stderr(&mut stderr);
        let initialize = initialize.with_context(|| {
            format!(
                "Kiro ACP streaming turn did not return initialize result{}",
                runtime_kiro_stderr_suffix(&stderr_text)
            )
        })?;
        let session = session.with_context(|| {
            format!(
                "Kiro ACP streaming turn did not return session/new result{}",
                runtime_kiro_stderr_suffix(&stderr_text)
            )
        })?;
        let prompt_response = prompt_response.with_context(|| {
            format!(
                "Kiro ACP streaming turn did not return session/prompt response{}",
                runtime_kiro_stderr_suffix(&stderr_text)
            )
        })?;
        let turn = RuntimeKiroAcpPromptTurnResult {
            initialize,
            session,
            prompt_response,
            notifications,
        };
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
            let has_tool_calls =
                prodex_provider_core::kiro_provider_core_response_has_tool_calls(&response);
            sender.send(RuntimeKiroStreamingChunk::Data(
                runtime_kiro_chat_completion_chunk(
                    &chat_completion_id,
                    None,
                    prodex_provider_core::kiro_provider_core_chat_completion_empty_delta(),
                    Some(runtime_kiro_chat_completion_finish_reason(
                        &response,
                        has_tool_calls,
                    )),
                )?,
            ))?;
            sender.send(RuntimeKiroStreamingChunk::Data(
                b"data: [DONE]\n\n".to_vec(),
            ))?;
        } else {
            sequence_number += 1;
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

#[allow(clippy::too_many_arguments)]
fn runtime_kiro_stream_notification(
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
                super::provider_bridge::RuntimeProviderBridgeKind::DeepSeek,
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

fn runtime_kiro_chat_completion_chunk(
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

fn runtime_kiro_created_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn runtime_kiro_read_stderr(stderr: &mut impl Read) -> String {
    let mut marker = [0_u8; 1];
    stderr
        .read(&mut marker)
        .ok()
        .filter(|read| *read != 0)
        .map(|_| "present".to_string())
        .unwrap_or_default()
}

fn runtime_kiro_stderr_suffix(stderr: &str) -> String {
    let stderr = stderr.trim();
    if stderr.is_empty() {
        String::new()
    } else {
        "; subprocess reported an error".to_string()
    }
}

enum RuntimeKiroStreamingChunk {
    Data(Vec<u8>),
    Error(io::Error),
    End,
}

struct RuntimeKiroStreamingReader {
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

#[cfg(test)]
mod tests;
