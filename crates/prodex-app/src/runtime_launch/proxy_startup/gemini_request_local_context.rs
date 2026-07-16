use super::super::gemini_request_io::runtime_gemini_path_has_symlink_component;
use base64::Engine;
use prodex_provider_core::{
    gemini_provider_core_local_context_text_part, gemini_provider_core_media_part_from_data,
    gemini_provider_core_mime_type_for_uri, gemini_provider_core_mime_type_is_text,
    gemini_provider_core_skip_context_path_name, gemini_provider_core_text_part,
};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

pub(super) const RUNTIME_GEMINI_CONTEXT_FILE_LIMIT: usize = 16;
pub(super) const RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT: usize = 2048;
pub(super) const RUNTIME_GEMINI_CONTEXT_DEPTH_LIMIT: usize = 32;
pub(super) const RUNTIME_GEMINI_CONTEXT_DIRECTORY_ENTRY_LIMIT: usize = 256;
const RUNTIME_GEMINI_CONTEXT_BYTE_LIMIT: usize = 128 * 1024;

#[derive(Default)]
pub(super) struct RuntimeGeminiFileReadBudget {
    pub(super) files: usize,
    bytes: usize,
    paths: BTreeSet<PathBuf>,
    pub(super) scanned_entries: usize,
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
    if runtime_gemini_path_has_symlink_component(&path) {
        return None;
    }
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
    let data = runtime_gemini_read_local_context_file(
        &path,
        &metadata,
        RUNTIME_GEMINI_CONTEXT_BYTE_LIMIT.saturating_sub(budget.bytes),
    )?;
    budget.files = budget.files.saturating_add(1);
    budget.bytes = budget.bytes.saturating_add(data.len());
    budget.paths.insert(dedup_path);
    let mime_type = mime_type
        .unwrap_or_else(|| gemini_provider_core_mime_type_for_uri(&path.to_string_lossy()));
    if gemini_provider_core_mime_type_is_text(mime_type)
        && let Ok(text) = String::from_utf8(data.clone())
    {
        return Some(gemini_provider_core_local_context_text_part(
            path.display(),
            &text,
        ));
    }
    gemini_provider_core_media_part_from_data(
        &base64::engine::general_purpose::STANDARD.encode(data),
        Some(mime_type),
    )
}

fn runtime_gemini_part_from_local_dir(
    path: &Path,
    budget: &mut RuntimeGeminiFileReadBudget,
) -> Option<serde_json::Value> {
    runtime_gemini_part_from_local_dir_at_depth(path, budget, 0)
}

fn runtime_gemini_part_from_local_dir_at_depth(
    path: &Path,
    budget: &mut RuntimeGeminiFileReadBudget,
    depth: usize,
) -> Option<serde_json::Value> {
    if depth >= RUNTIME_GEMINI_CONTEXT_DEPTH_LIMIT
        || budget.scanned_entries >= RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT
    {
        return None;
    }
    let remaining = RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT.saturating_sub(budget.scanned_entries);
    let entry_limit = remaining.min(RUNTIME_GEMINI_CONTEXT_DIRECTORY_ENTRY_LIMIT);
    let mut scanned = 0usize;
    let mut entries = fs::read_dir(path)
        .ok()?
        .take(entry_limit)
        .filter_map(|entry| {
            scanned = scanned.saturating_add(1);
            entry.ok().map(|entry| entry.path())
        })
        .collect::<Vec<_>>();
    budget.scanned_entries = budget.scanned_entries.saturating_add(scanned);
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
        if gemini_provider_core_skip_context_path_name(name) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&entry) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if let Some(part) =
                runtime_gemini_part_from_local_dir_at_depth(&entry, budget, depth + 1)
            {
                parts.push(part);
            }
        } else if metadata.is_file()
            && let Some(part) = runtime_gemini_part_from_local_path(&entry, None, budget)
        {
            parts.push(part);
        }
    }
    (!parts.is_empty()).then(|| {
        gemini_provider_core_text_part(
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join("\n\n"),
        )
    })
}

fn runtime_gemini_read_local_context_file(
    path: &Path,
    metadata: &fs::Metadata,
    max_bytes: usize,
) -> Option<Vec<u8>> {
    let file = fs::File::open(path).ok()?;
    let opened_metadata = file.metadata().ok()?;
    if !runtime_gemini_same_local_context_file(metadata, &opened_metadata) {
        return None;
    }

    let mut data = Vec::new();
    file.take((max_bytes as u64).saturating_add(1))
        .read_to_end(&mut data)
        .ok()?;
    (data.len() <= max_bytes).then_some(data)
}

#[cfg(unix)]
fn runtime_gemini_same_local_context_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn runtime_gemini_same_local_context_file(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    true
}

pub(super) fn runtime_gemini_resolve_local_path(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    std::env::current_dir().ok().map(|cwd| cwd.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_local_context_file_read_rejects_replaced_file() {
        let path = runtime_gemini_local_context_test_path("replaced");
        fs::write(&path, "small").unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let _ = fs::remove_file(&path);
        fs::File::create(&path)
            .unwrap()
            .set_len((RUNTIME_GEMINI_CONTEXT_BYTE_LIMIT as u64).saturating_add(1))
            .unwrap();

        let data = runtime_gemini_read_local_context_file(
            &path,
            &metadata,
            RUNTIME_GEMINI_CONTEXT_BYTE_LIMIT,
        );

        assert!(data.is_none());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn gemini_local_context_file_read_respects_byte_limit() {
        let path = runtime_gemini_local_context_test_path("limit");
        fs::write(&path, b"abcdef").unwrap();
        let metadata = fs::metadata(&path).unwrap();

        let data = runtime_gemini_read_local_context_file(&path, &metadata, 5);

        assert!(data.is_none());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn gemini_local_context_directory_scan_is_bounded() {
        let root = runtime_gemini_local_context_test_path("huge-directory");
        fs::create_dir_all(&root).unwrap();
        for index in 0..RUNTIME_GEMINI_CONTEXT_DIRECTORY_ENTRY_LIMIT + 32 {
            fs::write(root.join(format!("{index:04}.txt")), "context").unwrap();
        }
        let mut budget = RuntimeGeminiFileReadBudget::default();

        let part = runtime_gemini_part_from_local_path(&root, None, &mut budget);

        assert!(part.is_some());
        assert_eq!(
            budget.scanned_entries,
            RUNTIME_GEMINI_CONTEXT_DIRECTORY_ENTRY_LIMIT
        );
        assert_eq!(budget.files, RUNTIME_GEMINI_CONTEXT_FILE_LIMIT);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn gemini_local_context_directory_scan_skips_symlinked_directory() {
        let root = runtime_gemini_local_context_test_path("symlink-directory");
        let outside = runtime_gemini_local_context_test_path("symlink-directory-outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "outside secret").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
        let mut budget = RuntimeGeminiFileReadBudget::default();

        let part = runtime_gemini_part_from_local_path(&root, None, &mut budget);

        assert!(part.is_none());
        assert_eq!(budget.files, 0);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    fn runtime_gemini_local_context_test_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "prodex-gemini-local-context-{name}-{}-{nanos}",
            std::process::id()
        ))
    }
}
