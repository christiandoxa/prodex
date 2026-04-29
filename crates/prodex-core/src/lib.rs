use anyhow::{Context, Result};
use dirs::home_dir;
use std::env;
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
