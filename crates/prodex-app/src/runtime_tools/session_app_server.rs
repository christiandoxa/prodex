use super::{ChildProcessPlan, RuntimeToolLaunchStrategy};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub(super) fn build_session_app_server_companion(
    strategy: &RuntimeToolLaunchStrategy,
    overlay_home: &Path,
    runtime_args: &[std::ffi::OsString],
) -> Result<Option<(ChildProcessPlan, PathBuf)>> {
    if !strategy.args.super_mode
        || strategy.desktop_command.is_some()
        || prodex_runtime_launch::is_codex_exec_invocation(runtime_args)
        || prodex_runtime_launch::codex_resume_requested(runtime_args)
        || runtime_args.iter().any(|arg| arg == "--remote")
    {
        return Ok(None);
    }
    let socket = overlay_home.join(".s");
    let mut args = vec![
        std::ffi::OsString::from("app-server"),
        std::ffi::OsString::from("--listen"),
        std::ffi::OsString::from(format!("unix://{}", socket.display())),
    ];
    let mut index = 0;
    while index < runtime_args.len() {
        let argument = runtime_args[index].to_string_lossy();
        let next = runtime_args.get(index + 1).map(|value| value.to_owned());
        match argument.as_ref() {
            "-c" | "--config" | "--enable" | "--disable" | "--code-mode-host" => {
                if let Some(value) = next {
                    args.extend([runtime_args[index].clone(), value]);
                    index += 2;
                    continue;
                }
            }
            "--strict-config" => args.push(runtime_args[index].clone()),
            "-m" | "--model" => {
                if let Some(value) = next.and_then(|value| value.into_string().ok()) {
                    args.extend([
                        "-c".into(),
                        format!(
                            "model={}",
                            crate::runtime_catalog_config::toml_string_literal(&value)
                        )
                        .into(),
                    ]);
                    index += 2;
                    continue;
                }
            }
            value if value.starts_with("--model=") => {
                args.extend([
                    "-c".into(),
                    format!(
                        "model={}",
                        crate::runtime_catalog_config::toml_string_literal(
                            value.trim_start_matches("--model=")
                        )
                    )
                    .into(),
                ]);
            }
            _ => {}
        }
        index += 1;
    }
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
