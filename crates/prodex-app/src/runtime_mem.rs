use anyhow::{Context, Result};
use dirs::home_dir;
use prodex_core::AppPaths;
use std::env;
use std::path::{Path, PathBuf};

#[allow(unused_imports)]
pub(crate) use prodex_runtime_mem::{
    RuntimeMemTranscriptMode, ensure_runtime_mem_codex_watch_for_home,
    ensure_runtime_mem_codex_watch_for_home_at_path,
    ensure_runtime_mem_codex_watch_for_home_at_path_with_mode,
    ensure_runtime_mem_codex_watch_for_home_with_mode,
    ensure_runtime_mem_codex_watch_for_sessions_root,
    ensure_runtime_mem_codex_watch_for_sessions_root_with_mode, runtime_mem_claude_plugin_dir,
    runtime_mem_claude_plugin_dir_from_home, runtime_mem_claude_plugin_manifest_path,
    runtime_mem_data_dir_from_home, runtime_mem_default_codex_schema, runtime_mem_extract_mode,
    runtime_mem_extract_mode_with_detail, runtime_mem_full_codex_schema,
    runtime_mem_settings_path_from_home, runtime_mem_super_default_transcript_mode,
    runtime_mem_transcript_watch_config_path_from_home,
};

pub(super) fn ensure_runtime_mem_prodex_observer(paths: &AppPaths) -> Result<PathBuf> {
    let home = home_dir().context("failed to determine home directory for claude-mem")?;
    let prodex_exe = env::current_exe().context("failed to determine current prodex executable")?;
    ensure_runtime_mem_prodex_observer_for_home(&home, paths, &prodex_exe)
}

pub(super) fn ensure_runtime_mem_prodex_observer_for_home(
    home: &Path,
    paths: &AppPaths,
    prodex_exe: &Path,
) -> Result<PathBuf> {
    prodex_runtime_mem::ensure_runtime_mem_prodex_observer_for_home_and_root(
        home,
        &paths.root,
        prodex_exe,
    )
}

#[cfg(test)]
pub(super) fn runtime_mem_prodex_claude_wrapper_path(paths: &AppPaths) -> PathBuf {
    prodex_runtime_mem::runtime_mem_prodex_claude_wrapper_path_from_root(&paths.root)
}
