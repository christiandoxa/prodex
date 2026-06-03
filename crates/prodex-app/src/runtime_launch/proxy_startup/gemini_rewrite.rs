use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_chat_request_body,
    runtime_deepseek_created_at, runtime_deepseek_rtk_wrapped_tool_arguments,
    runtime_deepseek_store_conversation,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use anyhow::{Context, Result};
use prodex_cli::SUPER_GEMINI_DEFAULT_MODEL;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;

#[path = "gemini_schema.rs"]
mod gemini_schema;

use gemini_schema::runtime_gemini_sanitize_function_schema;

#[derive(Clone)]
pub(crate) enum RuntimeGeminiAuth {
    ApiKey {
        api_key: String,
    },
    OAuth {
        access_token: String,
        project_id: Option<String>,
    },
}

#[derive(Clone)]
pub(crate) enum RuntimeGeminiProviderAuth {
    ApiKey {
        api_key: String,
    },
    OAuthProfiles {
        profiles: Vec<RuntimeGeminiOAuthProfileAuth>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeGeminiOAuthProfileAuth {
    pub(crate) profile_name: String,
    pub(crate) codex_home: PathBuf,
    pub(crate) email: Option<String>,
    pub(crate) access_token: String,
    pub(crate) project_id: Option<String>,
}

impl RuntimeGeminiOAuthProfileAuth {
    pub(crate) fn auth(&self) -> RuntimeGeminiAuth {
        RuntimeGeminiAuth::OAuth {
            access_token: self.access_token.clone(),
            project_id: self.project_id.clone(),
        }
    }
}

pub(super) struct RuntimeGeminiTranslatedRequest {
    pub(super) body: Vec<u8>,
    pub(super) messages: Vec<serde_json::Value>,
    pub(super) model: String,
    pub(super) stream: bool,
}

pub(super) fn runtime_gemini_generate_request_body(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
    code_assist: bool,
    project_id: Option<&str>,
) -> Result<RuntimeGeminiTranslatedRequest> {
    let original: serde_json::Value =
        serde_json::from_slice(body).context("failed to parse Codex Responses request JSON")?;
    let chat = runtime_deepseek_chat_request_body(body, conversations)?;
    let chat_value: serde_json::Value = serde_json::from_slice(&chat.body)
        .context("failed to parse translated chat request JSON")?;
    let model = chat_value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(SUPER_GEMINI_DEFAULT_MODEL)
        .to_string();
    let stream = chat_value
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let mut request = serde_json::Map::new();
    if let Some(system_instruction) = runtime_gemini_system_instruction(&chat_value) {
        request.insert("systemInstruction".to_string(), system_instruction);
    }
    request.insert(
        "contents".to_string(),
        serde_json::Value::Array(runtime_gemini_contents_from_chat(&chat_value)),
    );
    if let Some(tools) = runtime_gemini_tools_from_chat(&chat_value) {
        request.insert("tools".to_string(), tools);
    }
    if let Some(tool_config) = runtime_gemini_tool_config_from_chat(&chat_value) {
        request.insert("toolConfig".to_string(), tool_config);
    }
    request.insert(
        "generationConfig".to_string(),
        runtime_gemini_generation_config(&original, &chat_value, &model),
    );

    let body_value = if code_assist {
        serde_json::json!({
            "model": model,
            "project": project_id,
            "request": serde_json::Value::Object(request),
        })
    } else {
        serde_json::Value::Object(request)
    };
    let body = serde_json::to_vec(&body_value)
        .context("failed to serialize Gemini generateContent request JSON")?;
    Ok(RuntimeGeminiTranslatedRequest {
        body,
        messages: chat.messages,
        model,
        stream,
    })
}

pub(super) fn runtime_gemini_generate_buffered_response_parts(
    status: u16,
    mut response: reqwest::blocking::Response,
    request_id: u64,
    conversation_messages: Vec<serde_json::Value>,
    conversations: &RuntimeDeepSeekConversationStore,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let mut body = Vec::new();
    response
        .read_to_end(&mut body)
        .context("failed to read Gemini response body")?;
    let value: serde_json::Value =
        serde_json::from_slice(&body).context("failed to parse Gemini response JSON")?;
    let value = runtime_gemini_normalized_response_value(&value);
    let response = runtime_gemini_responses_value_from_generate_value(&value, request_id);
    if let Some(response_id) = response.get("id").and_then(serde_json::Value::as_str) {
        runtime_deepseek_store_conversation(
            conversations,
            response_id,
            conversation_messages,
            runtime_gemini_chat_assistant_messages_from_generate_value(&value, request_id),
        );
    }
    let body = serde_json::to_vec(&response).context("failed to serialize Responses JSON")?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    })
}

pub(super) fn runtime_gemini_normalized_response_value(
    value: &serde_json::Value,
) -> Cow<'_, serde_json::Value> {
    let Some(response) = value.get("response") else {
        return Cow::Borrowed(value);
    };
    let Some(trace_id) = value.get("traceId").and_then(serde_json::Value::as_str) else {
        return Cow::Borrowed(response);
    };
    let mut response = response.clone();
    if let Some(object) = response.as_object_mut() {
        object.insert(
            "responseId".to_string(),
            serde_json::Value::String(trace_id.to_string()),
        );
    }
    Cow::Owned(response)
}

pub(super) fn runtime_gemini_upstream_url(
    base_url: &str,
    auth: &RuntimeGeminiAuth,
    model: &str,
    stream: bool,
) -> String {
    let method = if stream {
        "streamGenerateContent"
    } else {
        "generateContent"
    };
    match auth {
        RuntimeGeminiAuth::ApiKey { .. } => {
            let model_path = if model.starts_with("models/") {
                model.to_string()
            } else {
                format!("models/{model}")
            };
            let suffix = if stream {
                format!("/{model_path}:{method}?alt=sse")
            } else {
                format!("/{model_path}:{method}")
            };
            format!("{}{}", base_url.trim_end_matches('/'), suffix)
        }
        RuntimeGeminiAuth::OAuth { .. } => {
            let mut url = format!("{}:{method}", crate::gemini_code_assist_endpoint());
            if stream {
                url.push_str("?alt=sse");
            }
            url
        }
    }
}

pub(super) fn runtime_gemini_project_id(auth: &RuntimeGeminiAuth) -> Option<&str> {
    match auth {
        RuntimeGeminiAuth::ApiKey { .. } => None,
        RuntimeGeminiAuth::OAuth { project_id, .. } => project_id.as_deref(),
    }
}

pub(super) fn runtime_gemini_responses_usage(
    usage: &serde_json::Value,
) -> Option<serde_json::Value> {
    let input_tokens = usage
        .get("promptTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("candidatesTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("totalTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    Some(serde_json::json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": total_tokens,
    }))
}

pub(super) fn runtime_gemini_chat_assistant_messages_from_generate_value(
    value: &serde_json::Value,
    request_id: u64,
) -> Vec<serde_json::Value> {
    let Some(parts) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    let mut text = String::new();
    let mut reasoning_content = String::new();
    let mut tool_calls = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if let Some(part_text) = part.get("text").and_then(serde_json::Value::as_str) {
            if part
                .get("thought")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                reasoning_content.push_str(part_text);
            } else {
                text.push_str(part_text);
            }
        }
        if let Some(function_call) = part.get("functionCall") {
            let name = function_call
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool_call");
            let call_id = runtime_gemini_function_call_id(function_call, request_id, index);
            let args = function_call
                .get("args")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let args = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
            let mut tool_call = serde_json::json!({
                "id": call_id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": runtime_deepseek_rtk_wrapped_tool_arguments(name, &args),
                },
            });
            if let Some(signature) = runtime_gemini_thought_signature(part)
                .or_else(|| runtime_gemini_thought_signature(function_call))
            {
                tool_call["gemini_thought_signature"] = serde_json::Value::String(signature);
            }
            tool_calls.push(tool_call);
        }
    }
    if text.is_empty() && reasoning_content.is_empty() && tool_calls.is_empty() {
        return Vec::new();
    }
    let mut assistant = serde_json::json!({
        "role": "assistant",
        "content": if text.is_empty() {
            if tool_calls.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(String::new())
            }
        } else {
            serde_json::Value::String(text)
        },
    });
    if !reasoning_content.is_empty() {
        assistant["reasoning_content"] = serde_json::Value::String(reasoning_content);
    }
    if !tool_calls.is_empty() {
        assistant["tool_calls"] = serde_json::Value::Array(tool_calls);
    }
    vec![assistant]
}

fn runtime_gemini_responses_value_from_generate_value(
    value: &serde_json::Value,
    request_id: u64,
) -> serde_json::Value {
    let response_id = runtime_gemini_response_id(value, request_id);
    let model = runtime_gemini_model(value);
    let mut output = Vec::new();
    if let Some(parts) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    {
        let mut text = String::new();
        for (index, part) in parts.iter().enumerate() {
            if let Some(part_text) = part.get("text").and_then(serde_json::Value::as_str)
                && !part
                    .get("thought")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            {
                text.push_str(part_text);
            }
            if let Some(function_call) = part.get("functionCall") {
                output.push(runtime_gemini_responses_tool_call_item(
                    function_call,
                    request_id,
                    index,
                ));
            }
        }
        if !text.is_empty() {
            output.insert(
                0,
                serde_json::json!({
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": text,
                    }],
                }),
            );
        }
    }
    let mut response = serde_json::json!({
        "id": response_id,
        "object": "response",
        "created_at": runtime_deepseek_created_at(),
        "model": model,
        "output": output,
    });
    if let Some(usage) = value
        .get("usageMetadata")
        .and_then(runtime_gemini_responses_usage)
    {
        response["usage"] = usage;
    }
    response
}

fn runtime_gemini_contents_from_chat(chat: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut contents = Vec::new();
    let mut tool_names_by_call_id = BTreeMap::new();
    let Some(messages) = chat.get("messages").and_then(serde_json::Value::as_array) else {
        return contents;
    };
    for message in messages {
        let role = message
            .get("role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("user");
        match role {
            "system" => {}
            "assistant" => {
                let mut parts = Vec::new();
                if let Some(content) = chat_message_text(message).filter(|text| !text.is_empty()) {
                    parts.push(serde_json::json!({ "text": content }));
                }
                if let Some(tool_calls) = message
                    .get("tool_calls")
                    .and_then(serde_json::Value::as_array)
                {
                    for tool_call in tool_calls {
                        let call_id = tool_call
                            .get("id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default();
                        if let Some(function) = tool_call.get("function") {
                            let name = function
                                .get("name")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("tool_call");
                            if !call_id.is_empty() {
                                tool_names_by_call_id.insert(call_id.to_string(), name.to_string());
                            }
                            let args = function
                                .get("arguments")
                                .and_then(serde_json::Value::as_str)
                                .and_then(|args| {
                                    serde_json::from_str::<serde_json::Value>(args).ok()
                                })
                                .unwrap_or_else(|| serde_json::json!({}));
                            let function_call =
                                runtime_gemini_function_call_part(call_id, name, args);
                            let mut part = serde_json::json!({
                                "functionCall": function_call,
                            });
                            if let Some(signature) = tool_call
                                .get("gemini_thought_signature")
                                .or_else(|| function.get("gemini_thought_signature"))
                                .and_then(serde_json::Value::as_str)
                                .filter(|signature| !signature.trim().is_empty())
                            {
                                part["thoughtSignature"] =
                                    serde_json::Value::String(signature.to_string());
                            }
                            parts.push(part);
                        }
                    }
                }
                if !parts.is_empty() {
                    contents.push(serde_json::json!({
                        "role": "model",
                        "parts": parts,
                    }));
                }
            }
            "tool" => {
                let call_id = message
                    .get("tool_call_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let name = message
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
                    .or_else(|| tool_names_by_call_id.get(call_id).cloned())
                    .unwrap_or_else(|| "tool_call".to_string());
                let response = chat_message_text(message)
                    .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                    .unwrap_or_else(|| {
                        serde_json::json!({
                            "output": chat_message_text(message).unwrap_or_default()
                        })
                    });
                let function_response =
                    runtime_gemini_function_response_part(call_id, &name, response);
                contents.push(serde_json::json!({
                    "role": "user",
                    "parts": [{
                        "functionResponse": function_response,
                    }],
                }));
            }
            _ => {
                contents.push(serde_json::json!({
                    "role": "user",
                    "parts": [{ "text": chat_message_text(message).unwrap_or_default() }],
                }));
            }
        }
    }
    if contents.is_empty() {
        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{ "text": "" }],
        }));
    }
    contents
}

fn runtime_gemini_function_call_part(
    call_id: &str,
    name: &str,
    args: serde_json::Value,
) -> serde_json::Value {
    let mut call = serde_json::json!({
        "name": name,
        "args": args,
    });
    if !call_id.trim().is_empty() {
        call["id"] = serde_json::Value::String(call_id.to_string());
    }
    call
}

fn runtime_gemini_function_response_part(
    call_id: &str,
    name: &str,
    response: serde_json::Value,
) -> serde_json::Value {
    let mut function_response = serde_json::json!({
        "name": name,
        "response": response,
    });
    if !call_id.trim().is_empty() {
        function_response["id"] = serde_json::Value::String(call_id.to_string());
    }
    function_response
}

fn runtime_gemini_system_instruction(chat: &serde_json::Value) -> Option<serde_json::Value> {
    let messages = chat.get("messages")?.as_array()?;
    let text = messages
        .iter()
        .filter(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("system"))
        .filter_map(chat_message_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    (!text.trim().is_empty()).then(|| serde_json::json!({ "parts": [{ "text": text }] }))
}

fn runtime_gemini_tools_from_chat(chat: &serde_json::Value) -> Option<serde_json::Value> {
    let declarations = chat
        .get("tools")?
        .as_array()?
        .iter()
        .filter_map(runtime_gemini_function_declaration_from_chat_tool)
        .collect::<Vec<_>>();
    (!declarations.is_empty()).then(|| {
        serde_json::json!([{
            "functionDeclarations": declarations,
        }])
    })
}

fn runtime_gemini_function_declaration_from_chat_tool(
    tool: &serde_json::Value,
) -> Option<serde_json::Value> {
    let function = tool.get("function")?;
    let name = function.get("name").and_then(serde_json::Value::as_str)?;
    let default_parameters = serde_json::json!({"type": "object"});
    let parameters = function.get("parameters").unwrap_or(&default_parameters);
    let mut declaration = serde_json::json!({
        "name": name,
        "parameters": runtime_gemini_sanitize_function_schema(parameters),
    });
    if let Some(description) = function
        .get("description")
        .and_then(serde_json::Value::as_str)
    {
        declaration["description"] = serde_json::Value::String(description.to_string());
    }
    Some(declaration)
}

fn runtime_gemini_tool_config_from_chat(chat: &serde_json::Value) -> Option<serde_json::Value> {
    let tool_choice = chat.get("tool_choice")?;
    if tool_choice.as_str() == Some("auto") {
        return None;
    }
    if tool_choice.as_str() == Some("none") {
        return Some(serde_json::json!({
            "functionCallingConfig": {
                "mode": "NONE",
            }
        }));
    }
    if tool_choice.as_str() == Some("required") {
        return Some(serde_json::json!({
            "functionCallingConfig": {
                "mode": "ANY",
            }
        }));
    }
    let name = tool_choice
        .get("function")
        .and_then(|function| function.get("name"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| tool_choice.get("name").and_then(serde_json::Value::as_str))?;
    Some(serde_json::json!({
        "functionCallingConfig": {
            "mode": "ANY",
            "allowedFunctionNames": [name],
        }
    }))
}

fn runtime_gemini_generation_config(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
) -> serde_json::Value {
    let mut config = serde_json::Map::new();
    for (from, to) in [
        ("temperature", "temperature"),
        ("top_p", "topP"),
        ("max_tokens", "maxOutputTokens"),
    ] {
        if let Some(value) = chat.get(from) {
            config.insert(to.to_string(), value.clone());
        }
    }
    if let Some(thinking_config) = runtime_gemini_thinking_config(original, model) {
        config.insert("thinkingConfig".to_string(), thinking_config);
    }
    serde_json::Value::Object(config)
}

fn runtime_gemini_thinking_config(
    original: &serde_json::Value,
    model: &str,
) -> Option<serde_json::Value> {
    let effort = original
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("high")
        .to_ascii_lowercase();
    if effort == "none" || effort == "minimal" {
        return Some(serde_json::json!({
            "includeThoughts": false,
            "thinkingBudget": 0,
        }));
    }
    if runtime_gemini_model_uses_thinking_level(model) {
        let level = match effort.as_str() {
            "low" => "LOW",
            "medium" => "MEDIUM",
            _ => "HIGH",
        };
        return Some(serde_json::json!({
            "includeThoughts": true,
            "thinkingLevel": level,
        }));
    }
    let budget = match effort.as_str() {
        "low" => 1024,
        "medium" | "high" => 8192,
        "xhigh" => 24576,
        _ => 8192,
    };
    Some(serde_json::json!({
        "includeThoughts": true,
        "thinkingBudget": budget,
    }))
}

fn runtime_gemini_model_uses_thinking_level(model: &str) -> bool {
    model.contains("gemini-3") || model.contains("gemma-3") || model.contains("gemma-4")
}

fn chat_message_text(message: &serde_json::Value) -> Option<String> {
    match message.get("content") {
        Some(serde_json::Value::String(text)) => Some(text.clone()),
        Some(serde_json::Value::Array(items)) => Some(
            items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(serde_json::Value::as_str)
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => None,
    }
}

fn runtime_gemini_response_id(value: &serde_json::Value, request_id: u64) -> String {
    value
        .get("responseId")
        .or_else(|| value.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("resp_gemini_{request_id}"))
}

fn runtime_gemini_model(value: &serde_json::Value) -> String {
    value
        .get("modelVersion")
        .or_else(|| value.get("model"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(SUPER_GEMINI_DEFAULT_MODEL)
        .to_string()
}

fn runtime_gemini_responses_tool_call_item(
    function_call: &serde_json::Value,
    request_id: u64,
    index: usize,
) -> serde_json::Value {
    let call_id = runtime_gemini_function_call_id(function_call, request_id, index);
    let name = function_call
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool_call");
    let args = function_call
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let args = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
    serde_json::json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": runtime_deepseek_rtk_wrapped_tool_arguments(name, &args),
    })
}

fn runtime_gemini_function_call_id(
    function_call: &serde_json::Value,
    request_id: u64,
    index: usize,
) -> String {
    function_call
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("call_gemini_{request_id}_{index}"))
}

fn runtime_gemini_thought_signature(part: &serde_json::Value) -> Option<String> {
    part.get("thoughtSignature")
        .or_else(|| part.get("thought_signature"))
        .and_then(serde_json::Value::as_str)
        .filter(|signature| !signature.trim().is_empty())
        .map(str::to_string)
}

#[cfg(test)]
#[path = "gemini_rewrite_tests.rs"]
mod gemini_rewrite_tests;
