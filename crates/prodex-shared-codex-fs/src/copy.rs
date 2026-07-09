use super::*;

pub fn copy_codex_home(source: &Path, destination: &Path) -> Result<()> {
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
    copy_directory_contents_under_root(source, source, destination)
}

fn copy_directory_contents_under_root(
    source_root: &Path,
    source: &Path,
    destination: &Path,
) -> Result<()> {
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
        copy_directory_entry(source_root, &source_path, &destination_path, file_type)?;
    }

    Ok(())
}

fn copy_directory_entry(
    source_root: &Path,
    source_path: &Path,
    destination_path: &Path,
    file_type: fs::FileType,
) -> Result<()> {
    if file_type.is_dir() {
        create_codex_home_if_missing(destination_path)?;
        return copy_directory_contents_under_root(source_root, source_path, destination_path);
    }

    if file_type.is_file() {
        copy_shared_codex_file_replacing_existing(source_path, destination_path, "failed to copy")?;
        return Ok(());
    }

    if file_type.is_symlink() {
        return copy_symlinked_file_under_root(source_root, source_path, destination_path);
    }

    Ok(())
}

fn copy_symlinked_file_under_root(
    source_root: &Path,
    source_path: &Path,
    destination_path: &Path,
) -> Result<()> {
    let source_root = fs::canonicalize(source_root)
        .with_context(|| format!("failed to resolve {}", source_root.display()))?;
    let Ok(target) = fs::canonicalize(source_path) else {
        return Ok(());
    };
    if !target.starts_with(&source_root) {
        return Ok(());
    }
    copy_shared_codex_file_replacing_existing(&target, destination_path, "failed to copy")
}

#[cfg(test)]
#[path = "../tests/src/copy.rs"]
mod tests;
