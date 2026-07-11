use super::chat_compatible_request::runtime_provider_chat_compatible_request_body;
use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_store_conversation,
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
    runtime_kiro_acp_prompt_turn_with_command, runtime_kiro_acp_responses_value_from_prompt_turn,
    runtime_kiro_acp_session_new_request, runtime_kiro_acp_session_prompt_request,
};
use crate::runtime_proxy_shared::{RuntimeResponsesReply, RuntimeStreamingResponse};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest};
use anyhow::{Context, Result};
use runtime_proxy_crate::path_without_query;
use serde_json::{Value, json};
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

const KIRO_SEMANTIC_COMPACT_INSTRUCTIONS: &str = "\
Compact the supplied coding-agent transcript into one durable continuation summary. \
Preserve the user's goals, repository instructions, decisions, files changed, exact identifiers, \
commands and test results, unresolved failures, current worktree state, and the next concrete steps. \
Remove redundant narration and obsolete intermediate reasoning. Do not call tools. \
Return only the continuation summary, with no preamble or completion claim.";

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
            json!({
                "object": "list",
                "data": auth.model_catalog,
            }),
        ));
    }
    let model_id = path.split("/models/").nth(1)?.trim();
    if model_id.is_empty() {
        return None;
    }
    if let Some(model) = auth.model_catalog.iter().find(|model| {
        model
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.eq_ignore_ascii_case(model_id))
    }) {
        return Some(runtime_kiro_json_parts(200, model.clone()));
    }
    Some(runtime_kiro_json_parts(
        404,
        json!({
            "error": {
                "message": format!("model '{model_id}' is not available for kiro"),
                "type": "invalid_request_error",
                "code": "model_not_found",
            }
        }),
    ))
}

pub(super) fn send_runtime_kiro_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeKiroProfileAuth,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let path = path_without_query(&request.path_and_query);
    let chat_completions_route = path.ends_with("/chat/completions");
    let messages_route = path.ends_with("/messages");
    if !(path.ends_with("/responses") || chat_completions_route || messages_route) {
        return Ok(RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Buffered(runtime_kiro_json_parts(
                501,
                json!({
                    "error": {
                        "message": format!("Kiro provider does not support {path} yet"),
                        "type": "invalid_request_error",
                        "code": "unsupported_path",
                    }
                }),
            )),
            gemini_context: None,
            copilot_context: None,
        });
    }
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
    let client_stream = if messages_route {
        serde_json::from_slice::<Value>(&request.body)
            .ok()
            .and_then(|value| value.get("stream").and_then(Value::as_bool))
            .unwrap_or(false)
    } else {
        false
    };
    let body = anthropic_request
        .as_ref()
        .map(|translated| translated.translated_request.body.clone())
        .unwrap_or(body);
    let body = match runtime_kiro_request_body_for_path(path, body) {
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
    let stream = if messages_route {
        client_stream
    } else {
        value
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };

    let translated = runtime_provider_chat_compatible_request_body(
        &body,
        &shared.deepseek_conversations,
        super::provider_bridge::RuntimeProviderBridgeKind::Kiro,
        "",
        false,
        Default::default(),
    )?;
    let prompt_messages = translated.messages.clone();
    let prompt = runtime_kiro_prompt_from_messages(&translated.messages);
    let overlay_root = create_private_kiro_temp_root("runtime")?;
    let result = (|| {
        let secret = read_kiro_auth_secret(&auth.codex_home)?;
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
            .as_deref()
            .map(Path::as_os_str)
            .unwrap_or(default_command.as_os_str());
        let turn = runtime_kiro_acp_prompt_turn_with_command(command, &cwd, &extra_env, &prompt)?;
        let mut response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, request_id);
        response["metadata"]["kiro"]["profile_name"] = Value::String(auth.profile_name.clone());
        if let Some(model) = value
            .get("model")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            response["requested_model"] = Value::String(model.to_string());
        }
        if response.get("status").and_then(Value::as_str) != Some("failed")
            && let Some(response_id) = response.get("id").and_then(Value::as_str)
        {
            runtime_deepseek_store_conversation(
                &shared.deepseek_conversations,
                response_id,
                translated.messages,
                runtime_kiro_acp_chat_assistant_messages_from_prompt_turn(&turn),
            );
        }
        if stream {
            let response = RuntimeLocalRewriteUpstreamResponse::Streaming(
                RuntimeLocalRewriteStreamingResponse {
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
                        value
                            .get("model")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string),
                        chat_completions_route,
                        shared,
                    )?),
                    profile_name: auth.profile_name.clone(),
                },
            );
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
            Ok(RuntimeLocalRewriteUpstreamResult {
                response,
                gemini_context: None,
                copilot_context: None,
            })
        } else {
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
                    runtime_kiro_anthropic_message_parts_from_response(
                        &response,
                        anthropic_request,
                    ),
                )
            } else {
                response
            };
            Ok(RuntimeLocalRewriteUpstreamResult {
                response,
                gemini_context: None,
                copilot_context: None,
            })
        }
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
    let mut value: Value =
        serde_json::from_slice(body).context("failed to parse Kiro compact request JSON")?;
    let object = value
        .as_object_mut()
        .context("Kiro compact request must be a JSON object")?;
    let input = object
        .get_mut("input")
        .and_then(Value::as_array_mut)
        .context("Kiro compact request must contain an input array")?;
    input.push(json!({
        "type": "message",
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": KIRO_SEMANTIC_COMPACT_INSTRUCTIONS,
        }],
    }));
    object.insert("stream".to_string(), Value::Bool(false));
    object.insert("store".to_string(), Value::Bool(false));
    for key in [
        "include",
        "previous_response_id",
        "prompt_cache_key",
        "text",
        "tool_choice",
        "tools",
    ] {
        object.remove(key);
    }

    let translated = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&value).context("failed to serialize Kiro compact request")?,
        &RuntimeDeepSeekConversationStore::default(),
        super::provider_bridge::RuntimeProviderBridgeKind::Kiro,
        "",
        false,
        Default::default(),
    )?;
    let prompt = runtime_kiro_prompt_from_messages(&translated.messages);
    let overlay_root = create_private_kiro_temp_root("compact")?;
    let result = (|| {
        let secret = read_kiro_auth_secret(&auth.codex_home)?;
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
            .as_deref()
            .map(Path::as_os_str)
            .unwrap_or(default_command.as_os_str());
        let turn = runtime_kiro_acp_prompt_turn_with_command(command, &cwd, &extra_env, &prompt)?;
        let response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, request_id);
        runtime_kiro_compact_summary_from_response(&response)
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

fn runtime_kiro_compact_summary_from_response(response: &Value) -> Result<String> {
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .context("Kiro compact response is missing output")?;
    let summary = output
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .and_then(|item| item.get("content"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|item| item.get("text").and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .context("Kiro compact response returned no summary text")?;
    Ok(summary.to_string())
}

fn runtime_kiro_chat_completion_value_from_response(response: &Value, request_id: u64) -> Value {
    let id = response
        .get("id")
        .and_then(Value::as_str)
        .map(|id| format!("chatcmpl_{id}"))
        .unwrap_or_else(|| format!("chatcmpl_kiro_{request_id}"));
    let created = response
        .get("created_at")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let model = response
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("kiro-cli");
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let assistant_text = output
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .and_then(|item| item.get("content"))
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|item| item.get("text"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tool_calls = output
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .map(|item| {
            json!({
                "id": item.get("call_id").and_then(Value::as_str).unwrap_or("call_kiro"),
                "type": "function",
                "function": {
                    "name": item.get("name").and_then(Value::as_str).unwrap_or("tool_call"),
                    "arguments": item.get("arguments").and_then(Value::as_str).unwrap_or("{}"),
                }
            })
        })
        .collect::<Vec<_>>();
    let has_tool_calls = !tool_calls.is_empty();
    let mut message = json!({
        "role": "assistant",
        "content": if assistant_text.is_empty() && has_tool_calls {
            Value::Null
        } else {
            Value::String(assistant_text.to_string())
        },
    });
    if has_tool_calls {
        message["tool_calls"] = Value::Array(tool_calls);
    }
    if let Some(reasoning_content) = response
        .get("metadata")
        .and_then(|metadata| metadata.get("kiro"))
        .and_then(|kiro| kiro.get("reasoning_content"))
        .and_then(Value::as_str)
        .filter(|reasoning| !reasoning.is_empty())
    {
        message["reasoning_content"] = Value::String(reasoning_content.to_string());
    }
    let finish_reason = runtime_kiro_chat_completion_finish_reason(response, has_tool_calls);
    let mut choice = json!({
        "index": 0,
        "message": message,
        "finish_reason": finish_reason,
    });
    if response.get("status").and_then(Value::as_str) == Some("failed")
        && let Some(error) = response.get("error")
    {
        choice["message"]["refusal"] = error
            .get("message")
            .cloned()
            .unwrap_or(Value::String("Kiro request failed".to_string()));
    }
    let mut completion = json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": [choice],
    });
    if let Some(requested_model) = response.get("requested_model") {
        completion["requested_model"] = requested_model.clone();
    }
    if let Some(metadata) = response.get("metadata") {
        completion["metadata"] = metadata.clone();
    }
    completion
}

fn runtime_kiro_chat_completion_finish_reason(
    response: &Value,
    has_tool_calls: bool,
) -> &'static str {
    if has_tool_calls {
        return "tool_calls";
    }
    if response
        .get("incomplete_details")
        .and_then(|details| details.get("reason"))
        .and_then(Value::as_str)
        == Some("max_output_tokens")
    {
        return "length";
    }
    "stop"
}

fn runtime_kiro_request_body_for_path(
    path: &str,
    body: Vec<u8>,
) -> std::result::Result<Vec<u8>, RuntimeHeapTrimmedBufferedResponseParts> {
    if !path.ends_with("/chat/completions") {
        return Ok(body);
    }
    let mut value: Value = serde_json::from_slice(&body).map_err(|_| {
        runtime_kiro_invalid_request_error(
            "Kiro chat completions request body must be valid JSON",
            "invalid_json",
        )
    })?;
    let Some(object) = value.as_object_mut() else {
        return Err(runtime_kiro_invalid_request_error(
            "Kiro chat completions request body must be a JSON object",
            "invalid_request_body",
        ));
    };
    if let Some(response_format) = object.get("response_format")
        && !runtime_kiro_supported_chat_response_format(response_format)
    {
        return Err(runtime_kiro_invalid_request_error(
            "Kiro provider only supports chat response_format type 'text' right now",
            "unsupported_response_format",
        ));
    }
    if object
        .get("n")
        .and_then(Value::as_u64)
        .is_some_and(|n| n > 1)
    {
        return Err(runtime_kiro_invalid_request_error(
            "Kiro provider only supports chat completion parameter n=1 right now",
            "unsupported_choice_count",
        ));
    }
    if let Some(stop) = object.get("stop") {
        if runtime_kiro_has_requested_stop_sequences(stop) {
            return Err(runtime_kiro_invalid_request_error(
                "Kiro provider does not support chat stop sequences right now",
                "unsupported_stop",
            ));
        }
        object.remove("stop");
    }
    if let Some(temperature) = object.get("temperature") {
        if runtime_kiro_has_requested_nondefault_number(temperature, 1.0) {
            return Err(runtime_kiro_invalid_request_error(
                "Kiro provider does not support non-default chat temperature right now",
                "unsupported_temperature",
            ));
        }
        object.remove("temperature");
    }
    if let Some(top_p) = object.get("top_p") {
        if runtime_kiro_has_requested_nondefault_number(top_p, 1.0) {
            return Err(runtime_kiro_invalid_request_error(
                "Kiro provider does not support non-default chat top_p right now",
                "unsupported_top_p",
            ));
        }
        object.remove("top_p");
    }
    if let Some(presence_penalty) = object.get("presence_penalty") {
        if runtime_kiro_has_requested_nondefault_number(presence_penalty, 0.0) {
            return Err(runtime_kiro_invalid_request_error(
                "Kiro provider does not support non-default chat presence_penalty right now",
                "unsupported_presence_penalty",
            ));
        }
        object.remove("presence_penalty");
    }
    if let Some(frequency_penalty) = object.get("frequency_penalty") {
        if runtime_kiro_has_requested_nondefault_number(frequency_penalty, 0.0) {
            return Err(runtime_kiro_invalid_request_error(
                "Kiro provider does not support non-default chat frequency_penalty right now",
                "unsupported_frequency_penalty",
            ));
        }
        object.remove("frequency_penalty");
    }
    if object
        .get("seed")
        .is_some_and(runtime_kiro_has_requested_sampling_value)
    {
        return Err(runtime_kiro_invalid_request_error(
            "Kiro provider does not support chat seed right now",
            "unsupported_seed",
        ));
    }
    if let Some(parallel_tool_calls) = object.get("parallel_tool_calls") {
        if runtime_kiro_has_requested_parallel_tool_calls_control(parallel_tool_calls) {
            return Err(runtime_kiro_invalid_request_error(
                "Kiro provider does not support chat parallel_tool_calls right now",
                "unsupported_parallel_tool_calls",
            ));
        }
        object.remove("parallel_tool_calls");
    }
    if object.contains_key("user") {
        object.remove("user");
    }
    runtime_kiro_strip_accepted_token_limit_controls(object)?;
    if object.contains_key("input") || !object.contains_key("messages") {
        return Ok(body);
    }
    let messages = object.remove("messages").ok_or_else(|| {
        runtime_kiro_invalid_request_error(
            "Kiro chat completions request is missing messages",
            "missing_messages",
        )
    })?;
    let items = messages
        .as_array()
        .ok_or_else(|| {
            runtime_kiro_invalid_request_error(
                "Kiro chat completions messages must be an array",
                "invalid_messages",
            )
        })?
        .iter()
        .flat_map(runtime_kiro_responses_items_from_chat_message)
        .collect::<Vec<_>>();
    object.insert("input".to_string(), Value::Array(items));
    if !object.contains_key("tools")
        && let Some(functions) = object.remove("functions")
        && let Some(functions) = functions.as_array()
    {
        object.insert(
            "tools".to_string(),
            Value::Array(
                functions
                    .iter()
                    .filter_map(runtime_kiro_tool_from_legacy_chat_function)
                    .collect(),
            ),
        );
    }
    if !object.contains_key("tool_choice")
        && let Some(function_call) = object.remove("function_call")
        && let Some(tool_choice) =
            runtime_kiro_tool_choice_from_legacy_chat_function_call(&function_call)
    {
        object.insert("tool_choice".to_string(), tool_choice);
    }
    serde_json::to_vec(&value).map_err(|_| {
        runtime_kiro_invalid_request_error(
            "failed to serialize rewritten Kiro chat completions body",
            "invalid_request_body",
        )
    })
}

fn runtime_kiro_supported_chat_response_format(value: &Value) -> bool {
    value.is_null()
        || value
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| matches!(kind, "text"))
}

fn runtime_kiro_has_requested_stop_sequences(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => items.iter().any(|item| match item {
            Value::String(text) => !text.is_empty(),
            _ => true,
        }),
        _ => true,
    }
}

fn runtime_kiro_has_requested_sampling_value(value: &Value) -> bool {
    !matches!(value, Value::Null)
}

fn runtime_kiro_has_requested_nondefault_number(value: &Value, default: f64) -> bool {
    match value {
        Value::Null => false,
        Value::Number(number) => number
            .as_f64()
            .is_none_or(|value| (value - default).abs() > f64::EPSILON),
        _ => true,
    }
}

fn runtime_kiro_has_requested_parallel_tool_calls_control(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(true) => false,
        Value::Bool(false) => true,
        _ => true,
    }
}

fn runtime_kiro_strip_accepted_token_limit_controls(
    object: &mut serde_json::Map<String, Value>,
) -> std::result::Result<(), RuntimeHeapTrimmedBufferedResponseParts> {
    for field in ["max_output_tokens", "max_tokens", "max_completion_tokens"] {
        let Some(value) = object.get(field) else {
            continue;
        };
        if value.as_u64().is_none_or(|count| count == 0) {
            return Err(runtime_kiro_invalid_request_error(
                &format!("Kiro {field} must be a positive integer"),
                "unsupported_token_limit",
            ));
        }
    }
    for field in ["max_output_tokens", "max_tokens", "max_completion_tokens"] {
        object.remove(field);
    }
    Ok(())
}

fn runtime_kiro_invalid_request_error(
    message: &str,
    code: &str,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    runtime_kiro_json_parts(
        400,
        json!({
            "error": {
                "message": message,
                "type": "invalid_request_error",
                "code": code,
            }
        }),
    )
}

fn runtime_kiro_prompt_from_messages(messages: &[serde_json::Value]) -> String {
    let mut sections = Vec::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("message");
        let mut block = String::new();
        if let Some(content) = message.get("content").and_then(runtime_kiro_message_text) {
            block.push_str(&content);
        }
        if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                let name = tool_call
                    .get("function")
                    .and_then(|v| v.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("tool_call");
                let arguments = tool_call
                    .get("function")
                    .and_then(|v| v.get("arguments"))
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                if !block.is_empty() {
                    block.push('\n');
                }
                block.push_str(&format!("Tool call {name}: {arguments}"));
            }
        }
        if block.trim().is_empty() {
            continue;
        }
        sections.push(format!(
            "{}:\n{}",
            runtime_kiro_role_label(role),
            block.trim()
        ));
    }
    if sections.is_empty() {
        "User:\n".to_string()
    } else {
        sections.join("\n\n")
    }
}

fn runtime_kiro_responses_items_from_chat_message(message: &Value) -> Vec<Value> {
    let Some(object) = message.as_object() else {
        return Vec::new();
    };
    let role = object.get("role").and_then(Value::as_str).unwrap_or("user");
    let mut items = Vec::new();
    if !matches!(role, "tool" | "function")
        && let Some(text) = object
            .get("content")
            .and_then(runtime_kiro_chat_message_text)
    {
        items.push(json!({
            "type": "message",
            "role": role,
            "content": [{
                "type": "input_text",
                "text": text,
            }],
        }));
    }
    if role == "assistant"
        && let Some(tool_calls) = object.get("tool_calls").and_then(Value::as_array)
    {
        for tool_call in tool_calls {
            let Some(function) = tool_call.get("function").and_then(Value::as_object) else {
                continue;
            };
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("tool_call");
            let arguments = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            items.push(json!({
                "type": "function_call",
                "call_id": tool_call
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("call_kiro"),
                "name": name,
                "arguments": arguments,
            }));
        }
    }
    if role == "assistant"
        && !object.contains_key("tool_calls")
        && let Some(function_call) = object.get("function_call").and_then(Value::as_object)
    {
        let name = function_call
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("tool_call");
        let arguments = function_call
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("{}");
        let call_id = function_call
            .get("call_id")
            .or_else(|| function_call.get("id"))
            .and_then(Value::as_str)
            .unwrap_or(name);
        items.push(json!({
            "type": "function_call",
            "call_id": call_id,
            "name": name,
            "arguments": arguments,
        }));
    }
    if matches!(role, "tool" | "function") {
        let output = object
            .get("content")
            .and_then(runtime_kiro_chat_message_text)
            .unwrap_or_default();
        items.push(json!({
            "type": "function_call_output",
            "call_id": object
                .get("tool_call_id")
                .or_else(|| object.get("call_id"))
                .or_else(|| object.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("call_kiro"),
            "output": output,
        }));
    }
    items
}

fn runtime_kiro_chat_message_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.trim().is_empty()).then(|| text.to_string()),
        Value::Array(items) => {
            let mut text = String::new();
            for item in items {
                if let Some(chunk) = item
                    .get("text")
                    .and_then(Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(chunk);
                } else if let Some(chunk) = runtime_kiro_chat_message_text(item) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&chunk);
                }
            }
            (!text.trim().is_empty()).then_some(text)
        }
        Value::Object(object) => object
            .get("text")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(str::to_string),
        _ => None,
    }
}

fn runtime_kiro_tool_from_legacy_chat_function(function: &Value) -> Option<Value> {
    let function = function.as_object()?;
    let name = function.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    let mut tool_function = serde_json::Map::new();
    tool_function.insert("name".to_string(), Value::String(name.to_string()));
    if let Some(description) = function
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|description| !description.is_empty())
    {
        tool_function.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }
    if let Some(parameters) = function.get("parameters") {
        tool_function.insert("parameters".to_string(), parameters.clone());
    }
    Some(json!({
        "type": "function",
        "function": Value::Object(tool_function),
    }))
}

fn runtime_kiro_tool_choice_from_legacy_chat_function_call(function_call: &Value) -> Option<Value> {
    if let Some(choice) = function_call
        .as_str()
        .filter(|choice| matches!(*choice, "auto" | "none"))
    {
        return Some(Value::String(choice.to_string()));
    }
    let object = function_call.as_object()?;
    let name = object.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(json!({
        "type": "function",
        "function": {
            "name": name,
        }
    }))
}

fn runtime_kiro_message_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.trim().is_empty()).then(|| text.to_string()),
        Value::Array(items) => {
            let mut text = String::new();
            for item in items {
                if let Some(chunk) = runtime_kiro_message_text(item) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&chunk);
                }
            }
            (!text.trim().is_empty()).then_some(text)
        }
        Value::Object(object) => {
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                return (!text.trim().is_empty()).then(|| text.to_string());
            }
            if let Some(text) = object.get("content").and_then(runtime_kiro_message_text) {
                return Some(text);
            }
            if let Some(tool_output) = object.get("output").and_then(runtime_kiro_message_text) {
                return Some(tool_output);
            }
            None
        }
        _ => None,
    }
}

fn runtime_kiro_role_label(role: &str) -> &'static str {
    match role {
        "system" => "System",
        "assistant" => "Assistant",
        "tool" => "Tool",
        _ => "User",
    }
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
    let text = value
        .get("output")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                (item.get("type").and_then(Value::as_str) == Some("message"))
                    .then(|| item.get("content"))
                    .flatten()
                    .and_then(runtime_kiro_content_text)
            })
        })
        .unwrap_or_default();
    let tool_use_blocks = value
        .get("output")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
                .map(|item| {
                    let input = item
                        .get("arguments")
                        .and_then(Value::as_str)
                        .and_then(|arguments| serde_json::from_str::<Value>(arguments).ok())
                        .unwrap_or_else(|| json!({}));
                    json!({
                        "type": "tool_use",
                        "id": item.get("call_id").cloned().unwrap_or_else(|| Value::String("call_kiro".to_string())),
                        "name": item.get("name").cloned().unwrap_or_else(|| Value::String("tool_call".to_string())),
                        "input": input,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut content = tool_use_blocks;
    if !text.is_empty() {
        content.push(json!({
            "type": "text",
            "text": text,
        }));
    }
    let usage = value.get("usage").cloned().unwrap_or_else(|| json!({}));
    let stop_reason = if content
        .iter()
        .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
    {
        "tool_use"
    } else {
        value
            .get("metadata")
            .and_then(|metadata| metadata.get("kiro"))
            .and_then(|kiro| kiro.get("stop_reason"))
            .and_then(Value::as_str)
            .map(|reason| match reason {
                "max_output_tokens" => "max_tokens",
                "tool_use" => "tool_use",
                _ => "end_turn",
            })
            .unwrap_or("end_turn")
    };
    runtime_kiro_json_parts(
        200,
        json!({
            "id": value.get("id").cloned().unwrap_or_else(|| Value::String("msg_kiro".to_string())),
            "type": "message",
            "role": "assistant",
            "model": anthropic_request.requested_model,
            "content": content,
            "stop_reason": stop_reason,
            "stop_sequence": Value::Null,
            "usage": {
                "input_tokens": usage.get("input_tokens").cloned().unwrap_or(Value::from(0)),
                "output_tokens": usage.get("output_tokens").cloned().unwrap_or(Value::from(0)),
            }
        }),
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

fn runtime_kiro_streaming_reader(
    request_id: u64,
    prompt: String,
    prompt_messages: Vec<Value>,
    auth: &RuntimeKiroProfileAuth,
    requested_model: Option<String>,
    chat_completions_route: bool,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeKiroStreamingReader> {
    let secret = read_kiro_auth_secret(&auth.codex_home)?;
    let overlay_root = create_private_kiro_temp_root("runtime")?;
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
                            json!({"role": "assistant"}),
                            None,
                        )?,
                    ))?;
                    chat_delta_started = true;
                } else {
                    sender.send(RuntimeKiroStreamingChunk::Data(
                        runtime_provider_sse_event(
                            "response.created",
                            json!({
                                "type": "response.created",
                                "sequence_number": sequence_number,
                                "created_at": created_at,
                                "response": {"id": response_id},
                            }),
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
        response["created_at"] = Value::from(created_at);
        response["metadata"]["kiro"]["profile_name"] = Value::String(profile_name.to_string());
        if let Some(model) = requested_model.filter(|s| !s.is_empty()) {
            response["requested_model"] = Value::String(model);
        }
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
            let has_tool_calls = response
                .get("output")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        item.get("type").and_then(Value::as_str) == Some("function_call")
                    })
                });
            sender.send(RuntimeKiroStreamingChunk::Data(
                runtime_kiro_chat_completion_chunk(
                    &chat_completion_id,
                    None,
                    json!({}),
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
                    json!({
                        "type": "response.completed",
                        "sequence_number": sequence_number,
                        "created_at": created_at,
                        "response": response,
                    }),
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
                let delta = if include_model {
                    *chat_delta_started = true;
                    json!({"role": "assistant", "content": text})
                } else {
                    json!({"content": text})
                };
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
                        json!({
                            "type": "response.output_text.delta",
                            "sequence_number": *sequence_number,
                            "created_at": created_at,
                            "response_id": response_id,
                            "delta": text,
                        }),
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
    let arguments = raw_input
        .and_then(|value| serde_json::to_string(value).ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "{}".to_string());
    if chat_completions_route {
        let should_emit = added_tool_calls.insert(tool_call_id.to_string())
            || (raw_input.is_some() && delta_tool_calls.insert(tool_call_id.to_string()));
        if should_emit {
            let include_model = !*chat_delta_started;
            let delta = if include_model {
                *chat_delta_started = true;
                json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "index": 0,
                        "id": tool_call_id,
                        "type": "function",
                        "function": {
                            "name": item.get("name").and_then(Value::as_str).unwrap_or("tool_call"),
                            "arguments": arguments,
                        }
                    }]
                })
            } else {
                json!({
                    "tool_calls": [{
                        "index": 0,
                        "id": tool_call_id,
                        "type": "function",
                        "function": {
                            "name": item.get("name").and_then(Value::as_str).unwrap_or("tool_call"),
                            "arguments": arguments,
                        }
                    }]
                })
            };
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
                json!({
                    "type": "response.output_item.added",
                    "sequence_number": *sequence_number,
                    "item": item,
                }),
            )
            .into_bytes(),
        ))?;
    }
    if delta_tool_calls.insert(tool_call_id.to_string()) {
        *sequence_number += 1;
        let upstream_value = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": tool_call_id,
                        "function": {
                            "arguments": arguments,
                        }
                    }]
                }
            }]
        });
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
                json!({
                    "type": "response.output_item.done",
                    "sequence_number": *sequence_number,
                    "item": item,
                    "response_id": response_id,
                }),
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
    let name = runtime_kiro_stream_tool_name(title, kind);
    let arguments = raw_input
        .and_then(|value| serde_json::to_string(value).ok())
        .unwrap_or_else(|| "{}".to_string());
    json!({
        "type": "function_call",
        "call_id": tool_call_id,
        "name": name,
        "namespace": "kiro",
        "arguments": arguments,
        "metadata": {
            "kiro": {
                "title": title.unwrap_or("tool_call"),
                "status": status,
                "kind": kind,
            }
        }
    })
}

fn runtime_kiro_stream_tool_name(title: Option<&str>, kind: Option<&str>) -> String {
    let candidate = title
        .filter(|title| !title.trim().is_empty())
        .or(kind)
        .unwrap_or("tool_call");
    let mut normalized = String::new();
    let mut last_was_separator = false;
    for ch in candidate.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            last_was_separator = false;
        } else if !last_was_separator && !normalized.is_empty() {
            normalized.push('_');
            last_was_separator = true;
        }
    }
    normalized.trim_matches('_').to_string()
}

fn runtime_kiro_chat_completion_chunk(
    chat_completion_id: &str,
    model: Option<&str>,
    delta: Value,
    finish_reason: Option<&str>,
) -> Result<Vec<u8>> {
    let mut chunk = json!({
        "id": chat_completion_id,
        "object": "chat.completion.chunk",
        "choices": [{
            "index": 0,
            "delta": delta,
        }],
    });
    if let Some(model) = model.filter(|value| !value.is_empty()) {
        chunk["model"] = Value::String(model.to_string());
    }
    if let Some(finish_reason) = finish_reason {
        chunk["choices"][0]["finish_reason"] = Value::String(finish_reason.to_string());
    }
    Ok(format!(
        "data: {}\n\n",
        serde_json::to_string(&chunk).context("failed to serialize Kiro chat completion chunk")?
    )
    .into_bytes())
}

fn runtime_kiro_content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.is_empty()).then(|| text.clone()),
        Value::Array(items) => {
            let mut text = String::new();
            for item in items {
                if let Some(chunk) = runtime_kiro_content_text(item) {
                    text.push_str(&chunk);
                }
            }
            (!text.is_empty()).then_some(text)
        }
        Value::Object(object) => {
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                return (!text.is_empty()).then(|| text.to_string());
            }
            object.get("content").and_then(runtime_kiro_content_text)
        }
        _ => None,
    }
}

fn runtime_kiro_created_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn runtime_kiro_read_stderr(stderr: &mut impl Read) -> String {
    let mut stderr_text = String::new();
    let _ = stderr.read_to_string(&mut stderr_text);
    stderr_text
}

fn runtime_kiro_stderr_suffix(stderr: &str) -> String {
    let stderr = stderr.trim();
    if stderr.is_empty() {
        String::new()
    } else {
        format!("; stderr: {stderr}")
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
#[path = "local_rewrite_kiro/tests.rs"]
mod tests;
