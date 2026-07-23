use anyhow::{Context, Result};
use std::fs;
use std::io::Read as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

const GEMINI_EXTENSION_MAX_DEPTH: usize = 32;

pub(super) fn collect_files(
    directory: &Path,
    extension: &str,
    limit: usize,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut remaining = limit;
    collect_files_inner(directory, extension, &mut remaining, 0, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_inner(
    directory: &Path,
    extension: &str,
    remaining: &mut usize,
    depth: usize,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    if *remaining == 0 {
        return Ok(());
    }
    if depth > GEMINI_EXTENSION_MAX_DEPTH {
        anyhow::bail!("Gemini extension directory exceeds maximum scan depth");
    }
    if path_has_symlink_component(directory) {
        return Ok(());
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if *remaining == 0 {
            break;
        }
        *remaining = remaining.saturating_sub(1);
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_files_inner(&path, extension, remaining, depth + 1, files)?;
        } else if metadata.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some(extension)
        {
            files.push(path);
        }
    }
    Ok(())
}

pub(super) fn copy_dir_limited(source: &Path, target: &Path, limit: usize) -> Result<()> {
    let mut remaining = limit;
    copy_dir_limited_inner(source, target, &mut remaining, 0)
}

fn copy_dir_limited_inner(
    source: &Path,
    target: &Path,
    remaining: &mut usize,
    depth: usize,
) -> Result<()> {
    if depth > GEMINI_EXTENSION_MAX_DEPTH {
        anyhow::bail!("Gemini extension directory exceeds maximum copy depth");
    }
    if path_has_symlink_component(source) {
        return Ok(());
    }
    let Ok(entries) = fs::read_dir(source) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if *remaining == 0 {
            break;
        }
        *remaining = remaining.saturating_sub(1);
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)
            .with_context(|| format!("failed to stat {}", source_path.display()))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            fs::create_dir_all(&target_path)
                .with_context(|| format!("failed to create {}", target_path.display()))?;
            copy_dir_limited_inner(&source_path, &target_path, remaining, depth + 1)?;
        } else if metadata.is_file() {
            copy_file_limited(&source_path, &target_path, &metadata)?;
        }
    }
    Ok(())
}

fn copy_file_limited(source: &Path, target: &Path, metadata: &fs::Metadata) -> Result<()> {
    if metadata.len() > crate::GEMINI_COMPAT_FILE_LIMIT as u64 {
        return Ok(());
    }
    let file =
        fs::File::open(source).with_context(|| format!("failed to open {}", source.display()))?;
    let opened_metadata = file
        .metadata()
        .with_context(|| format!("failed to stat {}", source.display()))?;
    if !same_file_metadata(metadata, &opened_metadata) {
        return Ok(());
    }
    let mut bytes = Vec::new();
    file.take((crate::GEMINI_COMPAT_FILE_LIMIT as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", source.display()))?;
    if bytes.len() > crate::GEMINI_COMPAT_FILE_LIMIT {
        return Ok(());
    }
    write_file_atomic_no_symlink(target, bytes)?;
    fs::set_permissions(target, opened_metadata.permissions())
        .with_context(|| format!("failed to chmod {}", target.display()))?;
    Ok(())
}

pub(crate) fn read_text_limited(path: &Path, limit: usize) -> Option<String> {
    if path_has_symlink_component(path) {
        return None;
    }
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() as usize > limit {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let opened_metadata = file.metadata().ok()?;
    if !same_file_metadata(&metadata, &opened_metadata) {
        return None;
    }
    let mut bytes = Vec::new();
    file.take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > limit {
        return None;
    }
    String::from_utf8(bytes).ok()
}

pub(crate) fn path_has_symlink_component(path: &Path) -> bool {
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

#[cfg(unix)]
fn same_file_metadata(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn same_file_metadata(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    true
}

pub(super) fn write_executable_script(path: &Path, script: &str) -> Result<()> {
    write_file_atomic_no_symlink(path, script)?;
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

pub(crate) fn write_file_atomic_no_symlink(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if path_has_symlink_component(path) {
        anyhow::bail!("refusing to write through symlink {}", path.display());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        if path_has_symlink_component(parent) {
            anyhow::bail!("refusing to write through symlink {}", parent.display());
        }
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let parent = path.parent().unwrap_or(Path::new("."));
    let pid = std::process::id();
    for attempt in 0..16 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp_path = parent.join(format!(".{file_name}.prodex-tmp-{pid}-{nanos}-{attempt}"));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to create {}", tmp_path.display()));
            }
        };
        if let Err(error) = file.write_all(contents.as_ref()) {
            let _ = fs::remove_file(&tmp_path);
            return Err(error).with_context(|| format!("failed to write {}", tmp_path.display()));
        }
        if let Err(error) = file.sync_all() {
            let _ = fs::remove_file(&tmp_path);
            return Err(error).with_context(|| format!("failed to sync {}", tmp_path.display()));
        }
        drop(file);
        if path_has_symlink_component(path) {
            let _ = fs::remove_file(&tmp_path);
            anyhow::bail!("refusing to replace symlink {}", path.display());
        }
        return fs::rename(&tmp_path, path).with_context(|| {
            let _ = fs::remove_file(&tmp_path);
            format!("failed to write {}", path.display())
        });
    }
    anyhow::bail!("failed to allocate temporary path for {}", path.display())
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
        std::env::temp_dir()
            .canonicalize()
            .expect("temp dir should resolve")
            .join(format!("prodex-gemini-cli-compat-fs-{name}-{stamp}"))
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

    #[cfg(unix)]
    #[test]
    fn read_collect_and_copy_ignore_symlink_components() {
        let root = temp_dir("symlink");
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        let target = root.join("target");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(workspace.join("visible.md"), "visible").unwrap();
        fs::write(
            workspace.join("large.bin"),
            vec![b'x'; crate::GEMINI_COMPAT_FILE_LIMIT + 1],
        )
        .unwrap();
        fs::write(outside.join("secret.md"), "secret").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.md"), workspace.join("linked.md")).unwrap();
        std::os::unix::fs::symlink(&outside, workspace.join("linked_dir")).unwrap();

        assert_eq!(read_text_limited(&workspace.join("linked.md"), 1024), None);
        assert_eq!(
            read_text_limited(&workspace.join("linked_dir").join("secret.md"), 1024),
            None
        );
        let files = collect_files(&workspace, "md", 16).unwrap();
        assert_eq!(files, vec![workspace.join("visible.md")]);
        fs::create_dir_all(&target).unwrap();
        copy_dir_limited(&workspace, &target, 16).unwrap();
        assert!(target.join("visible.md").is_file());
        assert!(!target.join("large.bin").exists());
        assert!(!target.join("linked.md").exists());
        assert!(!target.join("linked_dir").exists());

        fs::remove_dir_all(root).unwrap();
    }
}
