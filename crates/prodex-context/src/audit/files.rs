use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::{is_compressible_context_file, is_context_backup};

pub(super) const CONTEXT_AUDIT_ROOTS: &[&str] = &[
    "AGENTS.md",
    "AGENTS.override.md",
    "memories",
    "memories_extensions",
    "rules",
    "skills",
];

pub(crate) fn collect_context_files(path: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_file() {
        paths.push(path.to_path_buf());
        return Ok(());
    }
    if !path.is_dir() {
        return Ok(());
    }

    let mut entries = fs::read_dir(path)
        .with_context(|| format!("failed to read context directory {}", path.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read entry in {}", path.display()))?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        collect_context_files(&entry.path(), paths)?;
    }
    Ok(())
}

pub(super) fn is_auditable_context_file(path: &Path) -> bool {
    !is_context_backup(path)
        && path.is_file()
        && (is_compressible_context_file(path)
            || matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("toml" | "json" | "yaml" | "yml")
            ))
}

pub(super) fn is_static_duplicate_context_file(path: &Path) -> bool {
    is_compressible_context_file(path)
}
