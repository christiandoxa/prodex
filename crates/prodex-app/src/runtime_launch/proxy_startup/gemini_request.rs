use super::super::chat_compatible_rewrite::{
    RuntimeChatCompatibleConversationStore, RuntimeDeepSeekRewriteOptions,
    runtime_provider_chat_compatible_request_body,
};
use super::super::provider_bridge::RuntimeProviderBridgeKind;
use super::RuntimeGeminiTranslatedRequest;
#[cfg(test)]
use super::gemini_request_extensions::{
    runtime_gemini_active_extension_manifests_from_roots,
    runtime_gemini_extension_context_files_from_roots,
};
#[cfg(test)]
use super::gemini_request_policy::RuntimeGeminiPolicyCompat;
#[cfg(test)]
use super::gemini_request_policy::runtime_gemini_settings_paths_for;
use super::gemini_request_session::{
    runtime_gemini_export_checkpoint, runtime_gemini_imported_session_contents,
};
use crate::RuntimeGeminiConfig;
use prodex_provider_core::{
    gemini_provider_core_contextual_user_instruction_text as runtime_gemini_contextual_user_instruction_text,
    gemini_provider_core_generation_config_from_request as runtime_gemini_generation_config,
    gemini_provider_core_harden_contents as runtime_gemini_harden_contents,
    gemini_provider_core_media_part_from_data as runtime_gemini_media_part_from_data,
    gemini_provider_core_media_part_from_uri_or_data_url as runtime_gemini_media_part_from_uri_or_data_url,
    gemini_provider_core_structured_command_tool_response as runtime_gemini_structured_command_tool_response,
    gemini_provider_core_tool_config_from_request as runtime_gemini_tool_config_from_chat,
};
mod gemini_request_chat_source;
mod gemini_request_context;
mod gemini_request_instruction;
mod gemini_request_local_context;
mod gemini_request_memory;
mod gemini_request_tools;
mod gemini_request_util;
use super::gemini_request_tool_output::runtime_gemini_mask_tool_response_for_history;
use anyhow::{Context, Result};
use gemini_request_chat_source::runtime_gemini_chat_source_request;
use gemini_request_context::{
    runtime_gemini_collect_at_path_parts, runtime_gemini_collect_explicit_file_parts,
};
use gemini_request_instruction::runtime_gemini_system_instruction;
use gemini_request_local_context::{
    RuntimeGeminiFileReadBudget, runtime_gemini_part_from_local_path,
};
use gemini_request_memory::runtime_gemini_hierarchical_memory;
#[cfg(test)]
use gemini_request_memory::runtime_gemini_memory_files_enabled;
#[cfg(test)]
pub(in super::super) use gemini_request_tools::runtime_gemini_blocked_tool_call_message;
pub(in super::super) use gemini_request_tools::runtime_gemini_blocked_tool_call_message_with_config;
use gemini_request_tools::runtime_gemini_tools_from_requests;
pub(super) use gemini_request_util::{runtime_gemini_config_dir, runtime_gemini_home_dir};
use prodex_runtime_gemini::GEMINI_DEFAULT_MODEL;
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

pub(super) const PRODEX_GEMINI_CODEX_PARITY_INSTRUCTION: &str = "\
You are Codex CLI, a coding agent. The user must experience native Codex CLI, not a Gemini assistant. \
Follow the active Codex, developer, AGENTS.md, and repository instructions exactly. \
Do not mention Prodex bridge internals, Gemini, backend provider details, or task completion status unless directly relevant to the user's request. \
Do not add closing meta-statements such as \"I have completed the request\" or \"I will consider this task complete\". \
Keep final responses in Codex CLI style: direct, brief, and focused on files changed, tests run, and unresolved blockers. \
Use the user's language for conversation unless repository prose or code content must stay in another language. \
If the user requests an exact string, answer-only output, or command output only, emit only that requested output and nothing else. \
For exact-output prompts, do not include explanations, diffs, status, previous-turn recaps, or extra sentences before or after the requested output. \
When final output is requested from a tool result, the final answer must match the tool output after trimming trailing newlines. \
After a required tool call has produced the requested answer, stop and do not call unrelated tools. \
Do not invoke MCP, project, search, or filesystem tools opportunistically; use them only when the current user request requires them. \
Match Codex tool workflows exactly: \
use available edit/apply_patch tools to change files instead of only describing edits; use shell/process tools to run commands; \
when a command returns a running session id, call the wait/read follow-up tool until the process exits or yields the needed output; \
when a command/tool result is marked failed, has a non-zero exit code, or reports missing paths, do not use its expected output paths or claim success until a follow-up command verifies the recovered state; \
when explaining file changes, use unified diff format only as a human-readable summary, not as a substitute for applying edits; \
use available web_search, tool_search, and MCP tools when the task calls for them. \
If the user asks you to monitor or fix GitHub Actions, use the local gh CLI with saved credentials when available; \
gh run watch is only a status view, so on failure inspect logs with gh run view <run-id> --log-failed, gh run view <run-id> --json jobs, or gh run view --job <job-id> --log. \
If a publish workflow fails while waiting for CI success, read that step log, follow the target CI run URL/SHA, inspect the failed CI job logs, and reproduce the exact failing local command before declaring blocked.";
pub(super) const PRODEX_GEMINI_TOOL_DISCIPLINE_INSTRUCTION: &str = "\
Tool discipline for Codex parity: do not install, upgrade, curl, clone, or browse for tooling unless the user explicitly asked for that action or local evidence proves it is required. \
When updating local tools, first inspect the actual executable path and local checkout/config, then use the installer or package manager that owns that path. \
For optional-tool update workflows, inspect OWNER/install files with normal shell or file tools; do not use SQZ, Token Savior, Claw Compactor, or other optimizer MCP/tools for local file reads unless the user explicitly asks for optimizer diagnostics. \
If two commands fail for the same install/update target, stop trying random package names or URLs and switch to local source/config inspection. \
Do not narrate repeated wait/poll steps; if a command is still running, wait for it with the follow-up tool and report only new information. \
Final success, latest-version, up-to-date, or no-blocker claims must be backed by an explicit verification command result observed after the relevant action; otherwise report the unresolved uncertainty.";
pub(super) const RUNTIME_GEMINI_MEMORY_BYTE_LIMIT: usize = 64 * 1024;
pub(super) const RUNTIME_GEMINI_IMPORT_BYTE_LIMIT: usize = 256 * 1024;
pub(super) const RUNTIME_GEMINI_EXTENSION_SCAN_LIMIT: usize = 64;
pub(super) const RUNTIME_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD: usize =
    RuntimeGeminiConfig::DEFAULT_TOOL_OUTPUT_MASK_THRESHOLD;
pub(super) const RUNTIME_GEMINI_TOOL_OUTPUT_PREVIEW_CHARS: usize = 1_000;

#[cfg(test)]
pub(in super::super) fn runtime_gemini_generate_request_body(
    body: &[u8],
    conversations: &RuntimeChatCompatibleConversationStore,
    code_assist: bool,
    project_id: Option<&str>,
    thinking_budget_tokens: Option<u64>,
) -> Result<RuntimeGeminiTranslatedRequest> {
    let config = crate::RuntimeConfig::compatibility_current();
    runtime_gemini_generate_request_body_with_config(
        body,
        conversations,
        code_assist,
        project_id,
        thinking_budget_tokens,
        &config.gemini,
    )
}

pub(in super::super) fn runtime_gemini_generate_request_body_with_config(
    body: &[u8],
    conversations: &RuntimeChatCompatibleConversationStore,
    code_assist: bool,
    project_id: Option<&str>,
    thinking_budget_tokens: Option<u64>,
    config: &RuntimeGeminiConfig,
) -> Result<RuntimeGeminiTranslatedRequest> {
    runtime_gemini_generate_request_body_with_local_file_access_and_config(
        body,
        conversations,
        code_assist,
        project_id,
        thinking_budget_tokens,
        true,
        config,
    )
}

#[cfg(test)]
pub(in super::super) fn runtime_gemini_generate_request_body_with_local_file_access(
    body: &[u8],
    conversations: &RuntimeChatCompatibleConversationStore,
    code_assist: bool,
    project_id: Option<&str>,
    thinking_budget_tokens: Option<u64>,
    allow_local_file_access: bool,
) -> Result<RuntimeGeminiTranslatedRequest> {
    let config = crate::RuntimeConfig::compatibility_current();
    runtime_gemini_generate_request_body_with_local_file_access_and_config(
        body,
        conversations,
        code_assist,
        project_id,
        thinking_budget_tokens,
        allow_local_file_access,
        &config.gemini,
    )
}

fn runtime_gemini_generate_request_body_with_local_file_access_and_config(
    body: &[u8],
    conversations: &RuntimeChatCompatibleConversationStore,
    code_assist: bool,
    project_id: Option<&str>,
    thinking_budget_tokens: Option<u64>,
    allow_local_file_access: bool,
    config: &RuntimeGeminiConfig,
) -> Result<RuntimeGeminiTranslatedRequest> {
    let original: serde_json::Value =
        serde_json::from_slice(body).context("failed to parse Codex Responses request JSON")?;
    let chat_source = runtime_gemini_chat_source_request(&original);
    let chat = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&chat_source).context("failed to serialize Gemini chat source JSON")?,
        conversations,
        RuntimeProviderBridgeKind::Gemini,
        GEMINI_DEFAULT_MODEL,
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )?;
    let mut chat_value: serde_json::Value = serde_json::from_slice(&chat.body)
        .context("failed to parse translated chat request JSON")?;
    if chat_value.get("tool_choice").is_none()
        && let Some(tool_choice) =
            super::super::provider_tools::runtime_provider_chat_tool_choice_from_responses_request(
                &original, false,
            )
    {
        chat_value["tool_choice"] = tool_choice;
    }
    let model = chat_value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(GEMINI_DEFAULT_MODEL)
        .to_string();
    let stream = chat_value
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let mut request = serde_json::Map::new();
    if let Some(system_instruction) =
        runtime_gemini_system_instruction(&chat_value, &original, allow_local_file_access, config)
    {
        request.insert("systemInstruction".to_string(), system_instruction);
    }
    request.insert(
        "contents".to_string(),
        serde_json::Value::Array(runtime_gemini_contents_from_chat(
            &chat_value,
            &original,
            allow_local_file_access,
            config,
        )),
    );
    if let Some(tools) = runtime_gemini_tools_from_requests(&original, &chat_value, &model, config)
    {
        request.insert("tools".to_string(), tools);
    }
    if let Some(tool_config) = runtime_gemini_tool_config_from_chat(&chat_value) {
        request.insert("toolConfig".to_string(), tool_config);
    }
    request.insert(
        "generationConfig".to_string(),
        runtime_gemini_generation_config(&original, &chat_value, &model, thinking_budget_tokens),
    );
    if let Some(settings) = original
        .get("safety_settings")
        .or_else(|| original.get("safetySettings"))
    {
        request.insert("safetySettings".to_string(), settings.clone());
    }
    if let Some(cached_content) = original
        .get("cached_content")
        .or_else(|| original.get("cachedContent"))
        .filter(|value| !value.is_null())
    {
        request.insert("cachedContent".to_string(), cached_content.clone());
    }
    if let Some(labels) = original.get("labels").filter(|value| !value.is_null()) {
        request.insert("labels".to_string(), labels.clone());
    }
    if allow_local_file_access {
        runtime_gemini_export_checkpoint(&original, &request, config)
            .context("failed to export Gemini session checkpoint")?;
    }

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

fn runtime_gemini_contents_from_chat(
    chat: &serde_json::Value,
    original: &serde_json::Value,
    allow_local_file_access: bool,
    config: &RuntimeGeminiConfig,
) -> Vec<serde_json::Value> {
    let mut contents = if allow_local_file_access {
        runtime_gemini_imported_session_contents(original, config)
    } else {
        Vec::new()
    };
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
                                .or_else(|| {
                                    tool_call
                                        .get("extra_content")
                                        .and_then(|value| value.get("google"))
                                        .and_then(|value| value.get("thought_signature"))
                                })
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
                if let Some(native_parts) = message
                    .get("gemini_native_parts")
                    .and_then(serde_json::Value::as_array)
                {
                    parts.extend(native_parts.iter().cloned());
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
                            allow_local_file_access,
                            config,
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
                if runtime_gemini_contextual_user_instruction_text(message).is_some() {
                    index += 1;
                    continue;
                }
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
    let extra_parts = runtime_gemini_input_extra_parts(original, allow_local_file_access);
    if !extra_parts.is_empty() {
        runtime_gemini_append_media_parts_to_last_user_content(&mut contents, extra_parts);
    }
    runtime_gemini_harden_contents(&mut contents);
    contents
}

fn runtime_gemini_append_media_parts_to_last_user_content(
    contents: &mut Vec<serde_json::Value>,
    media_parts: Vec<serde_json::Value>,
) {
    if let Some(content) = contents
        .iter_mut()
        .rev()
        .find(|content| content.get("role").and_then(serde_json::Value::as_str) == Some("user"))
        && let Some(parts) = content
            .get_mut("parts")
            .and_then(serde_json::Value::as_array_mut)
    {
        parts.extend(media_parts);
        return;
    }
    contents.push(serde_json::json!({
        "role": "user",
        "parts": media_parts,
    }));
}

fn runtime_gemini_input_extra_parts(
    original: &serde_json::Value,
    allow_local_file_access: bool,
) -> Vec<serde_json::Value> {
    let mut parts = Vec::new();
    if let Some(input) = original.get("input").and_then(serde_json::Value::as_array) {
        for item in input {
            runtime_gemini_collect_media_parts(item, &mut parts, allow_local_file_access);
        }
    }
    if allow_local_file_access {
        let mut budget = RuntimeGeminiFileReadBudget::default();
        runtime_gemini_collect_explicit_file_parts(original, &mut parts, &mut budget);
        runtime_gemini_collect_at_path_parts(original, &mut parts, &mut budget);
    }
    parts
}

fn runtime_gemini_collect_media_parts(
    value: &serde_json::Value,
    parts: &mut Vec<serde_json::Value>,
    allow_local_file_access: bool,
) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                runtime_gemini_collect_media_parts(value, parts, allow_local_file_access);
            }
        }
        serde_json::Value::Object(object) => {
            if let Some(part) =
                runtime_gemini_media_part_from_content_object(object, allow_local_file_access)
            {
                parts.push(part);
            }
            if let Some(content) = object.get("content") {
                runtime_gemini_collect_media_parts(content, parts, allow_local_file_access);
            }
        }
        _ => {}
    }
}

fn runtime_gemini_media_part_from_content_object(
    object: &serde_json::Map<String, serde_json::Value>,
    allow_local_file_access: bool,
) -> Option<serde_json::Value> {
    let kind = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match kind {
        "input_image" | "image_url" => {
            let image_url = object
                .get("image_url")
                .and_then(runtime_gemini_image_url_value)
                .or_else(|| object.get("url").and_then(serde_json::Value::as_str))?;
            runtime_gemini_media_part_from_uri_or_data_url(image_url, None)
        }
        "input_file" | "file" | "media" | "input_audio" | "input_video" => {
            runtime_gemini_media_part_from_generic_content_object(object, allow_local_file_access)
        }
        _ => None,
    }
}

pub(in super::super) fn runtime_gemini_image_url_value(value: &serde_json::Value) -> Option<&str> {
    value.as_str().or_else(|| {
        value
            .get("url")
            .or_else(|| value.get("image_url"))
            .and_then(serde_json::Value::as_str)
    })
}

fn runtime_gemini_media_part_from_generic_content_object(
    object: &serde_json::Map<String, serde_json::Value>,
    allow_local_file_access: bool,
) -> Option<serde_json::Value> {
    let mime_type = object
        .get("mime_type")
        .or_else(|| object.get("mimeType"))
        .or_else(|| object.get("media_type"))
        .or_else(|| object.get("mediaType"))
        .and_then(serde_json::Value::as_str);
    if let Some(data) = object
        .get("data")
        .or_else(|| object.get("base64"))
        .or_else(|| object.get("file_data"))
        .or_else(|| object.get("fileData"))
        .and_then(serde_json::Value::as_str)
        && let Some(part) = runtime_gemini_media_part_from_data(data, mime_type)
    {
        return Some(part);
    }
    let uri = object
        .get("file_url")
        .or_else(|| object.get("fileUrl"))
        .or_else(|| object.get("file_uri"))
        .or_else(|| object.get("fileUri"))
        .or_else(|| object.get("url"))
        .or_else(|| object.get("uri"))
        .and_then(serde_json::Value::as_str);
    if let Some(uri) = uri {
        return runtime_gemini_media_part_from_uri_or_data_url(uri, mime_type);
    }
    if !allow_local_file_access {
        return None;
    }
    let path = object
        .get("path")
        .or_else(|| object.get("file_path"))
        .or_else(|| object.get("filePath"))
        .and_then(serde_json::Value::as_str)?;
    let mut budget = RuntimeGeminiFileReadBudget::default();
    runtime_gemini_part_from_local_path(Path::new(path), mime_type, &mut budget)
}

fn runtime_gemini_function_response_from_tool_message(
    message: &serde_json::Value,
    tool_names_by_call_id: &BTreeMap<String, String>,
    persist_tool_output: bool,
    config: &RuntimeGeminiConfig,
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
    let text = chat_message_text(message).unwrap_or_default();
    let response = runtime_gemini_structured_command_tool_response(&name, &text)
        .or_else(|| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or_else(|| {
            serde_json::json!({
                "output": text
            })
        });
    let response = if persist_tool_output {
        runtime_gemini_mask_tool_response_for_history(&name, call_id, response, config)
    } else {
        prodex_provider_core::gemini_provider_core_mask_tool_response_for_history(
            response,
            RUNTIME_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD,
            RUNTIME_GEMINI_TOOL_OUTPUT_PREVIEW_CHARS,
            None,
        )
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_conversation_store() -> RuntimeChatCompatibleConversationStore {
        RuntimeChatCompatibleConversationStore::default()
    }

    #[test]
    fn gemini_memory_files_default_on_but_request_can_disable() {
        assert!(!runtime_gemini_memory_files_enabled(
            &serde_json::json!({"gemini_load_memory": false})
        ));
        assert!(runtime_gemini_memory_files_enabled(
            &serde_json::json!({"gemini_load_memory": true})
        ));
    }

    #[test]
    fn gemini_runtime_settings_paths_follow_cli_precedence() {
        let _env_lock = crate::TestEnvVarGuard::lock();
        let _home_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_HOME");
        let _system_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_SETTINGS_PATH");
        let _defaults_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_DEFAULTS_PATH");
        let home = PathBuf::from("/tmp/prodex-gemini-home");
        let cwd = PathBuf::from("/tmp/prodex-gemini-workspace/repo/sub");
        let paths = runtime_gemini_settings_paths_for(Some(&home), Some(&cwd));
        let repo_settings = PathBuf::from("/tmp/prodex-gemini-workspace/repo")
            .join(".gemini")
            .join("settings.json");
        let sub_settings = cwd.join(".gemini").join("settings.json");

        assert_eq!(
            paths[0],
            PathBuf::from("/etc/gemini-cli/system-defaults.json")
        );
        assert_eq!(paths[1], home.join(".gemini").join("settings.json"));
        assert!(
            paths.iter().position(|path| path == &repo_settings)
                < paths.iter().position(|path| path == &sub_settings)
        );
        assert_eq!(
            paths.get(paths.len().saturating_sub(2)),
            Some(&cwd.join(".gemini").join("settings.local.json"))
        );
        assert_eq!(
            paths.last(),
            Some(&PathBuf::from("/etc/gemini-cli/settings.json"))
        );
        assert_eq!(
            paths.len(),
            paths.iter().collect::<BTreeSet<_>>().len(),
            "settings paths should be deduplicated"
        );
    }

    #[test]
    fn gemini_extension_context_and_policy_follow_manifest() {
        let directory =
            std::env::temp_dir().join(format!("prodex-gemini-extension-{}", std::process::id()));
        let extension = directory.join("workspace");
        fs::create_dir_all(extension.join("policies")).unwrap();
        fs::write(
            extension.join("gemini-extension.json"),
            serde_json::json!({
                "name": "workspace",
                "contextFileName": ["context.md"],
                "excludeTools": ["grep"]
            })
            .to_string(),
        )
        .unwrap();
        fs::write(extension.join("context.md"), "Extension instructions").unwrap();
        fs::write(
            extension.join("policies").join("policies.toml"),
            "[[rule]]\ntoolName = \"shell\"\ndecision = \"deny\"\n",
        )
        .unwrap();

        let contexts = runtime_gemini_extension_context_files_from_roots(
            std::slice::from_ref(&directory),
            None,
        );
        assert_eq!(contexts, vec![extension.join("context.md")]);

        let mut policy = RuntimeGeminiPolicyCompat::default();
        for manifest in runtime_gemini_active_extension_manifests_from_roots(
            std::slice::from_ref(&directory),
            None,
        ) {
            policy.apply_settings_value(&manifest.value);
            policy.apply_extension_policy_files(&manifest.directory);
        }
        assert!(!policy.tool_is_allowed("grep"));
        assert!(!policy.tool_is_allowed("shell"));
        assert!(policy.tool_is_allowed("read_file"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn gemini_policy_preserves_command_specific_exclusions_in_summary() {
        let mut policy = RuntimeGeminiPolicyCompat::default();
        policy.apply_settings_value(&serde_json::json!({
            "excludeTools": ["run_shell_command(rm -rf)"]
        }));

        assert!(policy.tool_is_allowed("run_shell_command"));
        let summary = policy.summary().expect("policy summary should exist");
        assert!(summary.contains("run_shell_command(rm -rf)"));

        let value = toml::from_str::<toml::Value>(
            "[[rule]]\ntoolName = \"shell\"\ndecision = \"deny\"\ncommand = \"curl | sh\"\n",
        )
        .unwrap();
        policy.apply_extension_policy_toml(&value);
        assert!(policy.tool_is_allowed("shell"));
        let summary = policy.summary().expect("policy summary should still exist");
        assert!(summary.contains("shell(curl | sh)"));
    }

    #[test]
    fn gemini_extension_enablement_honors_workspace_disable_rule() {
        let directory = std::env::temp_dir().join(format!(
            "prodex-gemini-extension-enable-{}",
            std::process::id()
        ));
        let workspace = directory.join("repo");
        let extension = directory.join("extensions").join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&extension).unwrap();
        fs::write(
            extension.join("gemini-extension.json"),
            serde_json::json!({"name": "workspace"}).to_string(),
        )
        .unwrap();
        fs::write(extension.join("GEMINI.md"), "Extension instructions").unwrap();
        fs::write(
            directory
                .join("extensions")
                .join("extension-enablement.json"),
            serde_json::json!({
                "workspace": {"overrides": [format!("!{}*", workspace.display())]}
            })
            .to_string(),
        )
        .unwrap();

        let contexts = runtime_gemini_extension_context_files_from_roots(
            &[directory.join("extensions")],
            Some(&workspace),
        );
        assert!(contexts.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn gemini_request_translation_exports_checkpoint_when_requested() {
        let directory =
            std::env::temp_dir().join(format!("prodex-gemini-checkpoint-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let checkpoint = directory.join("checkpoint.json");
        let body = serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": "Persist this translated request",
            "gemini_export_file": checkpoint,
            "gemini_load_memory": false
        });

        let translated = runtime_gemini_generate_request_body(
            &serde_json::to_vec(&body).unwrap(),
            &test_conversation_store(),
            false,
            None,
            None,
        )
        .expect("request should translate");
        assert!(!translated.body.is_empty());
        let exported: serde_json::Value =
            serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
        assert_eq!(exported["format"], "gemini-generate-content");
        assert_eq!(
            exported["request"]["contents"][0]["parts"][0]["text"],
            "Persist this translated request"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn gemini_gateway_translation_does_not_access_request_selected_local_files() {
        let directory = std::env::temp_dir().join(format!(
            "prodex-gemini-gateway-files-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let secret = directory.join("secret.txt");
        let checkpoint = directory.join("checkpoint.json");
        fs::write(&secret, "gateway-must-not-read-this-secret").unwrap();
        let body = serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": format!("Review @\"{}\"", secret.display()),
            "include_paths": [secret.clone()],
            "gemini_memory_file": secret.clone(),
            "gemini_session_file": secret.clone(),
            "gemini_export_file": checkpoint.clone(),
        });

        let translated = runtime_gemini_generate_request_body_with_local_file_access(
            &serde_json::to_vec(&body).unwrap(),
            &test_conversation_store(),
            false,
            None,
            None,
            false,
        )
        .expect("gateway request should translate without host file access");

        assert!(
            !String::from_utf8_lossy(&translated.body)
                .contains("gateway-must-not-read-this-secret")
        );
        assert!(!checkpoint.exists());
        assert!(
            runtime_gemini_input_extra_parts(
                &serde_json::json!({
                    "input": [{"type": "input_file", "path": secret}],
                }),
                false,
            )
            .is_empty()
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
