use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub(super) fn runtime_gemini_read_text_limited(path: &Path, limit: usize) -> Option<String> {
    if limit == 0 {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let mut reader = file.take(limit as u64);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).to_string())
}

pub(super) fn runtime_gemini_truncate_to_bytes(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut end = limit.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

pub(super) fn runtime_gemini_collect_path_values(
    value: Option<&serde_json::Value>,
    paths: &mut Vec<PathBuf>,
) {
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
