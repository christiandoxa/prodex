//! Kiro ACP compatibility helpers.

#[cfg(feature = "mojo")]
use super::stream::kiro_mojo_body;
#[cfg(not(feature = "mojo"))]
use super::stream::{
    KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_EVENTS, kiro_provider_core_truncated_tool_activity_item,
};
#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{KiroKernelInput, KiroKernelOperation};
use serde_json::Value;
#[cfg(not(feature = "mojo"))]
use serde_json::json;

#[cfg(feature = "mojo")]
fn kiro_acp_mojo_value(input: KiroKernelInput<'_>) -> Value {
    serde_json::from_slice(&kiro_mojo_body(input)).expect("Mojo Kiro ACP shape is valid JSON")
}

#[cfg(feature = "mojo")]
fn kiro_acp_mojo_optional_value(input: KiroKernelInput<'_>) -> Option<Value> {
    let body = kiro_mojo_body(input);
    (!body.is_empty())
        .then(|| serde_json::from_slice(&body).expect("Mojo Kiro optional ACP shape is valid JSON"))
}

pub fn kiro_provider_core_acp_initialize_request(
    id: u64,
    client_name: &str,
    client_title: &str,
    client_version: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpInitializeRequest);
        input.request_id = id;
        input.name = Some(client_name);
        input.content = Some(client_title);
        input.model = Some(client_version);
        kiro_acp_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": {
                    "readTextFile": false,
                    "writeTextFile": false,
                },
                "terminal": false,
                "auth": {
                    "terminal": false,
                },
            },
            "clientInfo": {
                "name": client_name,
                "title": client_title,
                "version": client_version,
            },
        }
    })
}

pub fn kiro_provider_core_acp_session_new_request(id: u64, cwd: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpSessionNewRequest);
        input.request_id = id;
        input.content = Some(cwd);
        kiro_acp_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/new",
        "params": {
            "cwd": cwd,
            "mcpServers": [],
        }
    })
}

pub fn kiro_provider_core_acp_session_prompt_request(
    id: u64,
    session_id: &str,
    prompt: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpSessionPromptRequest);
        input.request_id = id;
        input.response_id = Some(session_id);
        input.content = Some(prompt);
        kiro_acp_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/prompt",
        "params": {
            "sessionId": session_id,
            "prompt": [{
                "type": "text",
                "text": prompt,
            }],
        }
    })
}

pub fn kiro_provider_core_acp_model_value(model_id: &str, name: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpModel);
        input.model = Some(model_id);
        input.name = Some(name);
        kiro_acp_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "id": model_id,
        "name": name,
        "object": "model",
        "owned_by": "kiro-cli",
    })
}

pub fn kiro_provider_core_acp_assistant_output_message(text: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpAssistantOutput);
        input.content = Some(text);
        kiro_acp_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": text,
        }],
    })
}

pub fn kiro_provider_core_acp_response_value(
    response_id: &str,
    created_at: u64,
    model: &str,
    output: Vec<Value>,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let output = serde_json::to_string(&output).expect("Kiro ACP output serializes");
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpResponse);
        input.response_id = Some(response_id);
        input.created_at = created_at;
        input.model = Some(model);
        input.output = Some(&output);
        kiro_acp_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "id": response_id,
        "object": "response",
        "created_at": created_at,
        "model": model,
        "output": output,
    })
}

pub fn kiro_provider_core_acp_chat_assistant_message(
    assistant_text: &str,
    reasoning_text: &str,
    tool_calls: Vec<Value>,
) -> Option<Value> {
    #[cfg(feature = "mojo")]
    {
        let tool_calls = serde_json::to_string(&tool_calls).expect("Kiro ACP tool calls serialize");
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpChatAssistant);
        input.content = Some(assistant_text);
        input.reason = Some(reasoning_text);
        input.tool_calls = Some(&tool_calls);
        kiro_acp_mojo_optional_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        if assistant_text.is_empty() && reasoning_text.is_empty() && tool_calls.is_empty() {
            return None;
        }
        let has_tool_calls = !tool_calls.is_empty();
        let mut assistant = json!({
            "role": "assistant",
            "content": if assistant_text.is_empty() {
                if has_tool_calls {
                    Value::String(String::new())
                } else {
                    Value::Null
                }
            } else {
                Value::String(assistant_text.to_string())
            },
        });
        if !reasoning_text.is_empty() {
            assistant["reasoning_content"] = Value::String(reasoning_text.to_string());
        }
        if has_tool_calls {
            assistant["tool_calls"] = Value::Array(tool_calls);
        }
        Some(assistant)
    }
}

pub fn kiro_provider_core_acp_plan_entry(content: &str, priority: &str, status: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpPlanEntry);
        input.content = Some(content);
        input.reason = Some(priority);
        input.status = Some(status);
        kiro_acp_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "content": content,
        "priority": priority,
        "status": status,
    })
}

pub fn kiro_provider_core_acp_error_value(code: i64, message: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let code = code.to_string();
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpError);
        input.call_id = Some(&code);
        input.content = Some(message);
        kiro_acp_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "code": code.to_string(),
        "message": message,
    })
}

pub fn kiro_provider_core_acp_mark_failed_response(response: &mut Value, code: i64, message: &str) {
    response["status"] = Value::String("failed".to_string());
    response["error"] = kiro_provider_core_acp_error_value(code, message);
}

pub fn kiro_provider_core_acp_session_info(title: Option<&str>, updated_at: Option<&str>) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpSessionInfo);
        input.name = title;
        input.status = updated_at;
        kiro_acp_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "title": title,
        "updated_at": updated_at,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn kiro_provider_core_acp_metadata(
    reasoning_text: &str,
    usage_update: Option<Value>,
    plan_entries: Option<Vec<Value>>,
    available_commands: Option<Vec<Value>>,
    current_mode_id: Option<&str>,
    session_title: Option<&str>,
    session_updated_at: Option<&str>,
    stop_reason: Option<&str>,
    tool_activities: Vec<Value>,
) -> Option<Value> {
    #[cfg(feature = "mojo")]
    {
        let usage_update = usage_update
            .as_ref()
            .map(|value| serde_json::to_string(value).expect("Kiro ACP usage serializes"));
        let plan_entries = plan_entries
            .as_ref()
            .map(|value| serde_json::to_string(value).expect("Kiro ACP plan serializes"));
        let available_commands = available_commands
            .as_ref()
            .map(|value| serde_json::to_string(value).expect("Kiro ACP commands serialize"));
        let tool_activities =
            serde_json::to_string(&tool_activities).expect("Kiro ACP tool activities serialize");
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpMetadata);
        input.reason = Some(reasoning_text);
        input.input = usage_update.as_deref();
        input.output = plan_entries.as_deref();
        input.tool_calls = available_commands.as_deref();
        input.model = current_mode_id;
        input.name = session_title;
        input.status = session_updated_at;
        input.finish_reason = stop_reason;
        input.extra = Some(&tool_activities);
        kiro_acp_mojo_optional_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        let mut tool_activities = tool_activities;
        let mut kiro_metadata = serde_json::Map::new();
        if !reasoning_text.is_empty() {
            kiro_metadata.insert(
                "reasoning_content".to_string(),
                Value::String(reasoning_text.to_string()),
            );
        }
        if let Some(usage_update) = usage_update {
            kiro_metadata.insert("usage_update".to_string(), usage_update);
        }
        if let Some(plan_entries) = plan_entries {
            kiro_metadata.insert("plan".to_string(), Value::Array(plan_entries));
        }
        if let Some(available_commands) = available_commands {
            kiro_metadata.insert(
                "available_commands".to_string(),
                Value::Array(available_commands),
            );
        }
        if let Some(current_mode_id) = current_mode_id {
            kiro_metadata.insert(
                "current_mode_id".to_string(),
                Value::String(current_mode_id.to_string()),
            );
        }
        if session_title.is_some() || session_updated_at.is_some() {
            kiro_metadata.insert(
                "session_info".to_string(),
                kiro_provider_core_acp_session_info(session_title, session_updated_at),
            );
        }
        if let Some(stop_reason) = stop_reason {
            kiro_metadata.insert(
                "stop_reason".to_string(),
                Value::String(stop_reason.to_string()),
            );
        }
        if tool_activities.len() > KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_EVENTS {
            tool_activities.truncate(KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_EVENTS.saturating_sub(1));
            tool_activities.push(kiro_provider_core_truncated_tool_activity_item());
        }
        if !tool_activities.is_empty() {
            kiro_metadata.insert("tool_activities".to_string(), Value::Array(tool_activities));
        }
        if kiro_metadata.is_empty() {
            return None;
        }
        Some(json!({ "kiro": kiro_metadata }))
    }
}

pub fn kiro_provider_core_acp_stop_reason(result: Option<&Value>) -> Option<String> {
    result
        .and_then(|result| {
            result
                .get("stopReason")
                .or_else(|| result.get("stop_reason"))
                .or_else(|| result.get("status"))
        })
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub fn kiro_provider_core_acp_incomplete_details(
    stop_reason: Option<&str>,
) -> Option<(&'static str, &'static str)> {
    match stop_reason {
        Some("max_tokens") => Some((
            "max_output_tokens",
            "Kiro stopped before end_turn because the model hit its output limit.",
        )),
        Some("max_turn_requests") => Some((
            "max_turn_requests",
            "Kiro stopped before end_turn because the turn hit its request limit.",
        )),
        Some("refusal") => Some(("refusal", "Kiro refused to continue the turn.")),
        Some("cancelled") => Some(("cancelled", "Kiro cancelled the turn before completion.")),
        _ => None,
    }
}

pub fn kiro_provider_core_acp_incomplete_details_value(reason: &str, message: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::AcpIncompleteDetails);
        input.reason = Some(reason);
        input.content = Some(message);
        kiro_acp_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "reason": reason,
        "message": message,
    })
}

pub fn kiro_provider_core_acp_mark_incomplete_response(
    response: &mut Value,
    reason: &str,
    message: &str,
) {
    response["status"] = Value::String("incomplete".to_string());
    response["incomplete_details"] =
        kiro_provider_core_acp_incomplete_details_value(reason, message);
}
