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
        fs::copy(source_path, destination_path).with_context(|| {
            format!(
                "failed to copy {} to {}",
                source_path.display(),
                destination_path.display()
            )
        })?;
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
        std::os::unix::fs::symlink(target, destination_path)
            .with_context(|| format!("failed to recreate symlink {}", destination_path.display()))
    }

    #[cfg(not(unix))]
    {
        let _ = (source_path, destination_path);
        bail!("symlinks are not supported on this platform");
    }
}
