use super::*;

pub(crate) fn copy_codex_home(source: &Path, destination: &Path) -> Result<()> {
    if !source.is_dir() {
        bail!("copy source {} is not a directory", source.display());
    }

    if same_path(source, destination) {
        bail!("copy source and destination are the same path");
    }

    if destination.exists() && !dir_is_empty(destination)? {
        bail!(
            "destination {} already exists and is not empty",
            destination.display()
        );
    }

    create_codex_home_if_missing(destination)?;
    copy_directory_contents(source, destination)
}

pub(super) fn copy_directory_contents(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory {}", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source.display()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read metadata for {}", source_path.display()))?;
        copy_directory_entry(&source_path, &destination_path, file_type)?;
    }

    Ok(())
}

fn copy_directory_entry(
    source_path: &Path,
    destination_path: &Path,
    file_type: fs::FileType,
) -> Result<()> {
    if file_type.is_dir() {
        create_codex_home_if_missing(destination_path)?;
        return copy_directory_contents(source_path, destination_path);
    }

    if file_type.is_file() {
        copy_shared_codex_file_replacing_existing(source_path, destination_path, "failed to copy")?;
        return Ok(());
    }

    if file_type.is_symlink() {
        return recreate_symlink(source_path, destination_path);
    }

    Ok(())
}

fn recreate_symlink(source_path: &Path, destination_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let target = fs::read_link(source_path)
            .with_context(|| format!("failed to read symlink {}", source_path.display()))?;
        if fs::symlink_metadata(destination_path).is_ok() {
            remove_path(destination_path)?;
        }
        std::os::unix::fs::symlink(target, destination_path)
            .with_context(|| format!("failed to recreate symlink {}", destination_path.display()))
    }

    #[cfg(not(unix))]
    {
        let _ = (source_path, destination_path);
        bail!("symlinks are not supported on this platform");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct CopyTestDir {
        path: PathBuf,
    }

    impl CopyTestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            let path = env::temp_dir().join(format!(
                "prodex-shared-copy-{name}-{}-{unique}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("test dir should be created");
            Self { path }
        }
    }

    impl Drop for CopyTestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn make_readonly(path: &Path) {
        let mut permissions = fs::metadata(path)
            .expect("metadata should read")
            .permissions();
        permissions.set_readonly(true);
        fs::set_permissions(path, permissions).expect("permissions should update");
    }

    #[test]
    fn copy_directory_contents_replaces_readonly_existing_file() {
        let temp_dir = CopyTestDir::new("readonly-existing-file");
        let source = temp_dir.path.join("source");
        let destination = temp_dir.path.join("destination");
        let relative_pack_path = Path::new(".tmp/plugins/.git/objects/pack/pack-test.pack");
        let source_pack_path = source.join(relative_pack_path);
        let destination_pack_path = destination.join(relative_pack_path);

        fs::create_dir_all(source_pack_path.parent().expect("source parent"))
            .expect("source parent should be created");
        fs::create_dir_all(destination_pack_path.parent().expect("destination parent"))
            .expect("destination parent should be created");
        fs::write(&source_pack_path, "fresh plugin pack").expect("source pack should write");
        fs::write(&destination_pack_path, "stale plugin pack")
            .expect("destination pack should write");
        make_readonly(&destination_pack_path);

        copy_directory_contents(&source, &destination)
            .expect("readonly destination pack should be replaced");

        assert_eq!(
            fs::read_to_string(&destination_pack_path)
                .expect("destination pack should be readable"),
            "fresh plugin pack"
        );
    }
}
