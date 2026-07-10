use super::gemini_request::RUNTIME_GEMINI_IMPORT_BYTE_LIMIT;
use super::gemini_request_io::{
    runtime_gemini_collect_path_values, runtime_gemini_read_text_limited,
    runtime_gemini_truncate_to_bytes,
};
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

pub(super) fn runtime_gemini_export_checkpoint(
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

pub(super) fn runtime_gemini_imported_session_contents(
    original: &serde_json::Value,
    allow_local_file_access: bool,
) -> Vec<serde_json::Value> {
    let mut contents = Vec::new();
    for value in runtime_gemini_import_values(original, allow_local_file_access) {
        contents.extend(runtime_gemini_import_contents_from_value(&value));
    }
    contents
}

fn runtime_gemini_import_values(
    original: &serde_json::Value,
    allow_local_file_access: bool,
) -> Vec<serde_json::Value> {
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
    if !allow_local_file_access {
        return values;
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
