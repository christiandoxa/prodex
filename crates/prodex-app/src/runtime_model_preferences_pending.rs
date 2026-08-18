use super::{AppPaths, LastModelSelection, record_model_preference};
use anyhow::{Context, Result};
use std::fs;

const MODEL_PREFERENCE_PENDING_FILE: &str = "model-preferences-pending.json";

fn path(paths: &AppPaths) -> std::path::PathBuf {
    paths.root.join(MODEL_PREFERENCE_PENDING_FILE)
}

pub(super) fn save_pending_model_preference(
    paths: &AppPaths,
    selection: &LastModelSelection,
) -> Result<()> {
    let contents =
        serde_json::to_vec(selection).context("failed to encode pending model preference")?;
    crate::runtime_store::write_private_file_atomic(&path(paths), &contents)
}

pub(super) fn flush_pending_model_preference(paths: &AppPaths) -> Result<()> {
    let path = path(paths);
    let contents = match fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let selection: LastModelSelection =
        serde_json::from_slice(&contents).context("failed to decode pending model preference")?;
    record_model_preference(paths, selection)?;
    fs::remove_file(path).context("failed to clear pending model preference")
}
