use super::*;

pub(super) fn collect_install_check_rows(paths: &AppPaths) -> Vec<(String, String)> {
    let mut rows = vec![
        version_check_row("Codex CLI", codex_bin(), "--version"),
        (
            "Codex auth".to_string(),
            command_status(codex_bin(), &["login", "status"]),
        ),
        version_check_row("Claude Code", claude_bin(), "--version"),
        version_check_row("Gemini CLI", gemini_bin(), "--version"),
        version_check_row("GitHub Copilot CLI", copilot_bin(), "--version"),
        version_check_row("Kiro CLI", kiro_bin(), "--version"),
        version_check_row("Antigravity CLI", agy_bin(), "--version"),
        version_check_row("RTK", "rtk", "--version"),
        version_check_row("Node.js", "node", "--version"),
        version_check_row("npx", "npx", "--version"),
        probe_check_row("codebase-memory-mcp"),
    ];
    rows.push((
        "Caveman".to_string(),
        optional_tool_health_status(&prodex_optional_tools::optional_tool_status(
            prodex_optional_tools::OptionalToolId::Caveman,
        )),
    ));
    rows.push(("Prodex home".to_string(), paths.root.display().to_string()));
    rows.push((
        "Shared CODEX_HOME".to_string(),
        paths.shared_codex_root.display().to_string(),
    ));
    rows
}

fn optional_tool_health_status(health: &prodex_optional_tools::ToolHealth) -> String {
    let state = match health.status {
        prodex_optional_tools::ToolHealthStatus::Installed => "installed",
        prodex_optional_tools::ToolHealthStatus::Missing => "missing",
        prodex_optional_tools::ToolHealthStatus::Invalid => "invalid",
        prodex_optional_tools::ToolHealthStatus::Degraded => "degraded",
    };
    let mut fields = vec![state.to_string()];
    if let Some(version) = &health.version {
        fields.push(format!("version={version}"));
    }
    if let Some(path) = &health.path {
        fields.push(format!("path={}", path.display()));
    }
    if let Some(digest) = &health.digest {
        fields.push(format!("digest={digest}"));
    }
    redaction_redact_secret_like_text(&fields.join(", "))
}

fn command_capability_probe_args(command: &str) -> &'static [&'static str] {
    match command {
        "codebase-memory-mcp" => &["--help"],
        _ => &["--version"],
    }
}

fn command_version_status(command: impl AsRef<std::ffi::OsStr>, version_arg: &str) -> String {
    let command = command.as_ref();
    let mut probe = std::process::Command::new(command);
    probe.arg(version_arg);
    match crate::command_probe_output(&mut probe, &command.to_string_lossy()) {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let line = stdout
                .lines()
                .chain(stderr.lines())
                .find(|line| !line.trim().is_empty())
                .unwrap_or("available")
                .trim();
            format!("ok ({line})")
        }
        Ok(output) => format!("warn (exit {})", output.status),
        Err(error) => format!(
            "missing ({})",
            redaction_redact_secret_like_text(&error.to_string())
        ),
    }
}

fn command_probe_status(command: impl AsRef<std::ffi::OsStr>, args: &[&str]) -> String {
    let command = command.as_ref();
    let mut probe = std::process::Command::new(command);
    probe.args(args);
    match crate::command_probe_output(&mut probe, &command.to_string_lossy()) {
        Ok(output) if output.status.success() => "ok (available)".to_string(),
        Ok(output) => format!("warn (exit {})", output.status),
        Err(error) => format!(
            "missing ({})",
            redaction_redact_secret_like_text(&error.to_string())
        ),
    }
}

fn version_check_row(
    name: &str,
    command: impl AsRef<std::ffi::OsStr>,
    version_arg: &str,
) -> (String, String) {
    (
        name.to_string(),
        command_version_status(command, version_arg),
    )
}

fn probe_check_row(command: &'static str) -> (String, String) {
    (
        command.to_string(),
        command_probe_status(command, command_capability_probe_args(command)),
    )
}

fn command_status(command: impl AsRef<std::ffi::OsStr>, args: &[&str]) -> String {
    let command = command.as_ref();
    let mut probe = std::process::Command::new(command);
    probe.args(args);
    match crate::command_probe_output(&mut probe, &command.to_string_lossy()) {
        Ok(output) if output.status.success() => "ok".to_string(),
        Ok(output) => format!("warn (exit {})", output.status),
        Err(error) => format!(
            "missing ({})",
            redaction_redact_secret_like_text(&error.to_string())
        ),
    }
}
