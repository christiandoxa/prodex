use std::env;
use std::path::{Path, PathBuf};

use super::PRODEX_OPTIMIZERS_HOME_ENV;

const PRODEX_OPTIMIZERS_DIR_NAME: &str = "prodex-optimizers";

pub(super) fn path_dirs_from_env() -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default()
}

pub(super) fn managed_optimizer_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(path) = env::var_os(PRODEX_OPTIMIZERS_HOME_ENV) {
        push_unique_path(&mut roots, PathBuf::from(path));
    }
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        push_unique_path(
            &mut roots,
            PathBuf::from(path).join(PRODEX_OPTIMIZERS_DIR_NAME),
        );
    }
    if let Some(home) = home_dir_from_env() {
        push_unique_path(
            &mut roots,
            home.join(".local")
                .join("share")
                .join(PRODEX_OPTIMIZERS_DIR_NAME),
        );
    }
    roots
}

pub(super) fn managed_optimizer_command_candidates(root: &Path, command: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    push_command_candidate(&mut candidates, root.join(command));
    if command == "codebase-memory-mcp" {
        let checkout = root.join("codebase-memory-mcp");
        push_command_candidate(&mut candidates, checkout.join(command));
        push_command_candidate(
            &mut candidates,
            checkout.join("build").join("c").join(command),
        );
        push_command_candidate(&mut candidates, checkout.join("bin").join(command));
    }
    candidates
}

pub(super) fn home_dir_from_env() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn push_command_candidate(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    candidates.push(path.clone());
    #[cfg(windows)]
    if path.extension().is_none() {
        if let Some(file_name) = path.file_name() {
            let mut exe_name = file_name.to_os_string();
            exe_name.push(".exe");
            candidates.push(path.with_file_name(exe_name));
        }
    }
}
