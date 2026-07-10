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
use self::ops::*;
pub use self::ops::{create_codex_home_if_missing, ensure_managed_profiles_root};
pub use self::prepare::{
    maintain_managed_codex_sessions, prepare_managed_codex_home,
    prepare_managed_codex_home_for_runtime_launch,
};

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
const CODEX_SESSION_MAINTENANCE_LOCK_FILE: &str = ".prodex-maintenance.lock";

fn open_codex_session_maintenance_lock(codex_home: &Path) -> Result<fs::File> {
    let sessions_dir = codex_home.join("sessions");
    fs::create_dir_all(&sessions_dir)
        .with_context(|| format!("failed to create {}", sessions_dir.display()))?;
    let lock_path = sessions_dir.join(CODEX_SESSION_MAINTENANCE_LOCK_FILE);
    fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))
}

pub fn lock_codex_sessions_for_child(codex_home: &Path) -> Result<fs::File> {
    let lock = open_codex_session_maintenance_lock(codex_home)?;
    lock.lock_shared()
        .with_context(|| "failed to lock Codex sessions for child process")?;
    Ok(lock)
}

fn try_lock_codex_session_maintenance(codex_home: &Path) -> Result<Option<fs::File>> {
    let lock = open_codex_session_maintenance_lock(codex_home)?;
    match lock.try_lock() {
        Ok(()) => Ok(Some(lock)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(err) => Err(err).context("failed to lock Codex sessions for maintenance"),
    }
}

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
