use super::{
    AppPaths, LastModelSelection, MODEL_PREFERENCE_LOCK_WAIT, record_model_preference,
    try_acquire_model_preference_lock,
};
use anyhow::{Context, Result};
use std::fs;
use std::time::Instant;

const MODEL_PREFERENCE_PENDING_FILE: &str = "model-preferences-pending.json";

fn path(paths: &AppPaths) -> std::path::PathBuf {
    paths.root.join(MODEL_PREFERENCE_PENDING_FILE)
}

fn read(path: &std::path::Path) -> Result<Vec<LastModelSelection>> {
    let contents = match fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    if let Ok(selections) = serde_json::from_slice::<Vec<LastModelSelection>>(&contents) {
        return Ok(selections);
    }
    serde_json::from_slice(&contents)
        .map(|selection| vec![selection])
        .context("failed to decode pending model preference")
}

fn lock(path: &std::path::Path) -> Result<fs::File> {
    try_acquire_model_preference_lock(path, Instant::now() + MODEL_PREFERENCE_LOCK_WAIT)?
        .ok_or_else(|| anyhow::anyhow!("pending model preference file is busy"))
}

pub(super) fn save_pending_model_preference(
    paths: &AppPaths,
    selection: &LastModelSelection,
) -> Result<()> {
    let path = path(paths);
    let _lock = lock(&path)?;
    let mut selections = read(&path)?;
    if let Some(existing) = selections
        .iter()
        .find(|existing| existing.scope == selection.scope)
        && (existing.selected_at, existing.generation)
            > (selection.selected_at, selection.generation)
    {
        return Ok(());
    }
    selections.retain(|existing| existing.scope != selection.scope);
    selections.push(selection.clone());
    selections.sort_by(|left, right| {
        (&left.scope.provider, &left.scope.catalog)
            .cmp(&(&right.scope.provider, &right.scope.catalog))
    });
    let contents =
        serde_json::to_vec(&selections).context("failed to encode pending model preference")?;
    crate::runtime_store::write_private_file_atomic(&path, &contents)
}

pub(super) fn flush_pending_model_preference(paths: &AppPaths) -> Result<()> {
    let path = path(paths);
    if !path.exists() {
        return Ok(());
    }
    let _lock = lock(&path)?;
    let selections = read(&path)?;
    if selections.is_empty() {
        return Ok(());
    }
    for selection in selections {
        record_model_preference(paths, selection)?;
    }
    fs::remove_file(path).context("failed to clear pending model preference")
}
