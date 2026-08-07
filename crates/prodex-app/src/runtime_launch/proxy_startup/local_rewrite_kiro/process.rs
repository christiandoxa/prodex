use std::path::Path;
use std::process::{Child, Command};

pub(super) fn runtime_kiro_streaming_command(
    command: &Path,
    model: Option<&str>,
    effort: Option<&str>,
) -> Command {
    let mut acp_command = Command::new(command);
    let model = model
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or(prodex_provider_core::PRODEX_KIRO_DEFAULT_MODEL);
    acp_command.arg("acp").arg("--model").arg(model);
    if let Some(effort) = effort.map(str::trim).filter(|effort| !effort.is_empty()) {
        acp_command.arg("--effort").arg(effort);
    }
    acp_command
}

#[cfg(unix)]
pub(super) fn runtime_kiro_configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
pub(super) fn runtime_kiro_configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
pub(super) fn runtime_kiro_kill_process_group(child: &Child) {
    let pid = child.id() as libc::pid_t;
    if pid > 0 {
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
}

#[cfg(not(unix))]
pub(super) fn runtime_kiro_kill_process_group(_child: &Child) {}
