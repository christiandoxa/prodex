use anyhow::Result;
use prodex_core::AppPaths;
use std::path::{Path, PathBuf};

#[allow(unused_imports)]
pub(crate) use prodex_runtime_claude::{
    ensure_runtime_proxy_claude_profile_link, merge_runtime_proxy_claude_directory_contents,
    merge_runtime_proxy_claude_file, merge_runtime_proxy_claude_json_file,
    merge_runtime_proxy_claude_jsonl_file, merge_runtime_proxy_claude_symlink,
    migrate_runtime_proxy_claude_profile_dir_to_target, runtime_proxy_create_directory_symlink,
    runtime_proxy_create_symlink, runtime_proxy_merge_json_defaults, runtime_proxy_remove_path,
    runtime_proxy_resolve_symlink_target,
};

pub(crate) fn prepare_runtime_proxy_claude_config_dir(
    paths: &AppPaths,
    codex_home: &Path,
    managed: bool,
) -> Result<PathBuf> {
    prodex_runtime_claude::prepare_runtime_proxy_claude_config_dir(&paths.root, codex_home, managed)
}
