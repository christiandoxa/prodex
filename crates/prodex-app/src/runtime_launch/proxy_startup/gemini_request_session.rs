use super::gemini_request::RUNTIME_GEMINI_IMPORT_BYTE_LIMIT;
use super::gemini_request_io::runtime_gemini_read_text_limited;
use anyhow::{Context, Result};
use prodex_provider_core::gemini_provider_core_collect_path_values;
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
    let checkpoint = prodex_provider_core::gemini_provider_core_session_checkpoint_value(request);
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
) -> Vec<serde_json::Value> {
    let mut contents = Vec::new();
    for value in runtime_gemini_import_values(original) {
        contents.extend(
            prodex_provider_core::gemini_provider_core_import_contents_from_value(
                &value,
                RUNTIME_GEMINI_IMPORT_BYTE_LIMIT,
            ),
        );
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
        gemini_provider_core_collect_path_values(original.get(key), &mut paths);
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
