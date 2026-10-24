use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use prodex_mojo_core::optional_tools_policy::optional_tool_newest_stable_version;

use crate::PRODEX_OPTIMIZERS_HOME_ENV;

const PRODEX_OPTIMIZERS_DIR_NAME: &str = "prodex-optimizers";

pub(crate) fn path_dirs_from_env() -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default()
}

pub(crate) fn managed_optimizer_roots() -> Vec<PathBuf> {
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

pub(crate) fn newest_stable_managed_version(
    root: &Path,
    tool: &str,
) -> Result<Option<(String, PathBuf)>> {
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", root.display()));
        }
    };
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "optional-tool root {} must be a real directory",
        root.display()
    );
    newest_stable_version_directory(&root.join(tool))
}

fn newest_stable_version_directory(tool_root: &Path) -> Result<Option<(String, PathBuf)>> {
    let entries = match fs::read_dir(tool_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", tool_root.display()));
        }
    };
    let mut candidates = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", tool_root.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        let Some(candidate) = managed_version_directory_entry(entry.path(), file_type) else {
            continue;
        };
        candidates.push(candidate);
    }
    let names = candidates
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    let selected = optional_tool_newest_stable_version(&names)
        .map_err(|error| anyhow::anyhow!("failed to select managed tool version: {error:?}"))?;
    Ok(selected.map(|index| candidates.swap_remove(index)))
}

fn managed_version_directory_entry(
    path: PathBuf,
    file_type: fs::FileType,
) -> Option<(String, PathBuf)> {
    if !file_type.is_dir() || file_type.is_symlink() {
        return None;
    }
    let name = path.file_name()?.to_str()?.to_owned();
    Some((name, path))
}

pub(crate) fn managed_optimizer_command_candidates(root: &Path, command: &str) -> Vec<PathBuf> {
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

pub(crate) fn home_dir_from_env() -> Option<PathBuf> {
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

#[cfg(test)]
mod tests {
    #[test]
    fn managed_version_directory_rejects_non_unicode_names() {
        #[cfg(unix)]
        let name = {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(b"9.0.0\xff".to_vec())
        };
        #[cfg(windows)]
        let name = {
            use std::os::windows::ffi::OsStringExt;
            std::ffi::OsString::from_wide(&[b'9' as u16, b'.' as u16, 0xd800])
        };
        let file_type = std::fs::metadata(std::env::temp_dir()).unwrap().file_type();
        // Some filesystems reject these names before discovery can inspect them.
        assert_eq!(
            super::managed_version_directory_entry(name.into(), file_type),
            None
        );
    }
}
