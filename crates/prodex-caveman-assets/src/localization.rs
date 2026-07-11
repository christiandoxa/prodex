use anyhow::{Context, Result, bail};
use std::fs;
use std::io;
use std::path::Path;

use crate::fs_ops::{read_canonical_text_file_limited, write_text_file};

pub(crate) fn localize_text_file(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let contents = read_optional_link_target_contents(path)?;
            fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
            write_text_file(path, &contents)?;
        }
        Ok(metadata) if metadata.is_dir() => {
            bail!("{} is a directory, expected a file", path.display());
        }
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            write_text_file(path, "")?;
        }
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
        }
    }
    Ok(())
}

fn read_optional_link_target_contents(path: &Path) -> Result<String> {
    read_canonical_text_file_limited(path)
        .map(|contents| contents.unwrap_or_default())
        .with_context(|| format!("failed to read {}", path.display()))
}
