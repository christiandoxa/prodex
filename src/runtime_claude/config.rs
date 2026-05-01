use super::*;

#[allow(unused_imports)]
pub(crate) use prodex_runtime_claude::{
    PRODEX_CLAUDE_PROXY_API_KEY, RuntimeProxyClaudeLaunchModes,
    ensure_runtime_proxy_claude_launch_config, ensure_runtime_proxy_claude_settings,
    legacy_default_claude_config_dir, legacy_default_claude_config_path,
    parse_runtime_proxy_claude_version_text, runtime_proxy_claude_binary_version,
    runtime_proxy_claude_config_dir, runtime_proxy_claude_config_path,
    runtime_proxy_claude_config_value, runtime_proxy_claude_extract_launch_modes,
    runtime_proxy_claude_launch_args, runtime_proxy_claude_launch_env,
    runtime_proxy_claude_launch_model, runtime_proxy_claude_legacy_import_marker_path,
    runtime_proxy_claude_removed_env, runtime_proxy_claude_settings_path,
};

pub(crate) fn runtime_proxy_shared_claude_config_dir(paths: &AppPaths) -> PathBuf {
    prodex_runtime_claude::runtime_proxy_shared_claude_config_dir(&paths.root)
}
