use anyhow::{Context, Result};
use prodex_core::AppPaths;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const SESSION_FOLLOW_LIMIT: usize = 32;

pub(crate) fn recent_session_log_paths() -> Result<Vec<PathBuf>> {
    let sessions_root = AppPaths::discover()?.shared_codex_root.join("sessions");
    let mut paths = Vec::new();
    collect_session_log_paths(&sessions_root, &mut paths)?;
    paths.sort_by(|left, right| {
        let left_modified = path_modified_key(left);
        let right_modified = path_modified_key(right);
        right_modified
            .cmp(&left_modified)
            .then_with(|| left.cmp(right))
    });
    paths.truncate(SESSION_FOLLOW_LIMIT);
    Ok(paths)
}

fn collect_session_log_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", root.display())),
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read entry in {}", root.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_dir() {
            collect_session_log_paths(&path, paths)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn path_modified_key(path: &Path) -> u128 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
