pub(crate) use prodex_runtime_claude::{
    RuntimeProxyClaudeLaunchModes, ensure_runtime_proxy_claude_launch_config,
    parse_runtime_proxy_claude_version_text, runtime_proxy_claude_extract_launch_modes,
    runtime_proxy_claude_launch_args, runtime_proxy_claude_launch_env,
    runtime_proxy_claude_removed_env,
};

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use prodex_runtime_claude::{
    PRODEX_CLAUDE_PROXY_API_KEY, ensure_runtime_proxy_claude_settings,
    legacy_default_claude_config_dir, legacy_default_claude_config_path,
    runtime_proxy_claude_config_dir, runtime_proxy_claude_config_path,
    runtime_proxy_claude_config_value, runtime_proxy_claude_launch_model,
    runtime_proxy_claude_legacy_import_marker_path, runtime_proxy_claude_settings_path,
};

pub(super) fn runtime_proxy_claude_binary_version(binary: &std::ffi::OsStr) -> Option<String> {
    let mut command = std::process::Command::new(binary);
    command.arg("--version");
    let output = crate::command_probe_output(&mut command, "Claude version probe").ok()?;
    if !output.status.success() {
        return None;
    }
    parse_runtime_proxy_claude_version_text(&String::from_utf8_lossy(&output.stdout)).or_else(
        || parse_runtime_proxy_claude_version_text(&String::from_utf8_lossy(&output.stderr)),
    )
}

#[cfg(test)]
pub(crate) fn runtime_proxy_shared_claude_config_dir(
    paths: &crate::AppPaths,
) -> std::path::PathBuf {
    prodex_runtime_claude::runtime_proxy_shared_claude_config_dir(&paths.root)
}
