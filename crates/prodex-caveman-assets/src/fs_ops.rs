use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Read as _;
use std::io::Write as _;
use std::path::Path;

pub(crate) const TEXT_FILE_READ_LIMIT: usize = 512 * 1024;

pub(crate) fn write_file(path: &Path, contents: &[u8]) -> Result<()> {
    write_file_with_mode(path, contents, 0o600)
}

pub(crate) fn read_text_file_limited(path: &Path) -> Result<Option<String>> {
    ensure_no_symlink_component(path)?;
    read_text_file_limited_unchecked(path)
}

pub(crate) fn read_canonical_text_file_limited(path: &Path) -> Result<Option<String>> {
    let Ok(path) = fs::canonicalize(path) else {
        return Ok(None);
    };
    read_text_file_limited_unchecked(&path)
}

pub(crate) fn write_text_file(path: &Path, contents: &str) -> Result<()> {
    write_file(path, contents.as_bytes())
}

pub(crate) fn write_executable_file(path: &Path, contents: &str) -> Result<()> {
    write_file_with_mode(path, contents.as_bytes(), 0o755)
}

pub(crate) fn copy_file_streaming(source: &Path, destination: &Path) -> Result<bool> {
    let mut source_file = match fs::File::open(source) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", source.display()));
        }
    };
    let source_metadata = source_file
        .metadata()
        .with_context(|| format!("failed to inspect {}", source.display()))?;
    if !source_metadata.is_file() {
        bail!("{} is not a file", source.display());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        ensure_no_symlink_component(parent)?;
    }
    ensure_parent_has_no_symlink_component(destination)?;
    remove_existing_file_path(destination)?;
    ensure_parent_has_no_symlink_component(destination)?;

    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut destination_file = options
        .open(destination)
        .with_context(|| format!("failed to write {}", destination.display()))?;
    std::io::copy(&mut source_file, &mut destination_file).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    destination_file
        .flush()
        .with_context(|| format!("failed to write {}", destination.display()))?;
    Ok(true)
}

pub(crate) fn remove_existing_dir_path(path: &Path) -> Result<()> {
    ensure_parent_has_no_symlink_component(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };
    if metadata.file_type().is_symlink() {
        return fs::remove_file(path)
            .or_else(|_| fs::remove_dir(path))
            .with_context(|| format!("failed to remove {}", path.display()));
    }
    if !metadata.is_dir() {
        bail!("{} is not a directory", path.display());
    }
    fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))
}

fn write_file_with_mode(path: &Path, contents: &[u8], _mode: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        ensure_no_symlink_component(parent)?;
    }
    ensure_parent_has_no_symlink_component(path)?;
    remove_existing_file_path(path)?;
    ensure_parent_has_no_symlink_component(path)?;

    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(_mode);
    }

    let mut file = options
        .open(path)
        .with_context(|| format!("failed to write {}", path.display()))?;
    use std::io::Write as _;
    file.write_all(contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn remove_existing_file_path(path: &Path) -> Result<()> {
    ensure_parent_has_no_symlink_component(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        bail!("{} is a directory, expected a file", path.display());
    }
    fs::remove_file(path)
        .or_else(|_| fs::remove_dir(path))
        .with_context(|| format!("failed to remove {}", path.display()))
}

fn read_text_file_limited_unchecked(path: &Path) -> Result<Option<String>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };
    if !metadata.is_file() || metadata.len() > TEXT_FILE_READ_LIMIT as u64 {
        return Ok(None);
    }
    let file =
        fs::File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    let opened_metadata = file
        .metadata()
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if !same_file_metadata(&metadata, &opened_metadata) {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take((TEXT_FILE_READ_LIMIT as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if bytes.len() > TEXT_FILE_READ_LIMIT {
        return Ok(None);
    }
    let contents = String::from_utf8(bytes)
        .with_context(|| format!("failed to decode UTF-8 {}", path.display()))?;
    Ok(Some(contents))
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

fn ensure_parent_has_no_symlink_component(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_no_symlink_component(parent)?;
    }
    Ok(())
}

fn ensure_no_symlink_component(path: &Path) -> Result<()> {
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to inspect {}", current.display()));
            }
        };
        if metadata.file_type().is_symlink() {
            bail!("refusing to operate through symlink {}", current.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir()
            .canonicalize()
            .expect("temp dir should resolve")
            .join(format!(
                "prodex-caveman-fs-ops-{name}-{}-{nanos}",
                std::process::id()
            ))
    }

    #[cfg(unix)]
    #[test]
    fn write_text_file_replaces_symlink_without_touching_target() {
        let root = temp_dir("symlink");
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.txt");
        let path = root.join("file.txt");
        fs::write(&target, "do not touch").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        write_text_file(&path, "new contents").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "do not touch");
        assert!(
            !fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "new contents");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn remove_existing_dir_path_removes_symlink_without_touching_target() {
        let root = temp_dir("dir-symlink");
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target");
        let path = root.join("cache");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("keep.txt"), "keep").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        remove_existing_dir_path(&path).unwrap();

        assert!(fs::symlink_metadata(&path).is_err());
        assert_eq!(fs::read_to_string(target.join("keep.txt")).unwrap(), "keep");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn write_text_file_rejects_symlink_parent() {
        let root = temp_dir("write-parent-symlink");
        let outside = root.join("outside");
        let link = root.join("link");
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let result = write_text_file(&link.join("file.txt"), "do not write");

        assert!(result.is_err());
        assert!(!outside.join("file.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn remove_existing_dir_path_rejects_symlink_parent() {
        let root = temp_dir("remove-parent-symlink");
        let outside = root.join("outside");
        let link = root.join("link");
        fs::create_dir_all(outside.join("cache")).unwrap();
        fs::write(outside.join("cache").join("keep.txt"), "keep").unwrap();
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let result = remove_existing_dir_path(&link.join("cache"));

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(outside.join("cache").join("keep.txt")).unwrap(),
            "keep"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn read_text_file_limited_rejects_oversized_file() {
        let root = temp_dir("read-oversized");
        let path = root.join("config.toml");
        fs::create_dir_all(&root).unwrap();
        fs::write(&path, vec![b'a'; TEXT_FILE_READ_LIMIT + 1]).unwrap();

        assert!(read_text_file_limited(&path).unwrap().is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn read_text_file_limited_rejects_symlink_file() {
        let root = temp_dir("read-symlink");
        let target = root.join("target.txt");
        let link = root.join("config.toml");
        fs::create_dir_all(&root).unwrap();
        fs::write(&target, "secret").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(read_text_file_limited(&link).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn write_executable_file_sets_private_executable_mode() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_dir("executable");
        let path = root.join("bin/tool");

        write_executable_file(&path, "#!/usr/bin/env sh\n").unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o755
        );
        let _ = fs::remove_dir_all(root);
    }
}
