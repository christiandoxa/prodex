use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub(super) fn runtime_gemini_read_text_limited(path: &Path, limit: usize) -> Option<String> {
    if limit == 0 {
        return None;
    }
    if runtime_gemini_path_has_symlink_component(path) {
        return None;
    }
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let mut reader = file.take(limit as u64);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).to_string())
}

pub(super) fn runtime_gemini_path_has_symlink_component(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn gemini_read_text_limited_rejects_symlink_components() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gemini-read-text-symlink-{}",
            std::process::id()
        ));
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "outside secret").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), workspace.join("linked.txt"))
            .unwrap();
        std::os::unix::fs::symlink(&outside, workspace.join("linked_dir")).unwrap();

        assert_eq!(
            runtime_gemini_read_text_limited(&workspace.join("linked.txt"), 1024),
            None
        );
        assert_eq!(
            runtime_gemini_read_text_limited(
                &workspace.join("linked_dir").join("secret.txt"),
                1024
            ),
            None
        );
        fs::remove_dir_all(root).unwrap();
    }
}
