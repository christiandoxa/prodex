use anyhow::{Context, Result, bail};
use std::fs;
use std::io;
use std::path::Path;

pub(crate) fn localize_text_file(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let contents = read_optional_link_target_contents(path)?;
            fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
            fs::write(path, contents)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        Ok(metadata) if metadata.is_dir() => {
            bail!("{} is a directory, expected a file", path.display());
        }
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            fs::write(path, "").with_context(|| format!("failed to write {}", path.display()))?;
        }
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
        }
    }
    Ok(())
}

fn read_optional_link_target_contents(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}
