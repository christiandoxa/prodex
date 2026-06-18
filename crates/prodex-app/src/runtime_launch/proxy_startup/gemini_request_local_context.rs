use super::super::gemini_request_media::runtime_gemini_mime_type_for_uri;
use base64::Engine;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const RUNTIME_GEMINI_CONTEXT_FILE_LIMIT: usize = 16;
const RUNTIME_GEMINI_CONTEXT_BYTE_LIMIT: usize = 128 * 1024;

#[derive(Default)]
pub(super) struct RuntimeGeminiFileReadBudget {
    pub(super) files: usize,
    bytes: usize,
    paths: BTreeSet<PathBuf>,
}

pub(super) fn runtime_gemini_part_from_local_path(
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

pub(super) fn runtime_gemini_resolve_local_path(path: &Path) -> Option<PathBuf> {
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

pub(super) fn runtime_gemini_skip_context_path_name(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | "__pycache__" | "vendor"
    )
}
