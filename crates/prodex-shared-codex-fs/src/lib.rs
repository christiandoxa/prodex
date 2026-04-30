use anyhow::{Context, Result, bail};
use prodex_core::{AppPaths, prodex_previous_default_shared_codex_root};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

mod copy;
mod entries;
mod history;
mod migration;
mod ops;
mod prepare;

pub use self::copy::copy_codex_home;
use self::copy::*;
pub use self::entries::{SharedCodexEntry, SharedCodexEntryKind};
use self::history::*;
use self::migration::*;
pub use self::ops::create_codex_home_if_missing;
use self::ops::*;
pub use self::prepare::prepare_managed_codex_home;

const SHARED_CODEX_DIR_NAMES: &[&str] = &[
    "sessions",
    "archived_sessions",
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
    "AGENTS.md",
    "AGENTS.override.md",
    ".tmp/plugins.sha",
    ".tmp/known_marketplaces.json",
    ".tmp/app-server-remote-plugin-sync-v1",
];

const SHARED_CODEX_SQLITE_PREFIXES: &[&str] = &["state_", "logs_"];
const SHARED_CODEX_SQLITE_SUFFIXES: &[&str] = &[".sqlite", ".sqlite-shm", ".sqlite-wal"];

fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path_for_compare(left) == normalize_path_for_compare(right)
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
