use anyhow::{Context, Result};
use dirs::home_dir;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_PRODEX_DIR: &str = ".prodex";
pub const DEFAULT_CODEX_DIR: &str = ".codex";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub state_file: PathBuf,
    pub managed_profiles_root: PathBuf,
    pub shared_codex_root: PathBuf,
    pub legacy_shared_codex_root: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let root = match env::var_os("PRODEX_HOME") {
            Some(path) => absolutize(PathBuf::from(path))?,
            None => home_dir()
                .context("failed to determine home directory")?
                .join(DEFAULT_PRODEX_DIR),
        };

        Ok(Self {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: match env::var_os("PRODEX_SHARED_CODEX_HOME") {
                Some(path) => resolve_shared_codex_root(&root, PathBuf::from(path)),
                None => prodex_default_shared_codex_root(&root)?,
            },
            legacy_shared_codex_root: root.join("shared"),
            root,
        })
    }
}

pub fn absolutize(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    let current_dir = env::current_dir().context("failed to determine current directory")?;
    Ok(current_dir.join(path))
}

pub fn legacy_default_codex_home() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CODEX_DIR))
}

pub fn default_codex_home(paths: &AppPaths) -> Result<PathBuf> {
    let legacy = legacy_default_codex_home()?;
    Ok(select_default_codex_home(
        &paths.shared_codex_root,
        &legacy,
        env::var_os("PRODEX_SHARED_CODEX_HOME").is_some(),
    ))
}

pub fn select_default_codex_home(
    shared_codex_root: &Path,
    legacy_codex_home: &Path,
    override_active: bool,
) -> PathBuf {
    if override_active || shared_codex_root.exists() || !legacy_codex_home.exists() {
        shared_codex_root.to_path_buf()
    } else {
        legacy_codex_home.to_path_buf()
    }
}

pub fn prodex_default_shared_codex_root(_root: &Path) -> Result<PathBuf> {
    legacy_default_codex_home()
}

pub fn prodex_previous_default_shared_codex_root(root: &Path) -> PathBuf {
    root.join(DEFAULT_CODEX_DIR)
}

pub fn resolve_shared_codex_root(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

pub fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path_for_compare(left) == normalize_path_for_compare(right)
}

pub fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn format_binary_resolution(binary: &OsString) -> String {
    let configured = binary.to_string_lossy();
    match resolve_binary_path(binary) {
        Some(path) => format!("{configured} ({})", path.display()),
        None => format!("{configured} (not found)"),
    }
}

pub fn resolve_binary_path(binary: &OsString) -> Option<PathBuf> {
    let candidate = PathBuf::from(binary);
    if candidate.components().count() > 1 {
        if candidate.is_file() {
            return Some(fs::canonicalize(&candidate).unwrap_or(candidate));
        }
        return None;
    }

    let path_var = env::var_os("PATH")?;
    for directory in env::split_paths(&path_var) {
        let full_path = directory.join(&candidate);
        if full_path.is_file() {
            return Some(full_path);
        }
    }

    None
}
