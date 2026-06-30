use anyhow::{Context, Result, bail};
use prodex_core::{AppPaths, prodex_previous_default_shared_codex_root};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

mod copy;
mod entries;
mod history;
mod image_attachments;
mod migration;
mod ops;
mod prepare;

pub use self::copy::copy_codex_home;
use self::copy::*;
pub use self::entries::{SharedCodexEntry, SharedCodexEntryKind};
use self::history::*;
pub use self::image_attachments::persist_codex_session_image_attachments;
use self::migration::*;
pub use self::ops::create_codex_home_if_missing;
use self::ops::*;
pub use self::prepare::{maintain_managed_codex_sessions, prepare_managed_codex_home};

const SHARED_CODEX_DIR_NAMES: &[&str] = &[
    "sessions",
    "archived_sessions",
    "attachments",
    "image_attachments",
    "shell_snapshots",
    "memories",
    "memories_extensions",
    "rules",
    "skills",
    "agents",
    "plugins",
    ".tmp/plugins",
    ".tmp/marketplaces",
];

const SHARED_CODEX_FILE_NAMES: &[&str] = &[
    "history.jsonl",
    "config.toml",
    "managed_config.toml",
    "environments.toml",
    "AGENTS.md",
    "AGENTS.override.md",
    ".credentials.json",
    "goals_1.sqlite",
    "goals_1.sqlite-shm",
    "goals_1.sqlite-wal",
    ".tmp/plugins.sha",
    ".tmp/known_marketplaces.json",
    ".tmp/app-server-remote-plugin-sync-v1",
];

const SHARED_CODEX_SQLITE_PREFIXES: &[&str] = &["state_", "logs_", "goals_", "memories_"];
const SHARED_CODEX_SQLITE_SUFFIXES: &[&str] = &[".sqlite", ".sqlite-shm", ".sqlite-wal"];
const SHARED_CODEX_PROFILE_V2_CONFIG_SUFFIX: &str = ".config.toml";

fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path_for_compare(left) == normalize_path_for_compare(right)
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn persist_login_home(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        bail!(
            "refusing to overwrite existing login destination {}",
            destination.display()
        );
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_codex_home(source, destination)?;
            remove_dir_if_exists(source)
        }
    }
}

pub fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_dir_all(path).with_context(|| format!("failed to delete {}", path.display()))
}
