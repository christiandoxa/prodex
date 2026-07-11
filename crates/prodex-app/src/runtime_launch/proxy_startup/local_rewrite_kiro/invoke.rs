use super::RuntimeKiroProfileAuth;
use crate::profile_commands::{read_kiro_auth_secret, write_kiro_cli_data_dir};
use crate::runtime_kiro_acp::{
    RuntimeKiroAcpPromptTurnResult, runtime_kiro_acp_prompt_turn_with_command,
};
use anyhow::Result;
use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime as TokioRuntime;

pub(super) fn runtime_kiro_prompt_turn(
    auth: &RuntimeKiroProfileAuth,
    temp_name: &str,
    prompt: &str,
    async_runtime: &Arc<TokioRuntime>,
) -> Result<RuntimeKiroAcpPromptTurnResult> {
    let overlay_root = runtime_kiro_temp_dir(temp_name);
    let result = (|| {
        let secret = read_kiro_auth_secret(&auth.codex_home)?;
        let data_dir = overlay_root.join("kiro-data");
        write_kiro_cli_data_dir(&data_dir, &secret)?;
        let mut extra_env = vec![(OsString::from("Q_CLI_DATA_DIR"), data_dir.into_os_string())];
        if let Some(region) = secret
            .region
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            extra_env.push((OsString::from("AWS_REGION"), OsString::from(region)));
        }
        let cwd = env::current_dir().unwrap_or_else(|_| auth.codex_home.clone());
        let default_command = crate::kiro_bin();
        let command = auth
            .command
            .as_deref()
            .map(Path::as_os_str)
            .unwrap_or(default_command.as_os_str());
        runtime_kiro_acp_prompt_turn_with_command(command, &cwd, &extra_env, prompt)
    })();
    super::schedule_runtime_kiro_overlay_cleanup(async_runtime, overlay_root);
    result
}

pub(super) fn runtime_kiro_temp_dir(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!("prodex-kiro-{name}-{}-{stamp}", std::process::id()))
}
