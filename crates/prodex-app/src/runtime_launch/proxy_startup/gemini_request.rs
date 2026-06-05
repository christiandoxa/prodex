use super::super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_chat_request_body,
};
use super::RuntimeGeminiTranslatedRequest;
use super::gemini_schema::runtime_gemini_sanitize_function_schema;
use anyhow::{Context, Result};
use prodex_cli::SUPER_GEMINI_DEFAULT_MODEL;
use std::collections::BTreeMap;

const PRODEX_GEMINI_CODEX_PARITY_INSTRUCTION: &str = "\
You are running inside Codex through the Prodex Gemini bridge. Match Codex tool workflows exactly: \
use available edit/apply_patch tools to change files instead of only describing edits; use shell/process tools to run commands; \
when a command returns a running session id, call the wait/read follow-up tool until the process exits or yields the needed output; \
when explaining file changes, use unified diff format only as a human-readable summary, not as a substitute for applying edits; \
use available web_search, tool_search, and MCP tools when the task calls for them.";

pub(in super::super) fn runtime_gemini_generate_request_body(
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
    if let Some(tools) = runtime_gemini_tools_from_requests(&original, &chat_value) {
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

pub(in super::super) fn runtime_gemini_request_body_without_google_search(
    body: &[u8],
) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let request = runtime_gemini_request_object_mut(&mut value)?;
    let tools = request.get_mut("tools")?.as_array_mut()?;
    let original_len = tools.len();
    tools.retain(|tool| {
        !tool
            .as_object()
            .map(|object| object.contains_key("googleSearch"))
            .unwrap_or(false)
    });
    if tools.len() == original_len {
        return None;
    }
    if tools.is_empty() {
        request.remove("tools");
    }
    serde_json::to_vec(&value).ok()
}

fn runtime_gemini_request_object_mut(
    value: &mut serde_json::Value,
) -> Option<&mut serde_json::Map<String, serde_json::Value>> {
    if value.get("request").is_some() {
        value.get_mut("request")?.as_object_mut()
    } else {
        value.as_object_mut()
    }
}

fn runtime_gemini_contents_from_chat(chat: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut contents = Vec::new();
    let mut tool_names_by_call_id = BTreeMap::new();
    let Some(messages) = chat.get("messages").and_then(serde_json::Value::as_array) else {
        return contents;
    };
    let mut index = 0;
    while index < messages.len() {
        let message = &messages[index];
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
                let mut parts = Vec::new();
                while index < messages.len()
                    && messages[index]
                        .get("role")
                        .and_then(serde_json::Value::as_str)
                        == Some("tool")
                {
                    parts.push(serde_json::json!({
                        "functionResponse": runtime_gemini_function_response_from_tool_message(
                            &messages[index],
                            &tool_names_by_call_id,
                        ),
                    }));
                    index += 1;
                }
                contents.push(serde_json::json!({
                    "role": "user",
                    "parts": parts,
                }));
                continue;
            }
            _ => {
                contents.push(serde_json::json!({
                    "role": "user",
                    "parts": [{ "text": chat_message_text(message).unwrap_or_default() }],
                }));
            }
        }
        index += 1;
    }
    if contents.is_empty() {
        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{ "text": "" }],
        }));
    }
    contents
}

fn runtime_gemini_function_response_from_tool_message(
    message: &serde_json::Value,
    tool_names_by_call_id: &BTreeMap<String, String>,
) -> serde_json::Value {
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
    runtime_gemini_function_response_part(call_id, &name, response)
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
    let mut system_text = messages
        .iter()
        .filter(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("system"))
        .filter_map(chat_message_text)
        .collect::<Vec<_>>()
        .join("\n\n");

    if !system_text.is_empty() {
        system_text.push_str("\n\n");
        system_text.push_str(PRODEX_GEMINI_CODEX_PARITY_INSTRUCTION);
    } else {
        system_text = PRODEX_GEMINI_CODEX_PARITY_INSTRUCTION.to_string();
    }

    (!system_text.trim().is_empty())
        .then(|| serde_json::json!({ "parts": [{ "text": system_text }] }))
}

fn runtime_gemini_tools_from_requests(
    original: &serde_json::Value,
    chat: &serde_json::Value,
) -> Option<serde_json::Value> {
    let mut tools = Vec::new();
    if runtime_gemini_web_search_enabled(original) {
        tools.push(serde_json::json!({ "googleSearch": {} }));
    }
    if let Some(serde_json::Value::Array(function_tools)) =
        runtime_gemini_function_tools_from_chat(chat)
    {
        tools.extend(function_tools);
    }
    (!tools.is_empty()).then_some(serde_json::Value::Array(tools))
}

fn runtime_gemini_web_search_enabled(original: &serde_json::Value) -> bool {
    original
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| tools.iter().any(runtime_gemini_is_web_search_tool))
}

fn runtime_gemini_is_web_search_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    tool_type == "web_search"
        || tool_type == "web_search_preview"
        || tool_type.starts_with("web_search_preview_")
}

fn runtime_gemini_function_tools_from_chat(chat: &serde_json::Value) -> Option<serde_json::Value> {
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
