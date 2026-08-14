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

pub(super) fn runtime_kiro_configure_process_group(command: &mut Command) {
    crate::configure_child_process_group(command, runtime_kiro_owns_private_process_group());
}

pub(super) fn runtime_kiro_terminate_child(child: &mut Child) {
    let _ = crate::terminate_child_process_tree(child, runtime_kiro_owns_private_process_group());
}

fn runtime_kiro_owns_private_process_group() -> bool {
    std::env::var_os(crate::SUB_AGENT_RECURSION_MARKER).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn runtime_kiro_streaming_command_forwards_model_and_effort() {
        let command = runtime_kiro_streaming_command(
            Path::new("kiro-cli"),
            Some(" gpt-5.6-luna "),
            Some(" none "),
        );

        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![
                OsStr::new("acp"),
                OsStr::new("--model"),
                OsStr::new("gpt-5.6-luna"),
                OsStr::new("--effort"),
                OsStr::new("none"),
            ]
        );
    }
}
