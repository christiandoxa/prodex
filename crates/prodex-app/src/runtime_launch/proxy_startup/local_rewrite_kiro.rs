mod process;
mod request_validation;
mod response;
mod stream;
use self::process::{
    runtime_kiro_configure_process_group, runtime_kiro_kill_process_group,
    runtime_kiro_streaming_command,
};
use self::response::{runtime_kiro_anthropic_message_parts_from_response, runtime_kiro_json_parts};
use self::stream::{
    RuntimeKiroStreamingActivityState, runtime_kiro_finish_stream, runtime_kiro_stream_notification,
};

use self::request_validation::runtime_kiro_request_body_for_endpoint;
use super::chat_compatible_request::runtime_provider_chat_compatible_request_body;
use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekRewriteOptions, RuntimeDeepSeekWebSearchMode,
    runtime_deepseek_store_conversation,
};
use super::local_rewrite::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
};
use super::local_rewrite_copilot::runtime_copilot_remember_bindings_from_responses_body;
use super::local_rewrite_gemini_compact::{
    runtime_compact_reason, runtime_compact_response_parts,
    runtime_local_compact_response_parts_with_reason,
};
use super::local_rewrite_upstream::RuntimeLocalRewriteStreamingResponse;
use super::local_rewrite_upstream::{
    RuntimeLocalRewriteBindingContext, runtime_local_rewrite_binding_context,
    runtime_local_rewrite_binding_recorder, runtime_local_rewrite_raw_binding_identity,
    runtime_local_rewrite_remember_accepted_binding,
};
use super::provider_bridge::{RuntimeProviderBridgeKind, runtime_provider_canonical_model};
use super::provider_sse_events::{
    runtime_provider_sse_event, runtime_provider_sse_output_text_item_added_event,
    runtime_provider_sse_output_text_item_done_event,
};
use crate::profile_commands::prepare_kiro_cli_data_dir;
use crate::runtime_anthropic::{
    RuntimeAnthropicMessagesRequest, build_runtime_anthropic_error_parts,
    translate_runtime_anthropic_messages_request, translate_runtime_responses_reply_to_anthropic,
};
use crate::runtime_kiro_acp::{
    RuntimeKiroAcpClientInfo, RuntimeKiroAcpEnvelope, RuntimeKiroAcpInitializeResult,
    RuntimeKiroAcpNewSessionResult, RuntimeKiroAcpPromptTurnResult,
    RuntimeKiroAcpSessionNotification, RuntimeKiroAcpSessionUpdate,
    runtime_kiro_acp_chat_assistant_messages_from_prompt_turn, runtime_kiro_acp_initialize_request,
    runtime_kiro_acp_line_receiver, runtime_kiro_acp_prompt_turn_with_command_and_options,
    runtime_kiro_acp_reject_unsupported_server_request,
    runtime_kiro_acp_responses_value_from_prompt_turn, runtime_kiro_acp_session_new_request,
    runtime_kiro_acp_session_prompt_request,
};
use crate::runtime_proxy_shared::{RuntimeResponsesReply, RuntimeStreamingResponse};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest};
use anyhow::{Context, Result};
#[cfg(test)]
use prodex_provider_core::kiro_provider_core_responses_items_from_chat_message as runtime_kiro_responses_items_from_chat_message;
use prodex_provider_core::{
    ProviderEndpoint, ProviderId, RuntimeProviderBindingIdentity,
    kiro_provider_core_chat_completion_finish_reason as runtime_kiro_chat_completion_finish_reason,
    kiro_provider_core_chat_completion_value_from_response as runtime_kiro_chat_completion_value_from_response,
    kiro_provider_core_prompt_from_chat_messages as runtime_kiro_prompt_from_messages,
    kiro_provider_core_stream_content_text as runtime_kiro_content_text,
};
use prodex_provider_spi::{ProviderStreamMode, ProviderStreamMode::Streaming};
use runtime_proxy_crate::path_without_query;
use serde_json::Value;
use std::env;
use std::ffi::OsString;
use std::io::{self, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime as TokioRuntime;

const RUNTIME_KIRO_ACP_STREAM_TOTAL_TIMEOUT: Duration = Duration::from_secs(120);

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
    let model_id = if path.ends_with("/models") {
        None
    } else {
        let model_id = path.split("/models/").nth(1)?.trim();
        if model_id.is_empty() {
            return None;
        }
        Some(model_id)
    };
    let model_catalog = match prodex_provider_core::merge_provider_model_catalog_json(
        ProviderId::Kiro,
        &auth.model_catalog,
    ) {
        Ok(catalog) => catalog,
        Err(error) => {
            return Some(runtime_kiro_json_parts(
                503,
                prodex_provider_core::kiro_provider_core_invalid_request_error_value(
                    &error.to_string(),
                    "model_catalog_limit_exceeded",
                ),
            ));
        }
    };
    let Some(model_id) = model_id else {
        return Some(runtime_kiro_json_parts(
            200,
            prodex_provider_core::kiro_provider_core_model_list_value(&model_catalog),
        ));
    };
    let (status, body) =
        prodex_provider_core::kiro_provider_core_model_value_or_not_found(&model_catalog, model_id);
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
    let binding = runtime_local_rewrite_binding_context(shared, request)?;
    let binding_identity = runtime_local_rewrite_raw_binding_identity(
        shared,
        ProviderId::Kiro,
        None,
        &shared.upstream_base_url,
        Some(auth.profile_name.as_str()),
    )
    .ok_or_else(|| anyhow::anyhow!("Kiro binding identity is unavailable"))?;
    if !binding.candidate_allowed(Some(&binding_identity)) {
        anyhow::bail!("Kiro continuation binding is unavailable or unauthorized");
    }
    let path = path_without_query(&request.path_and_query);
    let chat_completions_route = endpoint == ProviderEndpoint::ChatCompletions;
    let messages_route = endpoint == ProviderEndpoint::Messages;
    if !(endpoint == ProviderEndpoint::Responses || chat_completions_route || messages_route) {
        return Ok(runtime_kiro_unsupported_route_result(path));
    }
    let conversations = shared.deepseek_conversations_for_request(request);
    let anthropic_request = match runtime_kiro_anthropic_request(request, messages_route) {
        Ok(translated) => translated,
        Err(response) => return Ok(*response),
    };
    let body = anthropic_request
        .as_ref()
        .map(|translated| translated.translated_request.body.clone())
        .unwrap_or(body);
    let body = match runtime_kiro_request_body(endpoint, body) {
        Ok(body) => body,
        Err(response) => return Ok(*response),
    };
    let value: Value =
        serde_json::from_slice(&body).context("failed to parse Codex Responses request JSON")?;
    let translated = runtime_provider_chat_compatible_request_body(
        &body,
        &conversations,
        RuntimeProviderBridgeKind::Kiro,
        "",
        false,
        runtime_kiro_rewrite_options(),
    )?;
    let prompt = runtime_kiro_prompt_from_messages(&translated.messages);
    let prompt_messages = translated.messages;
    let (requested_model, requested_effort) = runtime_kiro_requested_options(&value);
    let context = RuntimeKiroRequestContext {
        request_id,
        prompt,
        prompt_messages,
        auth,
        requested_model,
        requested_effort,
        chat_completions_route,
        shared,
        conversations,
        binding,
        binding_identity,
    };
    if stream_mode == Streaming {
        return runtime_kiro_streaming_upstream_result(context, anthropic_request);
    }
    runtime_kiro_buffered_upstream_result(context, anthropic_request)
}

fn runtime_kiro_unsupported_route_result(path: &str) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Buffered(runtime_kiro_json_parts(
            501,
            prodex_provider_core::kiro_provider_core_unsupported_path_error_value(path),
        )),
        gemini_context: None,
        copilot_context: None,
    }
}

fn runtime_kiro_anthropic_request(
    request: &RuntimeProxyRequest,
    messages_route: bool,
) -> std::result::Result<
    Option<RuntimeAnthropicMessagesRequest>,
    Box<RuntimeLocalRewriteUpstreamResult>,
> {
    if !messages_route {
        return Ok(None);
    }
    translate_runtime_anthropic_messages_request(request)
        .map(Some)
        .map_err(|err| {
            Box::new(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(
                    build_runtime_anthropic_error_parts(
                        400,
                        "invalid_request_error",
                        &err.to_string(),
                    ),
                ),
                gemini_context: None,
                copilot_context: None,
            })
        })
}

fn runtime_kiro_request_body(
    endpoint: ProviderEndpoint,
    body: Vec<u8>,
) -> std::result::Result<Vec<u8>, Box<RuntimeLocalRewriteUpstreamResult>> {
    runtime_kiro_request_body_for_endpoint(endpoint, body).map_err(|parts| {
        Box::new(RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
            gemini_context: None,
            copilot_context: None,
        })
    })
}

fn runtime_kiro_requested_options(value: &Value) -> (Option<String>, Option<String>) {
    let requested_model = value
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(|model| runtime_provider_canonical_model(RuntimeProviderBridgeKind::Kiro, model));
    let requested_effort = value
        .pointer("/reasoning/effort")
        .or_else(|| value.get("reasoning_effort"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|effort| !effort.is_empty())
        .map(str::to_string);
    (requested_model, requested_effort)
}

struct RuntimeKiroRequestContext<'a> {
    request_id: u64,
    prompt: String,
    prompt_messages: Vec<Value>,
    auth: &'a RuntimeKiroProfileAuth,
    requested_model: Option<String>,
    requested_effort: Option<String>,
    chat_completions_route: bool,
    shared: &'a RuntimeLocalRewriteProxyShared,
    conversations: RuntimeDeepSeekConversationStore,
    binding: RuntimeLocalRewriteBindingContext,
    binding_identity: RuntimeProviderBindingIdentity,
}

fn runtime_kiro_streaming_upstream_result(
    context: RuntimeKiroRequestContext<'_>,
    anthropic_request: Option<RuntimeAnthropicMessagesRequest>,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let request_id = context.request_id;
    let profile_name = context.auth.profile_name.clone();
    let shared = context.shared;
    let binding = context.binding.clone();
    let binding_identity = context.binding_identity.clone();
    let response =
        RuntimeLocalRewriteUpstreamResponse::Streaming(RuntimeLocalRewriteStreamingResponse {
            status: 200,
            headers: vec![(
                "content-type".to_string(),
                "text/event-stream; charset=utf-8".to_string(),
            )],
            body: Box::new(runtime_kiro_streaming_reader(context)?),
            profile_name,
            accepted_binding_recorder: Some(runtime_local_rewrite_binding_recorder(
                shared,
                binding_identity.clone(),
            )),
            accepted_binding: Some(binding.accepted_binding(binding_identity)),
        });
    let response = match anthropic_request.as_ref() {
        Some(anthropic_request) => runtime_kiro_anthropic_streaming_local_response(
            response,
            anthropic_request,
            request_id,
            &shared.runtime_shared,
        )?,
        None => response,
    };
    Ok(RuntimeLocalRewriteUpstreamResult {
        response,
        gemini_context: None,
        copilot_context: None,
    })
}

fn runtime_kiro_buffered_upstream_result(
    context: RuntimeKiroRequestContext<'_>,
    anthropic_request: Option<RuntimeAnthropicMessagesRequest>,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let RuntimeKiroRequestContext {
        request_id,
        prompt,
        prompt_messages,
        auth,
        requested_model,
        requested_effort,
        chat_completions_route,
        conversations,
        shared,
        binding,
        binding_identity,
    } = context;
    let (data_dir, secret) = prepare_kiro_cli_data_dir(&auth.codex_home)?;
    (|| {
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
        let response_succeeded = response.get("status").and_then(Value::as_str) != Some("failed");
        if response_succeeded && let Some(response_id) = response.get("id").and_then(Value::as_str)
        {
            runtime_deepseek_store_conversation(
                &conversations,
                response_id,
                prompt_messages,
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
        if response_succeeded {
            runtime_local_rewrite_remember_accepted_binding(
                shared,
                &binding_identity,
                binding.previous_response_id.as_deref(),
                binding.turn_state.as_deref(),
                binding.session_id.as_deref(),
            )?;
            if let RuntimeLocalRewriteUpstreamResponse::Buffered(parts) = &response {
                let recorder =
                    runtime_local_rewrite_binding_recorder(shared, binding_identity.clone());
                runtime_copilot_remember_bindings_from_responses_body(Some(&recorder), &parts.body);
            }
        }
        Ok(RuntimeLocalRewriteUpstreamResult {
            response,
            gemini_context: None,
            copilot_context: None,
        })
    })()
}

pub(super) fn runtime_kiro_compact_response_parts(
    request_id: u64,
    body: &[u8],
    async_runtime: &Arc<TokioRuntime>,
    auth: &RuntimeKiroProfileAuth,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    match runtime_kiro_semantic_compact_summary(request_id, body, async_runtime, auth) {
        Ok(summary) => runtime_compact_response_parts(&summary, "kiro", "semantic", None),
        Err(error) => runtime_local_compact_response_parts_with_reason(
            body,
            "kiro",
            runtime_compact_reason(&error),
        ),
    }
}

fn runtime_kiro_semantic_compact_summary(
    request_id: u64,
    body: &[u8],
    _async_runtime: &Arc<TokioRuntime>,
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
    let (data_dir, secret) = prepare_kiro_cli_data_dir(&auth.codex_home)?;
    (|| {
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
            .or_else(|| value.get("reasoning_effort"))
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
    })()
}

fn schedule_runtime_kiro_blocking_work(
    async_runtime: &Arc<TokioRuntime>,
    work: impl FnOnce() + Send + 'static,
) {
    drop(async_runtime.spawn_blocking(work));
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
    let accepted_binding_recorder = streaming.accepted_binding_recorder;
    let accepted_binding = streaming.accepted_binding;
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
                accepted_binding_recorder,
                accepted_binding,
            })
        }
    })
}

fn runtime_kiro_streaming_reader(
    context: RuntimeKiroRequestContext<'_>,
) -> Result<RuntimeKiroStreamingReader> {
    let RuntimeKiroRequestContext {
        request_id,
        prompt,
        prompt_messages,
        auth,
        requested_model,
        requested_effort,
        chat_completions_route,
        shared,
        conversations,
        ..
    } = context;
    let (data_dir, secret) = prepare_kiro_cli_data_dir(&auth.codex_home)?;
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
    let idle_timeout = Duration::from_millis(
        shared
            .runtime_shared
            .runtime_config
            .tuning
            .stream_idle_timeout_ms,
    );
    let (sender, receiver) = mpsc::sync_channel(16);
    let error_sender = sender.clone();
    schedule_runtime_kiro_blocking_work(&async_runtime, move || {
        let result = runtime_kiro_streaming_worker(
            sender,
            RuntimeKiroStreamingWorkerContext {
                request_id,
                prompt,
                prompt_messages,
                cwd,
                extra_env,
                command,
                profile_name,
                requested_model,
                requested_effort,
                chat_completions_route,
                conversations,
                idle_timeout,
            },
        );
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
        idle_timeout,
    })
}

struct RuntimeKiroStreamingWorkerContext {
    request_id: u64,
    prompt: String,
    prompt_messages: Vec<Value>,
    cwd: PathBuf,
    extra_env: Vec<(OsString, OsString)>,
    command: PathBuf,
    profile_name: String,
    requested_model: Option<String>,
    requested_effort: Option<String>,
    chat_completions_route: bool,
    conversations: RuntimeDeepSeekConversationStore,
    idle_timeout: Duration,
}

fn runtime_kiro_streaming_worker(
    sender: SyncSender<RuntimeKiroStreamingChunk>,
    context: RuntimeKiroStreamingWorkerContext,
) -> Result<()> {
    let RuntimeKiroStreamingWorkerContext {
        request_id,
        prompt,
        prompt_messages,
        cwd,
        extra_env,
        command,
        profile_name,
        requested_model,
        requested_effort,
        chat_completions_route,
        conversations,
        idle_timeout,
    } = context;
    let mut acp_command = runtime_kiro_streaming_command(
        &command,
        requested_model.as_deref(),
        requested_effort.as_deref(),
    );
    acp_command
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .envs(extra_env.iter().cloned());
    runtime_kiro_configure_process_group(&mut acp_command);
    let mut child = acp_command
        .spawn()
        .with_context(|| format!("failed to start Kiro ACP agent {}", command.display()))?;
    let result = runtime_kiro_streaming_child(
        &mut child,
        RuntimeKiroStreamingContext {
            request_id,
            prompt: &prompt,
            prompt_messages,
            cwd: &cwd,
            profile_name: &profile_name,
            requested_model,
            chat_completions_route,
            conversations,
            idle_timeout,
        },
        sender,
    );
    runtime_kiro_kill_process_group(&child);
    let _ = child.kill();
    let _ = child.wait();
    result
}

struct RuntimeKiroStreamingContext<'a> {
    request_id: u64,
    prompt: &'a str,
    prompt_messages: Vec<Value>,
    cwd: &'a Path,
    profile_name: &'a str,
    requested_model: Option<String>,
    chat_completions_route: bool,
    conversations: RuntimeDeepSeekConversationStore,
    idle_timeout: Duration,
}

fn runtime_kiro_streaming_child(
    child: &mut std::process::Child,
    context: RuntimeKiroStreamingContext<'_>,
    sender: SyncSender<RuntimeKiroStreamingChunk>,
) -> Result<()> {
    let RuntimeKiroStreamingContext {
        request_id,
        prompt,
        prompt_messages,
        cwd,
        profile_name,
        requested_model,
        chat_completions_route,
        conversations,
        idle_timeout,
    } = context;
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
    let lines = runtime_kiro_acp_line_receiver(stdout);
    let mut state = RuntimeKiroStreamingState::new(request_id, requested_model.as_deref());
    runtime_kiro_receive_stream(
        &sender,
        &mut stdin,
        lines,
        prompt,
        &mut state,
        chat_completions_route,
        idle_timeout,
        RUNTIME_KIRO_ACP_STREAM_TOTAL_TIMEOUT,
    )?;
    drop(stdin);
    runtime_kiro_finish_stream(
        sender,
        RuntimeKiroStreamingContext {
            request_id,
            prompt,
            prompt_messages,
            cwd,
            profile_name,
            requested_model,
            chat_completions_route,
            conversations,
            idle_timeout,
        },
        state,
    )
}

struct RuntimeKiroStreamingState {
    initialize: Option<RuntimeKiroAcpInitializeResult>,
    session: Option<RuntimeKiroAcpNewSessionResult>,
    prompt_response: Option<RuntimeKiroAcpEnvelope>,
    notifications: Vec<RuntimeKiroAcpEnvelope>,
    response_id: String,
    created_at: u64,
    sequence_number: u64,
    message_item_id: String,
    message_item_open: bool,
    assistant_text: String,
    tool_activities: RuntimeKiroStreamingActivityState,
    prompt_sent: bool,
    chat_delta_started: bool,
    chat_completion_id: String,
    stream_model: String,
}

impl RuntimeKiroStreamingState {
    fn new(request_id: u64, requested_model: Option<&str>) -> Self {
        let response_id = format!("resp_kiro_{request_id}");
        Self {
            initialize: None,
            session: None,
            prompt_response: None,
            notifications: Vec::new(),
            response_id: response_id.clone(),
            created_at: runtime_kiro_created_at(),
            sequence_number: 0,
            message_item_id: format!("msg_kiro_{request_id}_0"),
            message_item_open: false,
            assistant_text: String::new(),
            tool_activities: RuntimeKiroStreamingActivityState::default(),
            prompt_sent: false,
            chat_delta_started: false,
            chat_completion_id: format!("chatcmpl_{response_id}"),
            stream_model: requested_model
                .filter(|model| !model.is_empty())
                .unwrap_or("kiro-cli")
                .to_string(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn runtime_kiro_receive_stream(
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    stdin: &mut impl Write,
    lines: Receiver<io::Result<String>>,
    prompt: &str,
    state: &mut RuntimeKiroStreamingState,
    chat_completions_route: bool,
    idle_timeout: Duration,
    total_timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + total_timeout;
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            anyhow::bail!(runtime_kiro_stream_total_timeout_message(
                state.prompt_sent,
                total_timeout,
            ));
        };
        let total_deadline_is_next = remaining <= idle_timeout;
        let Some(line) = runtime_kiro_next_stream_line(
            &lines,
            idle_timeout.min(remaining),
            total_deadline_is_next,
            state.prompt_sent,
            total_timeout,
        )?
        else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        runtime_kiro_process_stream_envelope(
            sender,
            stdin,
            RuntimeKiroAcpEnvelope::parse(line.trim())?,
            prompt,
            state,
            chat_completions_route,
        )?;
        if state.prompt_response.is_some() {
            break;
        }
    }
    Ok(())
}

fn runtime_kiro_next_stream_line(
    lines: &Receiver<io::Result<String>>,
    timeout: Duration,
    total_deadline_is_next: bool,
    visible_output: bool,
    total_timeout: Duration,
) -> Result<Option<String>> {
    match lines.recv_timeout(timeout) {
        Ok(Ok(line)) => Ok(Some(line)),
        Ok(Err(error)) => Err(error).context("failed to read Kiro ACP stdout"),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            if total_deadline_is_next {
                anyhow::bail!(runtime_kiro_stream_total_timeout_message(
                    visible_output,
                    total_timeout,
                ));
            }
            anyhow::bail!(
                "Kiro ACP stream timed out waiting for output; no reconnect was attempted"
            )
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
    }
}

fn runtime_kiro_stream_total_timeout_message(
    visible_output: bool,
    total_timeout: Duration,
) -> String {
    let seconds = total_timeout.as_secs_f64();
    if visible_output {
        format!(
            "Kiro ACP stream exceeded its {seconds:.3}-second total limit after output began; Prodex did not reconnect or replay committed output"
        )
    } else {
        format!(
            "Kiro ACP stream exceeded its {seconds:.3}-second total limit before output; retry the request after checking the Kiro agent"
        )
    }
}

fn runtime_kiro_process_stream_envelope(
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    stdin: &mut impl Write,
    envelope: RuntimeKiroAcpEnvelope,
    prompt: &str,
    state: &mut RuntimeKiroStreamingState,
    chat_completions_route: bool,
) -> Result<()> {
    if runtime_kiro_acp_reject_unsupported_server_request(stdin, &envelope)? {
        state.notifications.push(envelope);
        return Ok(());
    }
    if !state.prompt_sent && matches!(envelope.numeric_id(), Some(1)) && envelope.error.is_none() {
        return runtime_kiro_send_stream_prompt(
            sender,
            stdin,
            envelope.parse_session_new_result()?,
            prompt,
            state,
            chat_completions_route,
        );
    }
    match envelope.numeric_id() {
        Some(0) if envelope.error.is_none() => {
            state.initialize = Some(envelope.parse_initialize_result()?);
        }
        Some(1) if envelope.error.is_none() => {
            state.session = Some(envelope.parse_session_new_result()?);
        }
        Some(2) => state.prompt_response = Some(envelope),
        _ => {
            if let Ok(notification) = envelope.parse_session_notification() {
                runtime_kiro_stream_notification(
                    sender,
                    &notification,
                    &state.response_id,
                    &state.chat_completion_id,
                    &state.stream_model,
                    state.created_at,
                    &state.message_item_id,
                    &mut state.sequence_number,
                    &mut state.message_item_open,
                    &mut state.assistant_text,
                    &mut state.tool_activities,
                    chat_completions_route,
                    &mut state.chat_delta_started,
                )?;
            }
            state.notifications.push(envelope);
        }
    }
    Ok(())
}

fn runtime_kiro_send_stream_prompt(
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    stdin: &mut impl Write,
    session: RuntimeKiroAcpNewSessionResult,
    prompt: &str,
    state: &mut RuntimeKiroStreamingState,
    chat_completions_route: bool,
) -> Result<()> {
    writeln!(
        stdin,
        "{}",
        runtime_kiro_acp_session_prompt_request(2, &session.session_id, prompt)
    )
    .context("failed to write Kiro ACP session/prompt request")?;
    stdin
        .flush()
        .context("failed to flush Kiro ACP session/prompt request")?;
    state.prompt_sent = true;
    state.session = Some(session);
    if chat_completions_route {
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_kiro_chat_completion_chunk(
                &state.chat_completion_id,
                Some(&state.stream_model),
                prodex_provider_core::kiro_provider_core_chat_completion_role_delta(),
                None,
            )?,
        ))?;
        state.chat_delta_started = true;
    } else {
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_provider_sse_event(
                "response.created",
                prodex_provider_core::kiro_provider_core_response_created_event(
                    state.sequence_number,
                    state.created_at,
                    &state.response_id,
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

enum RuntimeKiroStreamingChunk {
    Data(Vec<u8>),
    Error(io::Error),
    End,
}

struct RuntimeKiroStreamingReader {
    receiver: Receiver<RuntimeKiroStreamingChunk>,
    pending: Cursor<Vec<u8>>,
    finished: bool,
    idle_timeout: Duration,
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
            match self.receiver.recv_timeout(self.idle_timeout) {
                Ok(RuntimeKiroStreamingChunk::Data(bytes)) => {
                    self.pending = Cursor::new(bytes);
                }
                Ok(RuntimeKiroStreamingChunk::Error(err)) => return Err(err),
                Ok(RuntimeKiroStreamingChunk::End) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.finished = true;
                    return Ok(0);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Kiro ACP stream timed out waiting for output",
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
