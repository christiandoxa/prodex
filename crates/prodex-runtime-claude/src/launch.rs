use crate::constants::PRODEX_CLAUDE_PROXY_API_KEY;
use crate::paths::runtime_proxy_claude_config_value;
use std::ffi::OsString;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeProxyClaudeLaunchModes {
    pub caveman_mode: bool,
}

pub fn runtime_proxy_claude_extract_launch_modes(
    claude_args: &[OsString],
) -> (RuntimeProxyClaudeLaunchModes, Vec<OsString>) {
    let mut launch_modes = RuntimeProxyClaudeLaunchModes::default();
    let mut prefix_len = 0;
    while let Some(arg) = claude_args.get(prefix_len).and_then(|value| value.to_str()) {
        match arg {
            "caveman" => launch_modes.caveman_mode = true,
            _ => break,
        }
        prefix_len += 1;
    }
    (launch_modes, claude_args[prefix_len..].to_vec())
}

pub fn runtime_proxy_claude_launch_args(
    claude_args: &[OsString],
    plugin_dirs: &[PathBuf],
) -> Vec<OsString> {
    let mut args = Vec::with_capacity(claude_args.len() + plugin_dirs.len() * 2);
    for plugin_dir in plugin_dirs {
        args.push(OsString::from("--plugin-dir"));
        args.push(plugin_dir.as_os_str().to_os_string());
    }
    args.extend(claude_args.iter().cloned());
    args
}

pub fn runtime_proxy_claude_launch_model(
    codex_home: &Path,
) -> codex_config::CodexConfigResult<String> {
    if let Some(model) = runtime_anthropic_crate::runtime_proxy_claude_model_override() {
        return Ok(model);
    }
    Ok(runtime_proxy_claude_config_value(codex_home, "model")?
        .unwrap_or_else(|| runtime_anthropic_crate::DEFAULT_PRODEX_CLAUDE_MODEL.to_string()))
}

pub fn runtime_proxy_claude_launch_env(
    listen_addr: SocketAddr,
    config_dir: &Path,
    codex_home: &Path,
) -> codex_config::CodexConfigResult<Vec<(&'static str, OsString)>> {
    let target_model = runtime_proxy_claude_launch_model(codex_home)?;
    let base_url = format!("http://{listen_addr}");
    let mut env = vec![
        ("CLAUDE_CONFIG_DIR", OsString::from(config_dir.as_os_str())),
        ("ANTHROPIC_BASE_URL", OsString::from(base_url.as_str())),
        (
            "ANTHROPIC_AUTH_TOKEN",
            OsString::from(PRODEX_CLAUDE_PROXY_API_KEY),
        ),
        (
            "ANTHROPIC_MODEL",
            OsString::from(runtime_anthropic_crate::runtime_proxy_claude_picker_model(
                &target_model,
            )),
        ),
    ];
    if runtime_anthropic_crate::runtime_proxy_claude_use_foundry_compat() {
        env.push(("CLAUDE_CODE_USE_FOUNDRY", OsString::from("1")));
        env.push(("ANTHROPIC_FOUNDRY_BASE_URL", OsString::from(base_url)));
        env.push((
            "ANTHROPIC_FOUNDRY_API_KEY",
            OsString::from(PRODEX_CLAUDE_PROXY_API_KEY),
        ));
    }
    env.extend(prodex_runtime_launch::local_proxy_bypass_env());
    env.extend(runtime_anthropic_crate::runtime_proxy_claude_pinned_alias_env());
    env.extend(
        runtime_anthropic_crate::runtime_proxy_claude_custom_model_option_env(&target_model),
    );
    Ok(env)
}

pub fn runtime_proxy_claude_removed_env() -> &'static [&'static str] {
    &[
        "ANTHROPIC_API_KEY",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
        "CLAUDE_CODE_USE_ANTHROPIC_AWS",
        "ANTHROPIC_BEDROCK_BASE_URL",
        "ANTHROPIC_VERTEX_BASE_URL",
        "ANTHROPIC_FOUNDRY_BASE_URL",
        "ANTHROPIC_AWS_BASE_URL",
        "ANTHROPIC_FOUNDRY_RESOURCE",
        "ANTHROPIC_VERTEX_PROJECT_ID",
        "ANTHROPIC_AWS_WORKSPACE_ID",
        "CLOUD_ML_REGION",
        "ANTHROPIC_FOUNDRY_API_KEY",
        "ANTHROPIC_AWS_API_KEY",
        "CLAUDE_CODE_SKIP_BEDROCK_AUTH",
        "CLAUDE_CODE_SKIP_VERTEX_AUTH",
        "CLAUDE_CODE_SKIP_FOUNDRY_AUTH",
        "CLAUDE_CODE_SKIP_ANTHROPIC_AWS_AUTH",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
        "ANTHROPIC_DEFAULT_OPUS_MODEL_DESCRIPTION",
        "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
        "ANTHROPIC_DEFAULT_SONNET_MODEL_DESCRIPTION",
        "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL_DESCRIPTION",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES",
        "ANTHROPIC_CUSTOM_MODEL_OPTION",
        "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME",
        "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION",
    ]
}

pub fn runtime_proxy_claude_binary_version(binary: &OsString) -> Option<String> {
    let output = Command::new(binary).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_runtime_proxy_claude_version_text(&String::from_utf8_lossy(&output.stdout)).or_else(
        || parse_runtime_proxy_claude_version_text(&String::from_utf8_lossy(&output.stderr)),
    )
}

pub fn parse_runtime_proxy_claude_version_text(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| token.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .map(str::to_string)
}
