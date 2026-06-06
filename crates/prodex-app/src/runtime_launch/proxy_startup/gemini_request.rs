use super::super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_chat_request_body,
};
use super::RuntimeGeminiTranslatedRequest;
use super::gemini_schema::runtime_gemini_sanitize_function_schema;
use anyhow::{Context, Result};
use base64::Engine;
use prodex_cli::SUPER_GEMINI_DEFAULT_MODEL;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const PRODEX_GEMINI_CODEX_PARITY_INSTRUCTION: &str = "\
You are running inside Codex through the Prodex Gemini bridge. Match Codex tool workflows exactly: \
use available edit/apply_patch tools to change files instead of only describing edits; use shell/process tools to run commands; \
when a command returns a running session id, call the wait/read follow-up tool until the process exits or yields the needed output; \
when explaining file changes, use unified diff format only as a human-readable summary, not as a substitute for applying edits; \
use available web_search, tool_search, and MCP tools when the task calls for them. \
If the user asks you to monitor or fix GitHub Actions, use the local gh CLI with saved credentials when available; \
gh run watch is only a status view, so on failure inspect logs with gh run view <run-id> --log-failed, gh run view <run-id> --json jobs, or gh run view --job <job-id> --log. \
If a publish workflow fails while waiting for CI success, read that step log, follow the target CI run URL/SHA, inspect the failed CI job logs, and reproduce the exact failing local command before declaring blocked.";
const RUNTIME_GEMINI_CONTEXT_FILE_LIMIT: usize = 16;
const RUNTIME_GEMINI_CONTEXT_BYTE_LIMIT: usize = 128 * 1024;
const RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT: usize = 2048;
const RUNTIME_GEMINI_MEMORY_BYTE_LIMIT: usize = 64 * 1024;
const RUNTIME_GEMINI_IMPORT_BYTE_LIMIT: usize = 256 * 1024;
const RUNTIME_GEMINI_EXTENSION_SCAN_LIMIT: usize = 64;
const RUNTIME_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD: usize = 50_000;
const RUNTIME_GEMINI_TOOL_OUTPUT_PREVIEW_CHARS: usize = 1_000;
const RUNTIME_GEMINI_DEFAULT_CONTEXT_EXCLUDES: &[&str] = &[
    "**/node_modules/**",
    "**/.git/**",
    "**/bower_components/**",
    "**/.svn/**",
    "**/.hg/**",
    "**/.vscode/**",
    "**/.idea/**",
    "**/dist/**",
    "**/build/**",
    "**/coverage/**",
    "**/__pycache__/**",
    "**/*.bin",
    "**/*.exe",
    "**/*.dll",
    "**/*.so",
    "**/*.dylib",
    "**/*.class",
    "**/*.jar",
    "**/*.war",
    "**/*.zip",
    "**/*.tar",
    "**/*.gz",
    "**/*.bz2",
    "**/*.rar",
    "**/*.7z",
    "**/*.pak",
    "**/*.rpa",
    "**/*.doc",
    "**/*.docx",
    "**/*.xls",
    "**/*.xlsx",
    "**/*.ppt",
    "**/*.pptx",
    "**/*.odt",
    "**/*.ods",
    "**/*.odp",
    "**/*.pyc",
    "**/*.pyo",
    "**/.DS_Store",
    "**/.env",
    "**/GEMINI.md",
];

pub(in super::super) fn runtime_gemini_generate_request_body(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
    code_assist: bool,
    project_id: Option<&str>,
    thinking_budget_tokens: Option<u64>,
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
    if let Some(system_instruction) = runtime_gemini_system_instruction(&chat_value, &original) {
        request.insert("systemInstruction".to_string(), system_instruction);
    }
    request.insert(
        "contents".to_string(),
        serde_json::Value::Array(runtime_gemini_contents_from_chat(&chat_value, &original)),
    );
    if let Some(tools) = runtime_gemini_tools_from_requests(&original, &chat_value, &model) {
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
    runtime_gemini_export_checkpoint(&original, &request)
        .context("failed to export Gemini session checkpoint")?;

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

fn runtime_gemini_request_object_mut(
    value: &mut serde_json::Value,
) -> Option<&mut serde_json::Map<String, serde_json::Value>> {
    if value.get("request").is_some() {
        value.get_mut("request")?.as_object_mut()
    } else {
        value.as_object_mut()
    }
}

fn runtime_gemini_export_checkpoint(
    original: &serde_json::Value,
    request: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let Some(path) = runtime_gemini_export_checkpoint_path(original) else {
        return Ok(());
    };
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let checkpoint = serde_json::json!({
        "version": 1,
        "format": "gemini-generate-content",
        "source": "prodex-gemini-bridge",
        "request": serde_json::Value::Object(request.clone()),
    });
    let body = serde_json::to_vec_pretty(&checkpoint)
        .context("failed to serialize Gemini session checkpoint")?;
    fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn runtime_gemini_export_checkpoint_path(original: &serde_json::Value) -> Option<PathBuf> {
    for key in [
        "gemini_export_file",
        "geminiExportFile",
        "gemini_checkpoint_export_file",
        "geminiCheckpointExportFile",
        "gemini_session_export_file",
        "geminiSessionExportFile",
    ] {
        if let Some(path) = original
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty())
        {
            return Some(PathBuf::from(path));
        }
    }
    for key in [
        "PRODEX_GEMINI_EXPORT_FILE",
        "PRODEX_GEMINI_CHECKPOINT_EXPORT_FILE",
    ] {
        if let Some(path) = env::var_os(key)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
        {
            return Some(path);
        }
    }
    None
}

fn runtime_gemini_contents_from_chat(
    chat: &serde_json::Value,
    original: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let mut contents = runtime_gemini_imported_session_contents(original);
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
    let extra_parts = runtime_gemini_input_extra_parts(original);
    if !extra_parts.is_empty() {
        runtime_gemini_append_media_parts_to_last_user_content(&mut contents, extra_parts);
    }
    contents
}

fn runtime_gemini_imported_session_contents(
    original: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let mut contents = Vec::new();
    for value in runtime_gemini_import_values(original) {
        contents.extend(runtime_gemini_import_contents_from_value(&value));
    }
    contents
}

fn runtime_gemini_import_values(original: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    for key in [
        "gemini_session",
        "geminiSession",
        "gemini_checkpoint",
        "geminiCheckpoint",
        "gemini_import",
        "geminiImport",
    ] {
        if let Some(value) = original.get(key).filter(|value| !value.is_null()) {
            values.push(value.clone());
        }
    }
    let mut paths = Vec::new();
    for key in [
        "gemini_session_file",
        "geminiSessionFile",
        "gemini_checkpoint_file",
        "geminiCheckpointFile",
        "gemini_import_file",
        "geminiImportFile",
    ] {
        runtime_gemini_collect_path_values(original.get(key), &mut paths);
    }
    for key in [
        "PRODEX_GEMINI_SESSION_FILE",
        "PRODEX_GEMINI_CHECKPOINT_FILE",
        "PRODEX_GEMINI_IMPORT_FILE",
    ] {
        if let Some(value) = env::var_os(key) {
            paths.extend(env::split_paths(&value));
        }
    }
    for path in paths {
        if let Some(text) =
            runtime_gemini_read_text_limited(&path, RUNTIME_GEMINI_IMPORT_BYTE_LIMIT)
        {
            values.push(
                serde_json::from_str::<serde_json::Value>(&text)
                    .unwrap_or(serde_json::Value::String(text)),
            );
        }
    }
    values
}

fn runtime_gemini_import_contents_from_value(value: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(array) = value.get("contents").and_then(serde_json::Value::as_array) {
        return array
            .iter()
            .filter_map(runtime_gemini_import_content_from_value)
            .collect();
    }
    if let Some(array) = value.get("history").and_then(serde_json::Value::as_array) {
        return array
            .iter()
            .filter_map(runtime_gemini_import_content_from_value)
            .collect();
    }
    if let Some(array) = value.as_array() {
        return array
            .iter()
            .flat_map(runtime_gemini_import_contents_from_value)
            .collect();
    }
    if let Some(content) = runtime_gemini_import_content_from_value(value) {
        return vec![content];
    }
    if let Some(text) = value.as_str() {
        return runtime_gemini_import_contents_from_text(text);
    }
    Vec::new()
}

fn runtime_gemini_import_content_from_value(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    if value
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .is_some()
    {
        let role =
            runtime_gemini_import_role(value.get("role").and_then(serde_json::Value::as_str));
        let parts = value.get("parts")?.clone();
        return Some(serde_json::json!({
            "role": role,
            "parts": parts,
        }));
    }
    let role = runtime_gemini_import_role(
        value
            .get("role")
            .or_else(|| value.get("author"))
            .or_else(|| value.get("type"))
            .and_then(serde_json::Value::as_str),
    );
    runtime_gemini_import_text_value(value).map(|text| {
        serde_json::json!({
            "role": role,
            "parts": [{ "text": text }],
        })
    })
}

fn runtime_gemini_import_contents_from_text(text: &str) -> Vec<serde_json::Value> {
    let mut contents = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
            && let Some(content) = runtime_gemini_import_content_from_value(&value)
        {
            contents.push(content);
        }
    }
    if !contents.is_empty() {
        return contents;
    }
    let text = runtime_gemini_truncate_to_bytes(text.trim(), RUNTIME_GEMINI_IMPORT_BYTE_LIMIT);
    if text.is_empty() {
        Vec::new()
    } else {
        vec![serde_json::json!({
            "role": "user",
            "parts": [{ "text": format!("Imported Gemini session/checkpoint context:\n{text}") }],
        })]
    }
}

fn runtime_gemini_import_text_value(value: &serde_json::Value) -> Option<String> {
    for key in ["content", "text", "message", "prompt"] {
        if let Some(text) = value.get(key).and_then(serde_json::Value::as_str) {
            return Some(text.to_string());
        }
    }
    value
        .get("content")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(serde_json::Value::as_str)
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.is_empty())
}

fn runtime_gemini_import_role(role: Option<&str>) -> &'static str {
    match role.unwrap_or_default() {
        "assistant" | "model" => "model",
        _ => "user",
    }
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

fn runtime_gemini_input_extra_parts(original: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut parts = Vec::new();
    if let Some(input) = original.get("input").and_then(serde_json::Value::as_array) {
        for item in input {
            runtime_gemini_collect_media_parts(item, &mut parts);
        }
    }
    let mut budget = RuntimeGeminiFileReadBudget::default();
    runtime_gemini_collect_explicit_file_parts(original, &mut parts, &mut budget);
    runtime_gemini_collect_at_path_parts(original, &mut parts, &mut budget);
    parts
}

fn runtime_gemini_collect_media_parts(
    value: &serde_json::Value,
    parts: &mut Vec<serde_json::Value>,
) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                runtime_gemini_collect_media_parts(value, parts);
            }
        }
        serde_json::Value::Object(object) => {
            if let Some(part) = runtime_gemini_media_part_from_content_object(object) {
                parts.push(part);
            }
            if let Some(content) = object.get("content") {
                runtime_gemini_collect_media_parts(content, parts);
            }
        }
        _ => {}
    }
}

fn runtime_gemini_media_part_from_content_object(
    object: &serde_json::Map<String, serde_json::Value>,
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
            runtime_gemini_media_part_from_generic_content_object(object)
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
    let path = object
        .get("path")
        .or_else(|| object.get("file_path"))
        .or_else(|| object.get("filePath"))
        .and_then(serde_json::Value::as_str)?;
    let mut budget = RuntimeGeminiFileReadBudget::default();
    runtime_gemini_part_from_local_path(Path::new(path), mime_type, &mut budget)
}

fn runtime_gemini_media_part_from_data(
    data: &str,
    mime_type: Option<&str>,
) -> Option<serde_json::Value> {
    let data = data.trim();
    if data.is_empty() {
        return None;
    }
    if let Some((data_url_mime_type, data)) = runtime_gemini_data_url_parts(data) {
        return Some(serde_json::json!({
            "inlineData": {
                "mimeType": data_url_mime_type,
                "data": data,
            }
        }));
    }
    Some(serde_json::json!({
        "inlineData": {
            "mimeType": mime_type.unwrap_or("application/octet-stream"),
            "data": data,
        }
    }))
}

#[derive(Default)]
struct RuntimeGeminiFileReadBudget {
    files: usize,
    bytes: usize,
    paths: BTreeSet<PathBuf>,
}

#[derive(Default)]
struct RuntimeGeminiContextFilter {
    project_root: Option<PathBuf>,
    use_default_excludes: bool,
    project_rules: Vec<RuntimeGeminiIgnoreRule>,
}

struct RuntimeGeminiIgnoreRule {
    base_dir: PathBuf,
    pattern: String,
    negated: bool,
    directory_only: bool,
}

fn runtime_gemini_collect_explicit_file_parts(
    original: &serde_json::Value,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    for key in [
        "include_paths",
        "includePaths",
        "read_many_files",
        "readManyFiles",
        "gemini_context_files",
        "geminiContextFiles",
    ] {
        if let Some(value) = original.get(key) {
            runtime_gemini_collect_file_reference_value(value, parts, budget);
        }
    }
}

fn runtime_gemini_collect_file_reference_value(
    value: &serde_json::Value,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    match value {
        serde_json::Value::String(path) => {
            if let Some(part) = runtime_gemini_part_from_local_path(Path::new(path), None, budget) {
                parts.push(part);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_gemini_collect_file_reference_value(item, parts, budget);
            }
        }
        serde_json::Value::Object(object) => {
            if object.contains_key("include") {
                runtime_gemini_collect_read_many_files_object(object, parts, budget);
                return;
            }
            if let Some(path) = object
                .get("path")
                .or_else(|| object.get("file_path"))
                .or_else(|| object.get("filePath"))
                .and_then(serde_json::Value::as_str)
            {
                let mime_type = object
                    .get("mime_type")
                    .or_else(|| object.get("mimeType"))
                    .and_then(serde_json::Value::as_str);
                if let Some(part) =
                    runtime_gemini_part_from_local_path(Path::new(path), mime_type, budget)
                {
                    parts.push(part);
                }
            }
        }
        _ => {}
    }
}

fn runtime_gemini_collect_read_many_files_object(
    object: &serde_json::Map<String, serde_json::Value>,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    let mut includes = Vec::new();
    runtime_gemini_collect_string_values(object.get("include"), &mut includes);
    let mut excludes = Vec::new();
    runtime_gemini_collect_string_values(object.get("exclude"), &mut excludes);
    let filter = RuntimeGeminiContextFilter::from_read_many_object(object);
    for include in includes {
        if budget.files >= RUNTIME_GEMINI_CONTEXT_FILE_LIMIT {
            break;
        }
        if runtime_gemini_path_has_glob(&include) {
            runtime_gemini_collect_glob_file_parts(&include, &excludes, &filter, parts, budget);
        } else if !filter.is_excluded(Path::new(&include), &excludes)
            && let Some(part) =
                runtime_gemini_part_from_local_path(Path::new(&include), None, budget)
        {
            parts.push(part);
        }
    }
}

fn runtime_gemini_collect_string_values(
    value: Option<&serde_json::Value>,
    values: &mut Vec<String>,
) {
    match value {
        Some(serde_json::Value::String(value)) => values.push(value.to_string()),
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                runtime_gemini_collect_string_values(Some(item), values);
            }
        }
        _ => {}
    }
}

fn runtime_gemini_collect_glob_file_parts(
    pattern: &str,
    excludes: &[String],
    filter: &RuntimeGeminiContextFilter,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    let root = runtime_gemini_glob_root(pattern);
    let Some(root) = runtime_gemini_resolve_local_path(&root) else {
        return;
    };
    let mut candidates = Vec::new();
    let mut scanned = 0;
    runtime_gemini_collect_context_candidates(
        &root,
        filter.use_default_excludes,
        &mut candidates,
        &mut scanned,
    );
    candidates.sort();
    for candidate in candidates {
        if budget.files >= RUNTIME_GEMINI_CONTEXT_FILE_LIMIT {
            break;
        }
        let candidate_match_path = runtime_gemini_context_match_path(&candidate, pattern);
        if !runtime_gemini_glob_matches(pattern, &candidate_match_path)
            || filter.is_excluded(&candidate, excludes)
        {
            continue;
        }
        if let Some(part) = runtime_gemini_part_from_local_path(&candidate, None, budget) {
            parts.push(part);
        }
    }
}

fn runtime_gemini_collect_context_candidates(
    path: &Path,
    use_default_excludes: bool,
    candidates: &mut Vec<PathBuf>,
    scanned: &mut usize,
) {
    if *scanned >= RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut entries = entries.filter_map(|entry| entry.ok()).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if *scanned >= RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT {
            break;
        }
        *scanned = scanned.saturating_add(1);
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if use_default_excludes && runtime_gemini_skip_context_path_name(name) {
            continue;
        }
        if path.is_dir() {
            runtime_gemini_collect_context_candidates(
                &path,
                use_default_excludes,
                candidates,
                scanned,
            );
        } else if path.is_file() {
            candidates.push(path);
        }
    }
}

fn runtime_gemini_glob_root(pattern: &str) -> PathBuf {
    let normalized = pattern.replace('\\', "/");
    let mut root = PathBuf::new();
    for component in normalized.split('/') {
        if runtime_gemini_path_has_glob(component) {
            break;
        }
        if component.is_empty() {
            if root.as_os_str().is_empty() {
                root.push(Path::new("/"));
            }
            continue;
        }
        root.push(component);
    }
    if root.as_os_str().is_empty() {
        PathBuf::from(".")
    } else if root.extension().is_some() {
        root.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        root
    }
}

fn runtime_gemini_context_match_path(path: &Path, pattern: &str) -> String {
    let match_path = if Path::new(pattern).is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
            .unwrap_or_else(|| path.to_path_buf())
    };
    match_path.to_string_lossy().replace('\\', "/")
}

impl RuntimeGeminiContextFilter {
    fn project_defaults() -> Self {
        let Some(project_root) = std::env::current_dir().ok() else {
            return Self {
                project_root: None,
                use_default_excludes: true,
                project_rules: Vec::new(),
            };
        };
        let mut project_rules = Vec::new();
        runtime_gemini_load_ignore_rules(
            &project_root.join(".gitignore"),
            &project_root,
            &project_root,
            &mut project_rules,
        );
        runtime_gemini_load_nested_gitignore_rules(
            &project_root,
            &project_root,
            true,
            &mut project_rules,
            &mut 0,
        );
        runtime_gemini_load_ignore_rules(
            &project_root.join(".geminiignore"),
            &project_root,
            &project_root,
            &mut project_rules,
        );
        Self {
            project_root: Some(project_root),
            use_default_excludes: true,
            project_rules,
        }
    }

    fn from_read_many_object(object: &serde_json::Map<String, serde_json::Value>) -> Self {
        let use_default_excludes = object
            .get("useDefaultExcludes")
            .or_else(|| object.get("use_default_excludes"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let filtering = object
            .get("file_filtering_options")
            .or_else(|| object.get("fileFilteringOptions"))
            .and_then(serde_json::Value::as_object);
        let respect_git_ignore = filtering
            .and_then(|options| {
                options
                    .get("respect_git_ignore")
                    .or_else(|| options.get("respectGitIgnore"))
            })
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let respect_gemini_ignore = filtering
            .and_then(|options| {
                options
                    .get("respect_gemini_ignore")
                    .or_else(|| options.get("respectGeminiIgnore"))
            })
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let Some(project_root) = std::env::current_dir().ok() else {
            return Self {
                project_root: None,
                use_default_excludes,
                project_rules: Vec::new(),
            };
        };
        let mut project_rules = Vec::new();
        if respect_git_ignore {
            runtime_gemini_load_ignore_rules(
                &project_root.join(".gitignore"),
                &project_root,
                &project_root,
                &mut project_rules,
            );
            runtime_gemini_load_nested_gitignore_rules(
                &project_root,
                &project_root,
                use_default_excludes,
                &mut project_rules,
                &mut 0,
            );
        }
        if respect_gemini_ignore {
            runtime_gemini_load_ignore_rules(
                &project_root.join(".geminiignore"),
                &project_root,
                &project_root,
                &mut project_rules,
            );
        }
        if let Some(filtering) = filtering {
            let mut custom_paths = Vec::new();
            runtime_gemini_collect_string_values(
                filtering
                    .get("custom_ignore_file_paths")
                    .or_else(|| filtering.get("customIgnoreFilePaths")),
                &mut custom_paths,
            );
            for path in custom_paths {
                let path = PathBuf::from(path);
                let path = if path.is_absolute() {
                    path
                } else {
                    project_root.join(path)
                };
                let base_dir = path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| project_root.clone());
                runtime_gemini_load_ignore_rules(
                    &path,
                    &base_dir,
                    &project_root,
                    &mut project_rules,
                );
            }
        }
        Self {
            project_root: Some(project_root),
            use_default_excludes,
            project_rules,
        }
    }

    fn is_excluded(&self, path: &Path, excludes: &[String]) -> bool {
        let match_path = self.match_path(path, "");
        if excludes.iter().any(|exclude| {
            let explicit_match_path = self.match_path(path, exclude);
            runtime_gemini_ignore_pattern_matches(exclude, &explicit_match_path, false)
        }) {
            return true;
        }
        if self.use_default_excludes
            && RUNTIME_GEMINI_DEFAULT_CONTEXT_EXCLUDES
                .iter()
                .any(|exclude| runtime_gemini_ignore_pattern_matches(exclude, &match_path, false))
        {
            return true;
        }
        let mut ignored = false;
        for rule in &self.project_rules {
            if runtime_gemini_ignore_rule_matches(rule, &match_path) {
                ignored = !rule.negated;
            }
        }
        ignored
    }

    fn match_path(&self, path: &Path, pattern: &str) -> String {
        if Path::new(pattern).is_absolute() {
            return path.to_string_lossy().replace('\\', "/");
        }
        if let Some(project_root) = self.project_root.as_deref() {
            return path
                .strip_prefix(project_root)
                .ok()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| path.to_path_buf())
                .to_string_lossy()
                .replace('\\', "/");
        }
        runtime_gemini_context_match_path(path, pattern)
    }
}

fn runtime_gemini_load_nested_gitignore_rules(
    directory: &Path,
    project_root: &Path,
    use_default_excludes: bool,
    rules: &mut Vec<RuntimeGeminiIgnoreRule>,
    scanned: &mut usize,
) {
    if *scanned >= RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut entries = entries.filter_map(|entry| entry.ok()).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if *scanned >= RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT {
            return;
        }
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !path.is_dir() || name == ".git" {
            continue;
        }
        if use_default_excludes && runtime_gemini_skip_context_path_name(name) {
            continue;
        }
        *scanned = scanned.saturating_add(1);
        runtime_gemini_load_ignore_rules(&path.join(".gitignore"), &path, project_root, rules);
        runtime_gemini_load_nested_gitignore_rules(
            &path,
            project_root,
            use_default_excludes,
            rules,
            scanned,
        );
    }
}

fn runtime_gemini_load_ignore_rules(
    path: &Path,
    base_dir: &Path,
    project_root: &Path,
    rules: &mut Vec<RuntimeGeminiIgnoreRule>,
) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    let base_dir = base_dir
        .strip_prefix(project_root)
        .unwrap_or(base_dir)
        .to_path_buf();
    for raw in content.lines() {
        let raw = raw.trim_start();
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let (negated, pattern) = raw
            .strip_prefix('!')
            .map(|pattern| (true, pattern))
            .unwrap_or((false, raw));
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }
        rules.push(RuntimeGeminiIgnoreRule {
            base_dir: base_dir.clone(),
            pattern: pattern.trim_end_matches('/').replace('\\', "/"),
            negated,
            directory_only: pattern.ends_with('/'),
        });
    }
}

fn runtime_gemini_ignore_rule_matches(rule: &RuntimeGeminiIgnoreRule, path: &str) -> bool {
    let base_dir = rule.base_dir.to_string_lossy().replace('\\', "/");
    let base_dir = base_dir
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string();
    let path = path
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    let scoped_path = if base_dir.is_empty() {
        path.as_str()
    } else if path == base_dir {
        ""
    } else if let Some(suffix) = path.strip_prefix(&format!("{base_dir}/")) {
        suffix
    } else {
        return false;
    };
    runtime_gemini_ignore_pattern_matches(&rule.pattern, scoped_path, rule.directory_only)
}

fn runtime_gemini_ignore_pattern_matches(pattern: &str, path: &str, directory_only: bool) -> bool {
    let pattern = pattern
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    let path = path
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    if pattern.is_empty() || path.is_empty() {
        return false;
    }
    if pattern.contains('/') {
        if runtime_gemini_glob_matches(&pattern, &path) {
            return true;
        }
        return directory_only
            && path
                .strip_prefix(&pattern)
                .is_some_and(|suffix| suffix.starts_with('/'));
    }
    let components = path.split('/').collect::<Vec<_>>();
    let component_limit = if directory_only {
        components.len().saturating_sub(1)
    } else {
        components.len()
    };
    components[..component_limit].iter().any(|component| {
        runtime_gemini_glob_segment_matches(pattern.as_bytes(), component.as_bytes())
    })
}

fn runtime_gemini_path_has_glob(path: &str) -> bool {
    path.contains('*') || path.contains('?')
}

fn runtime_gemini_glob_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    let pattern = pattern.trim_start_matches("./");
    let path = path.trim_start_matches("./");
    runtime_gemini_glob_component_matches(
        &pattern.split('/').collect::<Vec<_>>(),
        &path.split('/').collect::<Vec<_>>(),
    )
}

fn runtime_gemini_glob_component_matches(pattern: &[&str], path: &[&str]) -> bool {
    let Some((head, tail)) = pattern.split_first() else {
        return path.is_empty();
    };
    if *head == "**" {
        return runtime_gemini_glob_component_matches(tail, path)
            || path.split_first().is_some_and(|(_, path_tail)| {
                runtime_gemini_glob_component_matches(pattern, path_tail)
            });
    }
    path.split_first().is_some_and(|(path_head, path_tail)| {
        runtime_gemini_glob_segment_matches(head.as_bytes(), path_head.as_bytes())
            && runtime_gemini_glob_component_matches(tail, path_tail)
    })
}

fn runtime_gemini_glob_segment_matches(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((&b'*', tail)) => {
            runtime_gemini_glob_segment_matches(tail, text)
                || text.split_first().is_some_and(|(_, text_tail)| {
                    runtime_gemini_glob_segment_matches(pattern, text_tail)
                })
        }
        Some((&b'?', tail)) => text
            .split_first()
            .is_some_and(|(_, text_tail)| runtime_gemini_glob_segment_matches(tail, text_tail)),
        Some((&literal, tail)) => text.split_first().is_some_and(|(&value, text_tail)| {
            literal.eq_ignore_ascii_case(&value)
                && runtime_gemini_glob_segment_matches(tail, text_tail)
        }),
    }
}

fn runtime_gemini_collect_at_path_parts(
    original: &serde_json::Value,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    let mut texts = Vec::new();
    runtime_gemini_collect_input_texts(original.get("input"), &mut texts);
    let filter = RuntimeGeminiContextFilter::project_defaults();
    for text in texts {
        for path in runtime_gemini_at_paths_from_text(&text) {
            if !filter.is_excluded(&path, &[])
                && let Some(part) = runtime_gemini_part_from_local_path(&path, None, budget)
            {
                parts.push(part);
            }
        }
    }
}

fn runtime_gemini_collect_input_texts(value: Option<&serde_json::Value>, texts: &mut Vec<String>) {
    match value {
        Some(serde_json::Value::String(text)) => texts.push(text.clone()),
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                runtime_gemini_collect_input_texts(Some(item), texts);
            }
        }
        Some(serde_json::Value::Object(object)) => {
            if let Some(text) = object
                .get("text")
                .or_else(|| object.get("content"))
                .and_then(serde_json::Value::as_str)
            {
                texts.push(text.to_string());
            }
            runtime_gemini_collect_input_texts(object.get("content"), texts);
        }
        _ => {}
    }
}

fn runtime_gemini_at_paths_from_text(text: &str) -> Vec<PathBuf> {
    let bytes = text.as_bytes();
    let mut paths = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'@' {
            index += 1;
            continue;
        }
        let mut start = index.saturating_add(1);
        let mut end = start;
        if let Some(quote @ (b'"' | b'\'' | b'`')) = bytes.get(start).copied() {
            start = start.saturating_add(1);
            end = start;
            while end < bytes.len() && bytes[end] != quote {
                end += 1;
            }
            index = end.saturating_add(1);
        } else {
            while end < bytes.len()
                && !bytes[end].is_ascii_whitespace()
                && !matches!(
                    bytes[end],
                    b',' | b';' | b':' | b')' | b']' | b'}' | b'<' | b'>'
                )
            {
                end += 1;
            }
            index = end;
        }
        if start < end
            && let Some(token) = text.get(start..end)
            && !token.contains('@')
        {
            let path = PathBuf::from(token);
            if path.exists() {
                paths.push(path);
            }
        }
    }
    paths
}

fn runtime_gemini_part_from_local_path(
    path: &Path,
    mime_type: Option<&str>,
    budget: &mut RuntimeGeminiFileReadBudget,
) -> Option<serde_json::Value> {
    if budget.files >= RUNTIME_GEMINI_CONTEXT_FILE_LIMIT
        || budget.bytes >= RUNTIME_GEMINI_CONTEXT_BYTE_LIMIT
    {
        return None;
    }
    let path = runtime_gemini_resolve_local_path(path)?;
    let metadata = fs::metadata(&path).ok()?;
    if metadata.is_dir() {
        return runtime_gemini_part_from_local_dir(&path, budget);
    }
    if !metadata.is_file() {
        return None;
    }
    let dedup_path = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if budget.paths.contains(&dedup_path) {
        return None;
    }
    let file_len = usize::try_from(metadata.len()).ok()?;
    if file_len == 0 || file_len > RUNTIME_GEMINI_CONTEXT_BYTE_LIMIT.saturating_sub(budget.bytes) {
        return None;
    }
    let data = fs::read(&path).ok()?;
    budget.files = budget.files.saturating_add(1);
    budget.bytes = budget.bytes.saturating_add(data.len());
    budget.paths.insert(dedup_path);
    let mime_type =
        mime_type.unwrap_or_else(|| runtime_gemini_mime_type_for_uri(&path.to_string_lossy()));
    if runtime_gemini_mime_type_is_text(mime_type)
        && let Ok(text) = String::from_utf8(data.clone())
    {
        return Some(serde_json::json!({
            "text": format!("Content from @{}:\n{}", path.display(), text),
        }));
    }
    Some(serde_json::json!({
        "inlineData": {
            "mimeType": mime_type,
            "data": base64::engine::general_purpose::STANDARD.encode(data),
        }
    }))
}

fn runtime_gemini_part_from_local_dir(
    path: &Path,
    budget: &mut RuntimeGeminiFileReadBudget,
) -> Option<serde_json::Value> {
    let mut entries = fs::read_dir(path)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    let mut parts = Vec::new();
    for entry in entries {
        if budget.files >= RUNTIME_GEMINI_CONTEXT_FILE_LIMIT {
            break;
        }
        let name = entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if runtime_gemini_skip_context_path_name(name) {
            continue;
        }
        if entry.is_dir() {
            if let Some(part) = runtime_gemini_part_from_local_dir(&entry, budget) {
                parts.push(part);
            }
        } else if let Some(part) = runtime_gemini_part_from_local_path(&entry, None, budget) {
            parts.push(part);
        }
    }
    (!parts.is_empty()).then(|| {
        serde_json::json!({
            "text": parts
                .iter()
                .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join("\n\n"),
        })
    })
}

fn runtime_gemini_resolve_local_path(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    std::env::current_dir().ok().map(|cwd| cwd.join(path))
}

fn runtime_gemini_mime_type_is_text(mime_type: &str) -> bool {
    mime_type.starts_with("text/")
        || matches!(
            mime_type,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/typescript"
                | "application/x-sh"
                | "application/octet-stream"
        )
}

fn runtime_gemini_skip_context_path_name(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | "__pycache__" | "vendor"
    )
}

pub(in super::super) fn runtime_gemini_media_part_from_uri_or_data_url(
    uri_or_data: &str,
    mime_type: Option<&str>,
) -> Option<serde_json::Value> {
    let uri_or_data = uri_or_data.trim();
    if uri_or_data.is_empty() {
        return None;
    }
    if let Some((data_url_mime_type, data)) = runtime_gemini_data_url_parts(uri_or_data) {
        return Some(serde_json::json!({
            "inlineData": {
                "mimeType": data_url_mime_type,
                "data": data,
            }
        }));
    }
    Some(serde_json::json!({
        "fileData": {
            "fileUri": uri_or_data,
            "mimeType": mime_type.unwrap_or_else(|| runtime_gemini_mime_type_for_uri(uri_or_data)),
        }
    }))
}

pub(in super::super) fn runtime_gemini_data_url_parts(image_url: &str) -> Option<(&str, &str)> {
    let rest = image_url.strip_prefix("data:")?;
    let (metadata, data) = rest.split_once(',')?;
    if !metadata
        .split(';')
        .any(|segment| segment.eq_ignore_ascii_case("base64"))
    {
        return None;
    }
    let mime_type = metadata
        .split(';')
        .next()
        .filter(|mime_type| !mime_type.trim().is_empty())
        .unwrap_or("application/octet-stream");
    Some((mime_type, data))
}

pub(in super::super) fn runtime_gemini_mime_type_for_uri(uri: &str) -> &'static str {
    let uri = uri
        .split(['?', '#'])
        .next()
        .unwrap_or(uri)
        .to_ascii_lowercase();
    if uri.ends_with(".png") {
        "image/png"
    } else if uri.ends_with(".jpg") || uri.ends_with(".jpeg") {
        "image/jpeg"
    } else if uri.ends_with(".webp") {
        "image/webp"
    } else if uri.ends_with(".gif") {
        "image/gif"
    } else if uri.ends_with(".pdf") {
        "application/pdf"
    } else if uri.ends_with(".mp3") || uri.ends_with(".mpeg") {
        "audio/mpeg"
    } else if uri.ends_with(".wav") {
        "audio/wav"
    } else if uri.ends_with(".mp4") {
        "video/mp4"
    } else if uri.ends_with(".mov") {
        "video/quicktime"
    } else {
        "application/octet-stream"
    }
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
    let response = runtime_gemini_mask_tool_response_for_history(&name, call_id, response);
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

fn runtime_gemini_mask_tool_response_for_history(
    tool_name: &str,
    call_id: &str,
    response: serde_json::Value,
) -> serde_json::Value {
    runtime_gemini_mask_tool_response_for_history_with_threshold(
        tool_name,
        call_id,
        response,
        runtime_gemini_tool_output_mask_threshold(),
    )
}

fn runtime_gemini_mask_tool_response_for_history_with_threshold(
    tool_name: &str,
    call_id: &str,
    response: serde_json::Value,
    threshold: usize,
) -> serde_json::Value {
    if threshold == 0 {
        return response;
    }
    let output = runtime_gemini_tool_response_output_string(&response);
    if output.len() <= threshold {
        return response;
    }
    let saved_path = runtime_gemini_save_masked_tool_output(tool_name, call_id, &output);
    let mask = runtime_gemini_masked_tool_output_text(&output, saved_path.as_deref());
    if let serde_json::Value::Object(mut object) = response {
        object.insert("output".to_string(), serde_json::Value::String(mask));
        object.insert("_prodex_masked".to_string(), serde_json::Value::Bool(true));
        return serde_json::Value::Object(object);
    }
    serde_json::json!({
        "output": mask,
        "_prodex_masked": true,
    })
}

fn runtime_gemini_tool_output_mask_threshold() -> usize {
    env::var("PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(RUNTIME_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD)
}

fn runtime_gemini_tool_response_output_string(response: &serde_json::Value) -> String {
    response
        .get("output")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            response
                .get("content")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            serde_json::to_string_pretty(response).unwrap_or_else(|_| response.to_string())
        })
}

fn runtime_gemini_masked_tool_output_text(output: &str, saved_path: Option<&Path>) -> String {
    let path = saved_path
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    format!(
        "[tool_output_masked]\nOriginal Gemini tool output was {} chars / {} lines and was omitted from model history.\nFull output saved to: {}\nPreview:\n{}",
        output.chars().count(),
        output.lines().count(),
        path,
        runtime_gemini_tool_output_preview(output),
    )
}

fn runtime_gemini_tool_output_preview(output: &str) -> String {
    let preview_chars = RUNTIME_GEMINI_TOOL_OUTPUT_PREVIEW_CHARS;
    let char_count = output.chars().count();
    if char_count <= preview_chars {
        return output.to_string();
    }
    let head_len = preview_chars / 2;
    let tail_len = preview_chars.saturating_sub(head_len);
    let head = output.chars().take(head_len).collect::<String>();
    let tail = output
        .chars()
        .rev()
        .take(tail_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{head}\n...\n{tail}")
}

fn runtime_gemini_save_masked_tool_output(
    tool_name: &str,
    call_id: &str,
    output: &str,
) -> Option<PathBuf> {
    let directory = env::var_os("PRODEX_GEMINI_TOOL_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("prodex-gemini-tool-outputs"));
    fs::create_dir_all(&directory).ok()?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    let tool = runtime_gemini_sanitize_file_component(tool_name);
    let call = runtime_gemini_sanitize_file_component(call_id);
    let path = directory.join(format!(
        "{millis}-{}-{}-{}.txt",
        std::process::id(),
        tool,
        call
    ));
    fs::write(&path, output).ok()?;
    Some(path)
}

fn runtime_gemini_sanitize_file_component(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                Some(ch)
            } else if ch.is_whitespace() || ch == '.' || ch == '/' || ch == '\\' {
                Some('_')
            } else {
                None
            }
        })
        .take(64)
        .collect::<String>();
    if sanitized.is_empty() {
        sanitized.push_str("unknown");
    }
    sanitized
}

fn runtime_gemini_hierarchical_memory(original: &serde_json::Value) -> Option<String> {
    let mut sections = Vec::new();
    let mut budget = RUNTIME_GEMINI_MEMORY_BYTE_LIMIT;
    runtime_gemini_collect_inline_memory_sections(original, &mut sections, &mut budget);
    if runtime_gemini_memory_files_enabled(original) {
        runtime_gemini_collect_standard_memory_files(&mut sections, &mut budget);
    }
    runtime_gemini_collect_extension_memory_files(&mut sections, &mut budget);
    (!sections.is_empty()).then(|| sections.join("\n\n"))
}

fn runtime_gemini_collect_inline_memory_sections(
    original: &serde_json::Value,
    sections: &mut Vec<String>,
    budget: &mut usize,
) {
    for key in [
        "gemini_memory",
        "geminiMemory",
        "gemini_hierarchical_memory",
        "geminiHierarchicalMemory",
    ] {
        if let Some(value) = original.get(key).filter(|value| !value.is_null()) {
            runtime_gemini_collect_memory_value("Request", value, sections, budget);
        }
    }
    for key in [
        "gemini_memory_file",
        "geminiMemoryFile",
        "gemini_memory_files",
        "geminiMemoryFiles",
    ] {
        let mut paths = Vec::new();
        runtime_gemini_collect_path_values(original.get(key), &mut paths);
        for path in paths {
            runtime_gemini_push_memory_file_section("Request File", &path, sections, budget);
        }
    }
}

fn runtime_gemini_collect_memory_value(
    title: &str,
    value: &serde_json::Value,
    sections: &mut Vec<String>,
    budget: &mut usize,
) {
    match value {
        serde_json::Value::String(text) => {
            runtime_gemini_push_memory_section(title, text, sections, budget);
        }
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                let section_title = match key.as_str() {
                    "global" | "user" | "userMemory" => "Global",
                    "project" | "projectMemory" | "userProjectMemory" => "Project",
                    "private" | "privateProjectMemory" => "Private Project Memory",
                    "extension" | "extensionMemory" => "Extension",
                    _ => key.as_str(),
                };
                runtime_gemini_collect_memory_value(section_title, value, sections, budget);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_gemini_collect_memory_value(title, item, sections, budget);
            }
        }
        _ => {}
    }
}

fn runtime_gemini_memory_files_enabled(original: &serde_json::Value) -> bool {
    if runtime_gemini_env_bool("PRODEX_GEMINI_DISABLE_MEMORY") == Some(true)
        || runtime_gemini_env_bool("PRODEX_GEMINI_DISABLE_CONTEXT_FILES") == Some(true)
    {
        return false;
    }
    for key in [
        "gemini_load_memory",
        "geminiLoadMemory",
        "gemini_memory_files_enabled",
        "geminiMemoryFilesEnabled",
    ] {
        if let Some(value) = original.get(key) {
            return runtime_gemini_bool_value(value).unwrap_or(false);
        }
    }
    for key in ["PRODEX_GEMINI_LOAD_MEMORY", "PRODEX_GEMINI_MEMORY"] {
        if let Some(enabled) = runtime_gemini_env_bool(key) {
            return enabled;
        }
    }
    true
}

fn runtime_gemini_collect_standard_memory_files(sections: &mut Vec<String>, budget: &mut usize) {
    if let Some(home) = runtime_gemini_home_dir() {
        runtime_gemini_push_memory_file_section(
            "Global",
            &home.join(".gemini").join("GEMINI.md"),
            sections,
            budget,
        );
        runtime_gemini_push_memory_file_section(
            "Global Memory Inbox",
            &home.join(".gemini").join("memory").join("INBOX.md"),
            sections,
            budget,
        );
    }
    if let Ok(cwd) = env::current_dir() {
        let mut ancestors = cwd.ancestors().collect::<Vec<_>>();
        ancestors.reverse();
        for directory in ancestors {
            let title = if directory == cwd {
                "Project".to_string()
            } else {
                format!("Project: {}", directory.display())
            };
            runtime_gemini_push_memory_file_section(
                &title,
                &directory.join("GEMINI.md"),
                sections,
                budget,
            );
        }
        runtime_gemini_push_memory_file_section(
            "Private Project Memory",
            &cwd.join(".gemini").join("memory").join("MEMORY.md"),
            sections,
            budget,
        );
        runtime_gemini_push_memory_file_section(
            "Private Project Memory Inbox",
            &cwd.join(".gemini").join("memory").join("INBOX.md"),
            sections,
            budget,
        );
    }
}

fn runtime_gemini_collect_extension_memory_files(sections: &mut Vec<String>, budget: &mut usize) {
    let mut paths = Vec::new();
    if let Some(configured) = env::var_os("PRODEX_GEMINI_EXTENSION_MEMORY") {
        paths.extend(env::split_paths(&configured));
    }
    paths.extend(runtime_gemini_extension_context_files());
    for path in paths {
        runtime_gemini_push_memory_file_section("Extension", &path, sections, budget);
    }
}

#[derive(Clone)]
struct RuntimeGeminiExtensionManifest {
    directory: PathBuf,
    name: String,
    value: serde_json::Value,
}

fn runtime_gemini_extension_context_files() -> Vec<PathBuf> {
    let roots = runtime_gemini_extension_roots();
    let cwd = env::current_dir().ok();
    runtime_gemini_extension_context_files_from_roots(&roots, cwd.as_deref())
}

fn runtime_gemini_extension_context_files_from_roots(
    roots: &[PathBuf],
    cwd: Option<&Path>,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for extension in runtime_gemini_active_extension_manifests_from_roots(roots, cwd) {
        for name in runtime_gemini_extension_context_file_names(&extension.value) {
            let Some(path) = runtime_gemini_safe_extension_path(&extension.directory, &name) else {
                continue;
            };
            if !path.is_file() {
                continue;
            }
            let key = path.to_string_lossy().to_ascii_lowercase();
            if seen.insert(key) {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

fn runtime_gemini_active_extension_manifests() -> Vec<RuntimeGeminiExtensionManifest> {
    let roots = runtime_gemini_extension_roots();
    let cwd = env::current_dir().ok();
    runtime_gemini_active_extension_manifests_from_roots(&roots, cwd.as_deref())
}

fn runtime_gemini_active_extension_manifests_from_roots(
    roots: &[PathBuf],
    cwd: Option<&Path>,
) -> Vec<RuntimeGeminiExtensionManifest> {
    let mut manifests = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        if manifests.len() >= RUNTIME_GEMINI_EXTENSION_SCAN_LIMIT {
            break;
        }
        if root.join("gemini-extension.json").is_file() {
            if let Some(manifest) = runtime_gemini_load_extension_manifest(root, root.parent(), cwd)
                && seen.insert(manifest.name.to_ascii_lowercase())
            {
                manifests.push(manifest);
            }
            continue;
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            if manifests.len() >= RUNTIME_GEMINI_EXTENSION_SCAN_LIMIT {
                break;
            }
            let directory = entry.path();
            if !directory.is_dir() || !directory.join("gemini-extension.json").is_file() {
                continue;
            }
            if let Some(manifest) =
                runtime_gemini_load_extension_manifest(&directory, Some(root), cwd)
                && seen.insert(manifest.name.to_ascii_lowercase())
            {
                manifests.push(manifest);
            }
        }
    }
    manifests.sort_by(|left, right| left.name.cmp(&right.name));
    manifests
}

fn runtime_gemini_load_extension_manifest(
    directory: &Path,
    root: Option<&Path>,
    cwd: Option<&Path>,
) -> Option<RuntimeGeminiExtensionManifest> {
    let text = runtime_gemini_read_text_limited(
        &directory.join("gemini-extension.json"),
        RUNTIME_GEMINI_MEMORY_BYTE_LIMIT,
    )?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })?;
    let root = root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| directory.parent().unwrap_or(directory).to_path_buf());
    if !runtime_gemini_extension_is_enabled(&name, cwd, &root) {
        return None;
    }
    Some(RuntimeGeminiExtensionManifest {
        directory: directory.to_path_buf(),
        name,
        value,
    })
}

fn runtime_gemini_extension_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(configured) = env::var_os("PRODEX_GEMINI_EXTENSION_DIRS") {
        roots.extend(env::split_paths(&configured));
    }
    if let Some(home) = runtime_gemini_home_dir() {
        roots.push(home.join(".gemini").join("extensions"));
    }
    roots
}

fn runtime_gemini_extension_context_file_names(manifest: &serde_json::Value) -> Vec<String> {
    let Some(context) = manifest.get("contextFileName") else {
        return vec!["GEMINI.md".to_string()];
    };
    let mut names = Vec::new();
    runtime_gemini_collect_string_values(Some(context), &mut names);
    if names.is_empty() {
        names.push("GEMINI.md".to_string());
    }
    names
}

fn runtime_gemini_safe_extension_path(root: &Path, relative: &str) -> Option<PathBuf> {
    let relative = relative.trim();
    if relative.is_empty() {
        return None;
    }
    let path = Path::new(relative);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(root.join(path))
}

fn runtime_gemini_extension_is_enabled(
    name: &str,
    cwd: Option<&Path>,
    extension_root: &Path,
) -> bool {
    if let Some(enabled) = runtime_gemini_extension_name_override(name) {
        return enabled;
    }
    let Some(cwd) = cwd else {
        return true;
    };
    let path = extension_root.join("extension-enablement.json");
    let Some(text) = runtime_gemini_read_text_limited(&path, RUNTIME_GEMINI_MEMORY_BYTE_LIMIT)
    else {
        return true;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return true;
    };
    let Some(overrides) = value
        .get(name)
        .and_then(|extension| extension.get("overrides"))
        .and_then(serde_json::Value::as_array)
    else {
        return true;
    };
    let mut enabled = true;
    for rule in overrides.iter().filter_map(serde_json::Value::as_str) {
        if let Some(disable) = runtime_gemini_extension_override_matches(rule, cwd) {
            enabled = !disable;
        }
    }
    enabled
}

fn runtime_gemini_extension_name_override(name: &str) -> Option<bool> {
    let value = env::var("PRODEX_GEMINI_EXTENSIONS").ok()?;
    let requested = value
        .split([',', ';', ' ', '\n', '\t'])
        .filter_map(|item| {
            let item = item.trim().to_ascii_lowercase();
            (!item.is_empty()).then_some(item)
        })
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return None;
    }
    if requested.len() == 1 && requested[0] == "none" {
        return Some(false);
    }
    Some(
        requested
            .iter()
            .any(|item| item == &name.to_ascii_lowercase()),
    )
}

fn runtime_gemini_extension_override_matches(rule: &str, cwd: &Path) -> Option<bool> {
    let mut rule = rule.trim();
    if rule.is_empty() {
        return None;
    }
    let disable = rule.starts_with('!');
    if disable {
        rule = &rule[1..];
    }
    let include_subdirs = rule.ends_with('*');
    if include_subdirs {
        rule = &rule[..rule.len().saturating_sub(1)];
    }
    let rule = runtime_gemini_normalize_extension_override_path(rule);
    let cwd = runtime_gemini_normalize_extension_override_path(&cwd.to_string_lossy());
    let matches = if include_subdirs {
        cwd.starts_with(&rule)
    } else {
        cwd == rule
    };
    matches.then_some(disable)
}

fn runtime_gemini_normalize_extension_override_path(path: &str) -> String {
    let mut value = path.trim().replace('\\', "/");
    if !value.starts_with('/') {
        value.insert(0, '/');
    }
    if !value.ends_with('/') {
        value.push('/');
    }
    value
}

fn runtime_gemini_push_memory_file_section(
    title: &str,
    path: &Path,
    sections: &mut Vec<String>,
    budget: &mut usize,
) {
    let Some(text) = runtime_gemini_read_text_limited(path, *budget) else {
        return;
    };
    let title = format!("{title}: {}", path.display());
    runtime_gemini_push_memory_section(&title, &text, sections, budget);
}

fn runtime_gemini_push_memory_section(
    title: &str,
    text: &str,
    sections: &mut Vec<String>,
    budget: &mut usize,
) {
    if *budget == 0 {
        return;
    }
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let text = runtime_gemini_truncate_to_bytes(text, *budget);
    *budget = budget.saturating_sub(text.len());
    sections.push(format!("--- {title} ---\n{text}"));
}

fn runtime_gemini_read_text_limited(path: &Path, limit: usize) -> Option<String> {
    if limit == 0 {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let mut reader = file.take(limit as u64);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).to_string())
}

fn runtime_gemini_truncate_to_bytes(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut end = limit.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

fn runtime_gemini_collect_path_values(value: Option<&serde_json::Value>, paths: &mut Vec<PathBuf>) {
    match value {
        Some(serde_json::Value::String(path)) if !path.trim().is_empty() => {
            paths.push(PathBuf::from(path.trim()));
        }
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                runtime_gemini_collect_path_values(Some(item), paths);
            }
        }
        _ => {}
    }
}

fn runtime_gemini_home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

fn runtime_gemini_env_bool(key: &str) -> Option<bool> {
    env::var(key)
        .ok()
        .and_then(|value| runtime_gemini_bool_str(&value))
}

fn runtime_gemini_bool_value(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::Number(number) => Some(number.as_i64().unwrap_or_default() != 0),
        serde_json::Value::String(value) => runtime_gemini_bool_str(value),
        _ => None,
    }
}

fn runtime_gemini_bool_str(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn runtime_gemini_system_instruction(
    chat: &serde_json::Value,
    original: &serde_json::Value,
) -> Option<serde_json::Value> {
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
    if let Some(memory) = runtime_gemini_hierarchical_memory(original) {
        system_text.push_str("\n\n# Gemini CLI Memory Compatibility\n");
        system_text.push_str(&memory);
    }
    if let Some(policy) = RuntimeGeminiPolicyCompat::from_request_and_files(original).summary() {
        system_text.push_str("\n\n# Gemini CLI Policy Compatibility\n");
        system_text.push_str(&policy);
    }

    (!system_text.trim().is_empty())
        .then(|| serde_json::json!({ "parts": [{ "text": system_text }] }))
}

fn runtime_gemini_tools_from_requests(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
) -> Option<serde_json::Value> {
    let mut tools = Vec::new();
    if let Some(computer_use) = runtime_gemini_computer_use_tool(original) {
        tools.push(serde_json::json!({ "computerUse": computer_use }));
    }
    if runtime_gemini_code_execution_enabled(original) {
        tools.push(serde_json::json!({ "codeExecution": {} }));
    }
    if runtime_gemini_web_search_enabled(original) {
        tools.push(serde_json::json!({ "googleSearch": {} }));
    }
    if runtime_gemini_url_context_enabled(original) {
        tools.push(serde_json::json!({ "urlContext": {} }));
    }
    if let Some(serde_json::Value::Array(function_tools)) =
        runtime_gemini_function_tools_from_chat(original, chat, model)
    {
        tools.extend(function_tools);
    }
    (!tools.is_empty()).then_some(serde_json::Value::Array(tools))
}

pub(in super::super) fn runtime_gemini_request_body_without_tool(
    body: &[u8],
    tool_name: &str,
) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let request = runtime_gemini_request_object_mut(&mut value)?;
    let tools = request.get_mut("tools")?.as_array_mut()?;
    let original_len = tools.len();
    tools.retain(|tool| {
        !tool
            .as_object()
            .map(|object| object.contains_key(tool_name))
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

fn runtime_gemini_web_search_enabled(original: &serde_json::Value) -> bool {
    original
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| tools.iter().any(runtime_gemini_is_web_search_tool))
}

fn runtime_gemini_computer_use_tool(original: &serde_json::Value) -> Option<serde_json::Value> {
    let tool = original
        .get("tools")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find(|tool| runtime_gemini_is_computer_use_tool(tool))?;
    let source = tool
        .get("computerUse")
        .or_else(|| tool.get("computer_use"))
        .unwrap_or(tool);
    let environment = source
        .get("environment")
        .and_then(serde_json::Value::as_str)
        .filter(|environment| !environment.trim().is_empty())
        .unwrap_or("ENVIRONMENT_BROWSER");
    let mut computer_use = serde_json::json!({
        "environment": environment,
    });
    if let Some(excluded) = source
        .get("excludedPredefinedFunctions")
        .or_else(|| source.get("excluded_predefined_functions"))
        .filter(|value| !value.is_null())
    {
        computer_use["excludedPredefinedFunctions"] = excluded.clone();
    }
    Some(computer_use)
}

fn runtime_gemini_is_computer_use_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    matches!(
        tool_type,
        "computer" | "computer_use" | "computerUse" | "computer_use_preview"
    ) || tool_type.starts_with("computer_")
        || tool
            .as_object()
            .is_some_and(|object| object.contains_key("computerUse"))
}

fn runtime_gemini_code_execution_enabled(original: &serde_json::Value) -> bool {
    original
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| tools.iter().any(runtime_gemini_is_code_execution_tool))
}

fn runtime_gemini_url_context_enabled(original: &serde_json::Value) -> bool {
    original
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| tools.iter().any(runtime_gemini_is_url_context_tool))
}

fn runtime_gemini_is_code_execution_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    matches!(
        tool_type,
        "code_interpreter" | "code_execution" | "codeExecution"
    ) || tool
        .as_object()
        .is_some_and(|object| object.contains_key("codeExecution"))
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

fn runtime_gemini_is_url_context_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    tool_type == "web_fetch"
        || tool_type == "url_context"
        || tool_type == "urlContext"
        || tool_type == "web_fetch_preview"
        || tool_type.starts_with("web_fetch_preview_")
        || tool
            .as_object()
            .is_some_and(|object| object.contains_key("urlContext"))
}

#[derive(Default)]
struct RuntimeGeminiPolicyCompat {
    allowed_tools: Option<BTreeSet<String>>,
    excluded_tools: BTreeSet<String>,
    command_specific_exclusions: BTreeMap<String, BTreeSet<String>>,
    approval_mode: Option<String>,
}

impl RuntimeGeminiPolicyCompat {
    fn from_request_and_files(original: &serde_json::Value) -> Self {
        let mut policy = Self::default();
        for path in runtime_gemini_settings_paths() {
            if let Some(text) =
                runtime_gemini_read_text_limited(&path, RUNTIME_GEMINI_MEMORY_BYTE_LIMIT)
                && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
            {
                policy.apply_settings_value(&value);
            }
        }
        for extension in runtime_gemini_active_extension_manifests() {
            policy.apply_settings_value(&extension.value);
            policy.apply_extension_policy_files(&extension.directory);
        }
        for key in ["gemini_policy", "geminiPolicy"] {
            if let Some(value) = original.get(key).filter(|value| !value.is_null()) {
                policy.apply_settings_value(value);
            }
        }
        policy
    }

    fn apply_settings_value(&mut self, value: &serde_json::Value) {
        if let Some(mode) = value
            .pointer("/general/defaultApprovalMode")
            .or_else(|| value.pointer("/tools/approvalMode"))
            .or_else(|| value.get("approvalMode"))
            .and_then(serde_json::Value::as_str)
        {
            self.approval_mode = Some(mode.to_ascii_lowercase());
        }
        let mut excluded = Vec::new();
        runtime_gemini_collect_string_values(value.pointer("/tools/exclude"), &mut excluded);
        runtime_gemini_collect_string_values(value.get("excludeTools"), &mut excluded);
        for name in excluded {
            self.add_excluded_tool_or_command(&name);
        }

        let mut allowed = Vec::new();
        runtime_gemini_collect_string_values(value.pointer("/tools/allowed"), &mut allowed);
        runtime_gemini_collect_string_values(value.get("allowedTools"), &mut allowed);
        runtime_gemini_collect_string_values(value.pointer("/tools/core"), &mut allowed);
        runtime_gemini_collect_string_values(value.get("coreTools"), &mut allowed);
        if !allowed.is_empty() {
            let allowed_tools = self.allowed_tools.get_or_insert_with(BTreeSet::new);
            for name in allowed {
                allowed_tools.insert(runtime_gemini_normalize_tool_name(&name));
            }
        }
    }

    fn apply_extension_policy_files(&mut self, directory: &Path) {
        let policies = directory.join("policies");
        let Ok(entries) = fs::read_dir(policies) else {
            return;
        };
        for entry in entries.flatten().take(RUNTIME_GEMINI_EXTENSION_SCAN_LIMIT) {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
                continue;
            }
            let Some(text) =
                runtime_gemini_read_text_limited(&path, RUNTIME_GEMINI_MEMORY_BYTE_LIMIT)
            else {
                continue;
            };
            let Ok(value) = toml::from_str::<toml::Value>(&text) else {
                continue;
            };
            self.apply_extension_policy_toml(&value);
        }
    }

    fn apply_extension_policy_toml(&mut self, value: &toml::Value) {
        let Some(rules) = value.get("rule").and_then(toml::Value::as_array) else {
            return;
        };
        for rule in rules {
            let decision = rule
                .get("decision")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !matches!(decision.as_str(), "deny" | "block" | "blocked") {
                continue;
            }
            if let Some(tool_name) = rule.get("toolName").and_then(toml::Value::as_str) {
                if let Some(pattern) = runtime_gemini_policy_command_pattern_from_rule(rule) {
                    self.command_specific_exclusions
                        .entry(runtime_gemini_normalize_tool_name(tool_name))
                        .or_default()
                        .insert(pattern);
                } else {
                    self.add_excluded_tool_or_command(tool_name);
                }
            }
        }
    }

    fn add_excluded_tool_or_command(&mut self, value: &str) {
        if let Some((tool_name, pattern)) = runtime_gemini_parse_command_specific_tool(value) {
            self.command_specific_exclusions
                .entry(runtime_gemini_normalize_tool_name(&tool_name))
                .or_default()
                .insert(pattern);
            return;
        }
        self.excluded_tools
            .insert(runtime_gemini_normalize_tool_name(value));
    }

    fn filter_function_declarations(&self, declarations: &mut Vec<serde_json::Value>) {
        declarations.retain(|declaration| {
            declaration
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| self.tool_is_allowed(name))
        });
    }

    fn tool_is_allowed(&self, name: &str) -> bool {
        let aliases = runtime_gemini_tool_aliases(name);
        if aliases
            .iter()
            .any(|alias| self.excluded_tools.contains(alias))
        {
            return false;
        }
        if self.approval_mode.as_deref() == Some("plan") && runtime_gemini_tool_is_mutating(name) {
            return false;
        }
        if let Some(allowed) = &self.allowed_tools {
            return aliases.iter().any(|alias| allowed.contains(alias));
        }
        true
    }

    fn summary(&self) -> Option<String> {
        let mut lines = Vec::new();
        if let Some(mode) = &self.approval_mode {
            lines.push(format!("defaultApprovalMode: {mode}"));
        }
        if let Some(allowed) = &self.allowed_tools
            && !allowed.is_empty()
        {
            lines.push(format!(
                "allowed tools: {}",
                allowed.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        if !self.excluded_tools.is_empty() {
            lines.push(format!(
                "excluded tools: {}",
                self.excluded_tools
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.command_specific_exclusions.is_empty() {
            let entries = self
                .command_specific_exclusions
                .iter()
                .map(|(tool, patterns)| {
                    format!(
                        "{}({})",
                        tool,
                        patterns.iter().cloned().collect::<Vec<_>>().join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("excluded tool argument patterns: {entries}"));
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    fn blocked_tool_call_message(&self, name: &str, args: &serde_json::Value) -> Option<String> {
        let aliases = runtime_gemini_tool_aliases(name);
        if aliases
            .iter()
            .any(|alias| self.excluded_tools.contains(alias))
        {
            return Some(format!(
                "Gemini policy blocked tool call `{name}` because the tool is excluded."
            ));
        }
        let command = runtime_gemini_tool_call_command_text(args);
        for alias in aliases {
            let Some(patterns) = self.command_specific_exclusions.get(&alias) else {
                continue;
            };
            for pattern in patterns {
                if command_matches_policy_pattern(&command, pattern) {
                    return Some(format!(
                        "Gemini policy blocked tool call `{name}` because command `{command}` matched blocked pattern `{pattern}`."
                    ));
                }
            }
        }
        None
    }
}

pub(in super::super) fn runtime_gemini_blocked_tool_call_message(
    name: &str,
    args: &serde_json::Value,
) -> Option<String> {
    RuntimeGeminiPolicyCompat::from_request_and_files(&serde_json::Value::Null)
        .blocked_tool_call_message(name, args)
}

fn runtime_gemini_tool_call_command_text(args: &serde_json::Value) -> String {
    if let Some(object) = args.as_object() {
        for key in [
            "command",
            "cmd",
            "shell_command",
            "shellCommand",
            "command_line",
            "commandLine",
            "script",
        ] {
            if let Some(value) = object.get(key).and_then(serde_json::Value::as_str) {
                return value.to_string();
            }
        }
    }
    match args {
        serde_json::Value::String(text) => text.to_string(),
        _ => serde_json::to_string(args).unwrap_or_default(),
    }
}

fn command_matches_policy_pattern(command: &str, pattern: &str) -> bool {
    let command = command.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();
    command.contains(pattern.trim())
}

fn runtime_gemini_parse_command_specific_tool(value: &str) -> Option<(String, String)> {
    let value = value.trim();
    let open = value.find('(')?;
    let close = value.rfind(')')?;
    if close <= open {
        return None;
    }
    let tool = value[..open].trim();
    let pattern = value[open + 1..close].trim();
    if tool.is_empty() || pattern.is_empty() {
        return None;
    }
    Some((tool.to_string(), pattern.to_string()))
}

fn runtime_gemini_policy_command_pattern_from_rule(rule: &toml::Value) -> Option<String> {
    for key in [
        "command",
        "pattern",
        "matches",
        "commandPattern",
        "command_pattern",
    ] {
        if let Some(pattern) = rule.get(key).and_then(toml::Value::as_str) {
            let pattern = pattern.trim();
            if !pattern.is_empty() {
                return Some(pattern.to_string());
            }
        }
    }
    None
}

fn runtime_gemini_settings_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = runtime_gemini_home_dir() {
        paths.push(home.join(".gemini").join("settings.json"));
    }
    if let Ok(cwd) = env::current_dir() {
        paths.push(cwd.join(".gemini").join("settings.json"));
        paths.push(cwd.join(".gemini").join("settings.local.json"));
    }
    paths
}

fn runtime_gemini_normalize_tool_name(name: &str) -> String {
    let mut name = name.trim().to_ascii_lowercase().replace('-', "_");
    if let Some(suffix) = name.rsplit('.').next() {
        name = suffix.to_string();
    }
    name
}

fn runtime_gemini_tool_aliases(name: &str) -> BTreeSet<String> {
    let mut aliases = BTreeSet::new();
    let normalized = runtime_gemini_normalize_tool_name(name);
    aliases.insert(normalized.clone());
    if let Some(suffix) = normalized.rsplit("__").next() {
        aliases.insert(suffix.to_string());
    }
    match normalized.as_str() {
        "exec_command" | "run_shell_command" | "shell" | "bash" => {
            aliases
                .extend(["exec_command", "run_shell_command", "shell", "bash"].map(str::to_string));
        }
        "apply_patch" | "edit" | "replace" => {
            aliases.extend(["apply_patch", "edit", "replace"].map(str::to_string));
        }
        "read_file" | "read" => {
            aliases.extend(["read_file", "read"].map(str::to_string));
        }
        "read_many_files" | "glob" => {
            aliases.extend(["read_many_files", "glob"].map(str::to_string));
        }
        "grep" | "rip_grep" | "rg" | "search" => {
            aliases.extend(["grep", "rip_grep", "rg", "search"].map(str::to_string));
        }
        "write_file" | "write" => {
            aliases.extend(["write_file", "write"].map(str::to_string));
        }
        _ => {}
    }
    aliases
}

fn runtime_gemini_tool_is_mutating(name: &str) -> bool {
    let aliases = runtime_gemini_tool_aliases(name);
    aliases.iter().any(|alias| {
        matches!(
            alias.as_str(),
            "apply_patch"
                | "edit"
                | "replace"
                | "write"
                | "write_file"
                | "exec_command"
                | "run_shell_command"
                | "shell"
                | "bash"
        )
    })
}

fn runtime_gemini_function_tools_from_chat(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
) -> Option<serde_json::Value> {
    let mut declarations = chat
        .get("tools")?
        .as_array()?
        .iter()
        .filter_map(runtime_gemini_function_declaration_from_chat_tool)
        .collect::<Vec<_>>();
    runtime_gemini_apply_gemini3_tool_declaration_overrides(model, &mut declarations);
    RuntimeGeminiPolicyCompat::from_request_and_files(original)
        .filter_function_declarations(&mut declarations);
    (!declarations.is_empty()).then(|| {
        serde_json::json!([{
            "functionDeclarations": declarations,
        }])
    })
}

fn runtime_gemini_apply_gemini3_tool_declaration_overrides(
    model: &str,
    declarations: &mut [serde_json::Value],
) {
    if !runtime_gemini_model_uses_gemini3_toolset(model) {
        return;
    }
    for declaration in declarations {
        let Some(name) = declaration
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        if let Some(description) = runtime_gemini_gemini3_tool_description(&name)
            && let Some(object) = declaration.as_object_mut()
        {
            object.insert(
                "description".to_string(),
                serde_json::Value::String(description.to_string()),
            );
        }
        runtime_gemini_apply_gemini3_parameter_descriptions(&name, declaration);
    }
}

fn runtime_gemini_model_uses_gemini3_toolset(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    model.contains("gemini-3") || model == "auto" || model.contains("auto-gemini-3")
}

fn runtime_gemini_gemini3_tool_description(name: &str) -> Option<&'static str> {
    let aliases = runtime_gemini_tool_aliases(name);
    if aliases.contains("read_file") {
        return Some(
            "Read a file with targeted, surgical ranges. Prefer start_line and end_line when only part of the file is needed.",
        );
    }
    if aliases.contains("read_many_files") {
        return Some(
            "Read multiple files by paths or globs; honor git, Gemini, custom ignore rules, and default heavy-directory excludes.",
        );
    }
    if aliases.contains("grep") || aliases.contains("rg") || aliases.contains("rip_grep") {
        return Some(
            "Search text fast with ripgrep-style behavior. Use this before broad reads when locating symbols or literals.",
        );
    }
    if aliases.contains("exec_command") || aliases.contains("run_shell_command") {
        return Some(
            "Run a shell command. Prefer non-interactive commands and continue background sessions until the needed output is available.",
        );
    }
    if aliases.contains("write_file") {
        return Some(
            "Write complete file content. Do not use placeholders; preserve unrelated user changes.",
        );
    }
    if aliases.contains("apply_patch") || aliases.contains("replace") || aliases.contains("edit") {
        return Some(
            "Apply targeted file edits with exact context. Keep edits narrow and use this instead of describing changes.",
        );
    }
    None
}

fn runtime_gemini_apply_gemini3_parameter_descriptions(
    name: &str,
    declaration: &mut serde_json::Value,
) {
    let aliases = runtime_gemini_tool_aliases(name);
    if aliases.contains("read_file") {
        runtime_gemini_set_parameter_description(
            declaration,
            "start_line",
            "1-based first line to read when a targeted range is enough.",
        );
        runtime_gemini_set_parameter_description(
            declaration,
            "end_line",
            "1-based last line to read, inclusive.",
        );
    }
    if aliases.contains("grep") || aliases.contains("rg") || aliases.contains("rip_grep") {
        runtime_gemini_set_parameter_description(
            declaration,
            "pattern",
            "Literal or regex pattern to search for.",
        );
    }
}

fn runtime_gemini_set_parameter_description(
    declaration: &mut serde_json::Value,
    parameter: &str,
    description: &str,
) {
    let Some(properties) = declaration
        .get_mut("parameters")
        .and_then(|parameters| parameters.get_mut("properties"))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    let Some(property) = properties
        .get_mut(parameter)
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    property
        .entry("description".to_string())
        .or_insert_with(|| serde_json::Value::String(description.to_string()));
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
    thinking_budget_tokens: Option<u64>,
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
    for (from, to) in [
        ("top_k", "topK"),
        ("topK", "topK"),
        ("candidate_count", "candidateCount"),
        ("candidateCount", "candidateCount"),
        ("seed", "seed"),
        ("presence_penalty", "presencePenalty"),
        ("presencePenalty", "presencePenalty"),
        ("frequency_penalty", "frequencyPenalty"),
        ("frequencyPenalty", "frequencyPenalty"),
        ("response_mime_type", "responseMimeType"),
        ("responseMimeType", "responseMimeType"),
        ("response_schema", "responseSchema"),
        ("responseSchema", "responseSchema"),
        ("response_json_schema", "responseJsonSchema"),
        ("responseJsonSchema", "responseJsonSchema"),
        ("response_modalities", "responseModalities"),
        ("responseModalities", "responseModalities"),
        ("media_resolution", "mediaResolution"),
        ("mediaResolution", "mediaResolution"),
        ("audio_timestamp", "audioTimestamp"),
        ("audioTimestamp", "audioTimestamp"),
        ("speech_config", "speechConfig"),
        ("speechConfig", "speechConfig"),
    ] {
        if let Some(value) = original.get(from).filter(|value| !value.is_null()) {
            config.insert(to.to_string(), value.clone());
        }
    }
    if let Some(stop) = original
        .get("stop")
        .or_else(|| original.get("stop_sequences"))
        .or_else(|| original.get("stopSequences"))
        .filter(|value| !value.is_null())
    {
        config.insert("stopSequences".to_string(), stop.clone());
    }
    runtime_gemini_apply_text_format(original, &mut config);
    if let Some(thinking_config) =
        runtime_gemini_thinking_config(original, model, thinking_budget_tokens)
    {
        config.insert("thinkingConfig".to_string(), thinking_config);
    }
    serde_json::Value::Object(config)
}

fn runtime_gemini_apply_text_format(
    original: &serde_json::Value,
    config: &mut serde_json::Map<String, serde_json::Value>,
) {
    let Some(text) = original.get("text").and_then(serde_json::Value::as_object) else {
        return;
    };
    let Some(format) = text.get("format").and_then(serde_json::Value::as_object) else {
        return;
    };
    let format_type = format
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match format_type {
        "json_object" => {
            config.insert(
                "responseMimeType".to_string(),
                serde_json::Value::String("application/json".to_string()),
            );
        }
        "json_schema" => {
            config.insert(
                "responseMimeType".to_string(),
                serde_json::Value::String("application/json".to_string()),
            );
            if let Some(schema) = format.get("schema").or_else(|| format.get("json_schema")) {
                config.insert("responseJsonSchema".to_string(), schema.clone());
            }
        }
        _ => {}
    }
}

fn runtime_gemini_thinking_config(
    original: &serde_json::Value,
    model: &str,
    thinking_budget_tokens: Option<u64>,
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
    let budget = match (thinking_budget_tokens, effort.as_str()) {
        (Some(budget), _) => budget,
        (None, "low") => 1024,
        (None, "medium" | "high") => 8192,
        (None, "xhigh") => 24576,
        (None, _) => 8192,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Arc, Mutex};

    fn test_conversation_store() -> RuntimeDeepSeekConversationStore {
        Arc::new(Mutex::new(std::collections::BTreeMap::new()))
    }

    #[test]
    fn gemini_context_filter_honors_nested_gitignore_base_rules() {
        let directory = std::env::temp_dir().join(format!(
            "prodex-gemini-nested-ignore-{}",
            std::process::id()
        ));
        let nested = directory.join("nested");
        let other = directory.join("other");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&other).unwrap();
        fs::write(nested.join(".gitignore"), "*.log\n!keep.log\n").unwrap();
        fs::write(nested.join("ignored.log"), "nested ignored").unwrap();
        fs::write(nested.join("keep.log"), "nested kept").unwrap();
        fs::write(other.join("ignored.log"), "other kept").unwrap();

        let mut rules = Vec::new();
        let mut scanned = 0;
        runtime_gemini_load_nested_gitignore_rules(
            &directory,
            &directory,
            false,
            &mut rules,
            &mut scanned,
        );
        let filter = RuntimeGeminiContextFilter {
            project_root: Some(directory.clone()),
            use_default_excludes: false,
            project_rules: rules,
        };

        assert!(filter.is_excluded(&nested.join("ignored.log"), &[]));
        assert!(!filter.is_excluded(&nested.join("keep.log"), &[]));
        assert!(!filter.is_excluded(&other.join("ignored.log"), &[]));
        fs::remove_dir_all(directory).unwrap();
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
}
