// Test-only original wrapper text, frozen before Mojo promotion.
use std::path::Path;

fn original_posix_command(
    rtk: &Path,
    command: &Path,
    subcommands: &[&str],
    state_paths: &[&str],
) -> String {
    let rtk = shell_single_quote(&rtk.display().to_string());
    let command = shell_single_quote(&command.display().to_string());
    let rtk_env = original_state_exports(state_paths, false);
    let routing = if subcommands.is_empty() {
        format!("export PRODEX_RTK_AUTO_WRAP_DEPTH=1\n{rtk_env}exec {rtk} {command} \"$@\"\n")
    } else {
        let pattern = subcommands
            .iter()
            .map(|subcommand| subcommand.replace('|', "\\|"))
            .collect::<Vec<_>>()
            .join("|");
        format!(
            "for prodex_rtk_arg in \"$@\"; do\n  case \"$prodex_rtk_arg\" in\n    {pattern})\n      export PRODEX_RTK_AUTO_WRAP_DEPTH=1\n      {rtk_env}exec {rtk} {command} \"$@\"\n      ;;\n  esac\ndone\nexec {command} \"$@\"\n",
        )
    };
    format!(
        "#!/usr/bin/env sh\nif [ \"${{PRODEX_RTK_DISABLE_AUTO_WRAP:-}}\" = \"1\" ] || [ -n \"${{PRODEX_RTK_AUTO_WRAP_DEPTH:-}}\" ]; then\n  exec {command} \"$@\"\nfi\n{routing}",
    )
}

fn original_windows_command(
    rtk: &Path,
    command: &Path,
    subcommands: &[&str],
    state_paths: &[&str],
) -> String {
    let rtk = cmd_double_quote(&rtk.display().to_string());
    let command = cmd_double_quote(&command.display().to_string());
    let rtk_env = original_state_exports(state_paths, true);
    let routing = if subcommands.is_empty() {
        format!(
            "set \"PRODEX_RTK_AUTO_WRAP_DEPTH=1\"\r\n{rtk_env}{rtk} {command} %*\r\nexit /b %ERRORLEVEL%\r\n"
        )
    } else {
        let checks = subcommands
            .iter()
            .map(|subcommand| {
                format!(
                    "if /I \"%~1\"==\"{}\" goto prodex_rtk_wrap\r\n",
                    cmd_escape(subcommand)
                )
            })
            .collect::<String>();
        format!(
            ":prodex_rtk_scan\r\nif \"%~1\"==\"\" goto prodex_rtk_raw\r\n{checks}shift\r\ngoto prodex_rtk_scan\r\n:prodex_rtk_wrap\r\nset \"PRODEX_RTK_AUTO_WRAP_DEPTH=1\"\r\n{rtk_env}{rtk} {command} %*\r\nexit /b %ERRORLEVEL%\r\n"
        )
    };
    format!(
        "@echo off\r\nif \"%PRODEX_RTK_DISABLE_AUTO_WRAP%\"==\"1\" goto prodex_rtk_raw\r\nif defined PRODEX_RTK_AUTO_WRAP_DEPTH goto prodex_rtk_raw\r\n{routing}:prodex_rtk_raw\r\n{command} %*\r\nexit /b %ERRORLEVEL%\r\n"
    )
}

fn original_posix_shell(command: &Path, args: &[&str], state_paths: &[&str]) -> String {
    let args = args
        .iter()
        .map(|arg| format!(" {}", shell_single_quote(arg)))
        .collect::<String>();
    let rtk_env = original_state_exports(state_paths, false);
    format!(
        "#!/usr/bin/env sh\n{}exec {}{} \"$@\"\n",
        rtk_env,
        shell_single_quote(&command.display().to_string()),
        args
    )
}

fn original_windows_shell(command: &Path, args: &[&str], state_paths: &[&str]) -> String {
    let args = args
        .iter()
        .map(|arg| format!(" {}", cmd_double_quote(arg)))
        .collect::<String>();
    format!(
        "@echo off\r\n{}{}{} %*\r\nexit /b %ERRORLEVEL%\r\n",
        original_state_exports(state_paths, true),
        cmd_double_quote(&command.display().to_string()),
        args
    )
}

fn original_posix_unavailable(command: &str) -> String {
    let message = shell_single_quote(&format!(
        "prodex: {command} requested but '{command}' is not installed or not on PATH"
    ));
    format!("#!/usr/bin/env sh\necho {message} >&2\nexit 127\n")
}

fn original_windows_unavailable(command: &str) -> String {
    let message = cmd_escape(&format!(
        "prodex: {command} requested but '{command}' is not installed or not on PATH"
    ));
    format!("@echo off\r\n>&2 echo {message}\r\nexit /b 127\r\n")
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn cmd_escape(value: &str) -> String {
    value.replace('%', "%%")
}

fn cmd_double_quote(value: &str) -> String {
    format!("\"{}\"", cmd_escape(value))
}

fn original_state_exports(paths: &[&str], windows: bool) -> String {
    ["RTK_DB_PATH", "RTK_TEE_DIR", "RTK_AUDIT_DIR"]
        .iter()
        .zip(paths)
        .map(|(key, value)| {
            if windows {
                format!("set \"{key}={}\"\r\n", cmd_escape(value))
            } else {
                format!("export {key}={}\n", shell_single_quote(value))
            }
        })
        .collect()
}

pub(super) fn expected(input: &super::super::RtkWrapperInput<'_>) -> String {
    use super::super::{RtkWrapperKind, RtkWrapperPlatform};
    let rtk = Path::new(input.rtk);
    let command = Path::new(input.command);
    match (input.platform, input.kind) {
        (RtkWrapperPlatform::Posix, RtkWrapperKind::Command) => {
            original_posix_command(rtk, command, input.arguments, input.state_paths)
        }
        (RtkWrapperPlatform::Windows, RtkWrapperKind::Command) => {
            original_windows_command(rtk, command, input.arguments, input.state_paths)
        }
        (RtkWrapperPlatform::Posix, RtkWrapperKind::Shell) => {
            original_posix_shell(command, input.arguments, input.state_paths)
        }
        (RtkWrapperPlatform::Windows, RtkWrapperKind::Shell) => {
            original_windows_shell(command, input.arguments, input.state_paths)
        }
        (RtkWrapperPlatform::Posix, RtkWrapperKind::Unavailable) => {
            original_posix_unavailable(input.command)
        }
        (RtkWrapperPlatform::Windows, RtkWrapperKind::Unavailable) => {
            original_windows_unavailable(input.command)
        }
    }
}
