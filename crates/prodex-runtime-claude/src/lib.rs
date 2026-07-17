mod constants;
mod launch;
mod launch_config;
mod paths;
mod state_merge;

pub use constants::{
    DEFAULT_CLAUDE_CONFIG_DIR_NAME, DEFAULT_CLAUDE_CONFIG_FILE_NAME,
    DEFAULT_CLAUDE_SETTINGS_FILE_NAME, PRODEX_CLAUDE_CONFIG_DIR_NAME,
    PRODEX_CLAUDE_DEFAULT_WEB_TOOLS, PRODEX_CLAUDE_LEGACY_IMPORT_MARKER_NAME,
    PRODEX_CLAUDE_PROXY_API_KEY, PRODEX_SHARED_CLAUDE_DIR_NAME,
};
pub use launch::{
    RuntimeProxyClaudeLaunchModes, parse_runtime_proxy_claude_version_text,
    runtime_proxy_claude_binary_version, runtime_proxy_claude_extract_launch_modes,
    runtime_proxy_claude_launch_args, runtime_proxy_claude_launch_env,
    runtime_proxy_claude_launch_model, runtime_proxy_claude_removed_env,
};
pub use launch_config::{
    ensure_runtime_proxy_claude_launch_config, ensure_runtime_proxy_claude_settings,
};
pub use paths::{
    legacy_default_claude_config_dir, legacy_default_claude_config_path,
    runtime_proxy_claude_config_dir, runtime_proxy_claude_config_path,
    runtime_proxy_claude_config_value, runtime_proxy_claude_legacy_import_marker_path,
    runtime_proxy_claude_settings_path, runtime_proxy_shared_claude_config_dir,
};
pub use state_merge::{
    RuntimeProxyClaudeMergeOutcome, ensure_runtime_proxy_claude_profile_link,
    maybe_import_runtime_proxy_claude_legacy_home, merge_runtime_proxy_claude_directory_contents,
    merge_runtime_proxy_claude_file, merge_runtime_proxy_claude_json_file,
    merge_runtime_proxy_claude_jsonl_file, merge_runtime_proxy_claude_symlink,
    migrate_runtime_proxy_claude_profile_dir_to_target, prepare_runtime_proxy_claude_config_dir,
    prepare_runtime_proxy_claude_import_target, runtime_proxy_create_directory_symlink,
    runtime_proxy_create_symlink, runtime_proxy_merge_json_defaults, runtime_proxy_remove_path,
    runtime_proxy_resolve_symlink_target,
};
