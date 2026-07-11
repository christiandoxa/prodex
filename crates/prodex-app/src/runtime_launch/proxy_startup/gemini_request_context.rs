use super::super::gemini_request_io::runtime_gemini_path_has_symlink_component;
use super::gemini_request_local_context::{
    RUNTIME_GEMINI_CONTEXT_FILE_LIMIT, RuntimeGeminiFileReadBudget,
    runtime_gemini_part_from_local_path, runtime_gemini_resolve_local_path,
};
use prodex_provider_core::{
    gemini_provider_core_collect_input_texts, gemini_provider_core_collect_string_values,
    gemini_provider_core_skip_context_path_name,
};
use std::fs;
use std::path::{Path, PathBuf};

mod filter;
mod path_match;

use self::filter::RuntimeGeminiContextFilter;
use self::path_match::{
    runtime_gemini_context_match_path, runtime_gemini_glob_matches, runtime_gemini_glob_root,
    runtime_gemini_path_has_glob,
};

const RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT: usize = 2048;
pub(super) fn runtime_gemini_collect_explicit_file_parts(
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
    gemini_provider_core_collect_string_values(object.get("include"), &mut includes);
    let mut excludes = Vec::new();
    gemini_provider_core_collect_string_values(object.get("exclude"), &mut excludes);
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
    if runtime_gemini_path_has_symlink_component(&root) {
        return;
    }
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
        if use_default_excludes && gemini_provider_core_skip_context_path_name(name) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            runtime_gemini_collect_context_candidates(
                &path,
                use_default_excludes,
                candidates,
                scanned,
            );
        } else if metadata.is_file() {
            candidates.push(path);
        }
    }
}

pub(super) fn runtime_gemini_collect_at_path_parts(
    original: &serde_json::Value,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    let mut texts = Vec::new();
    gemini_provider_core_collect_input_texts(original.get("input"), &mut texts);
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
