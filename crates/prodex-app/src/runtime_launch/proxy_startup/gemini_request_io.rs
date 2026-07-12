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
    if !runtime_gemini_same_text_file(&metadata, &file.metadata().ok()?) {
        return None;
    }
    let mut reader = file.take(limit as u64);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).to_string())
}

#[cfg(unix)]
fn runtime_gemini_same_text_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn runtime_gemini_same_text_file(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    true
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

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;

    #[cfg(unix)]
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

    #[cfg(unix)]
    #[test]
    fn gemini_same_text_file_rejects_replaced_inode() {
        use std::os::unix::fs::MetadataExt;

        let root = std::env::temp_dir().join(format!(
            "prodex-gemini-same-text-file-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path_a = root.join("a.txt");
        let path_b = root.join("b.txt");

        fs::write(&path_a, "first").unwrap();
        fs::write(&path_b, "second").unwrap();
        let before = fs::symlink_metadata(&path_a).unwrap();
        let after = fs::symlink_metadata(&path_b).unwrap();
        assert_eq!(before.dev(), after.dev());
        assert_ne!(before.ino(), after.ino());
        assert!(!runtime_gemini_same_text_file(&before, &after));

        fs::remove_dir_all(root).unwrap();
    }
}
