use super::{ChildProcessPlan, RuntimeToolLaunchStrategy};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub(super) fn session_app_server_socket(overlay_home: &Path) -> PathBuf {
    overlay_home.join(".s")
}

pub(super) fn build_session_app_server_companion(
    strategy: &RuntimeToolLaunchStrategy,
    overlay_home: &Path,
    runtime_args: &[std::ffi::OsString],
    legacy_hook_bypass: bool,
) -> Result<Option<(ChildProcessPlan, PathBuf)>> {
    if !super::session_app_server_companion_eligible(strategy, runtime_args) {
        return Ok(None);
    }
    let socket = session_app_server_socket(overlay_home);
    let mut args = Vec::with_capacity(runtime_args.len() + 4);
    if legacy_hook_bypass {
        args.push(std::ffi::OsString::from("--dangerously-bypass-hook-trust"));
    }
    args.extend([
        std::ffi::OsString::from("app-server"),
        std::ffi::OsString::from("--listen"),
        std::ffi::OsString::from(format!("unix://{}", socket.display())),
    ]);
    args.extend(super::project_app_server_config_args(runtime_args, false));
    if strategy.args.full_access {
        args.extend([
            "-c".into(),
            "approval_policy=\"never\"".into(),
            "-c".into(),
            "sandbox_mode=\"danger-full-access\"".into(),
        ]);
    }
    Ok(Some((
        super::codex_child_plan(overlay_home.to_path_buf(), args),
        socket,
    )))
}
