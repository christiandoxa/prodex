use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn collect_files(
    directory: &Path,
    extension: &str,
    limit: usize,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_inner(directory, extension, limit, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_inner(
    directory: &Path,
    extension: &str,
    limit: usize,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    if files.len() >= limit {
        return Ok(());
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if files.len() >= limit {
            break;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_files_inner(&path, extension, limit, files)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            files.push(path);
        }
    }
    Ok(())
}

pub(super) fn copy_dir_limited(source: &Path, target: &Path, limit: usize) -> Result<()> {
    let Ok(entries) = fs::read_dir(source) else {
        return Ok(());
    };
    for entry in entries.flatten().take(limit) {
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            fs::create_dir_all(&target_path)
                .with_context(|| format!("failed to create {}", target_path.display()))?;
            copy_dir_limited(&source_path, &target_path, limit)?;
        } else if source_path.is_file() {
            fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

pub(super) fn read_text_limited(path: &Path, limit: usize) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() as usize > limit {
        return None;
    }
    fs::read_to_string(path).ok()
}

pub(super) fn write_executable_script(path: &Path, script: &str) -> Result<()> {
    fs::write(path, script).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to stat {}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to chmod {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("prodex-gemini-cli-compat-fs-{name}-{stamp}"))
    }

    #[test]
    fn read_text_limited_rejects_oversized_files() {
        let root = temp_dir("limit");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("value.txt");
        fs::write(&path, "hello").unwrap();

        assert_eq!(read_text_limited(&path, 5).as_deref(), Some("hello"));
        assert_eq!(read_text_limited(&path, 4), None);

        fs::remove_dir_all(root).unwrap();
    }
}
