use crate::{claude_bin, codex_bin};
use anyhow::{Context, Result};
use prodex_cli::{CapabilityCommands, CapabilityListArgs, SetupArgs};
use prodex_core::AppPaths;
use std::fs;
use std::process::{Command, Stdio};
use terminal_ui::{print_blank_line, print_panel, print_stdout_line};

#[derive(Debug, Clone)]
struct ProdexCapability {
    name: &'static str,
    category: &'static str,
    status: String,
    command: Option<&'static str>,
    description: String,
}

pub(crate) fn handle_capability(command: CapabilityCommands) -> Result<()> {
    match command {
        CapabilityCommands::List(args) => handle_capability_list(args),
    }
}

fn handle_capability_list(args: CapabilityListArgs) -> Result<()> {
    let capabilities = collect_capabilities();
    if args.json {
        let rows = capabilities
            .iter()
            .map(|capability| {
                serde_json::json!({
                    "name": capability.name,
                    "category": capability.category,
                    "status": capability.status,
                    "command": capability.command,
                    "description": capability.description,
                })
            })
            .collect::<Vec<_>>();
        print_stdout_line(
            &serde_json::to_string_pretty(&rows).context("failed to serialize capability list")?,
        );
        return Ok(());
    }

    let fields = capabilities
        .iter()
        .map(|capability| {
            (
                capability.name.to_string(),
                format!(
                    "{}; {}; {}",
                    capability.status, capability.category, capability.description
                ),
            )
        })
        .collect::<Vec<_>>();
    print_panel("Capabilities", &fields);
    Ok(())
}

pub(crate) fn collect_install_check_rows(paths: &AppPaths) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    rows.push((
        "Codex CLI".to_string(),
        command_version_status(codex_bin(), "--version"),
    ));
    rows.push((
        "Codex auth".to_string(),
        command_status(codex_bin(), &["login", "status"]),
    ));
    rows.push((
        "Claude Code".to_string(),
        command_version_status(claude_bin(), "--version"),
    ));
    rows.push((
        "RTK".to_string(),
        command_version_status("rtk", "--version"),
    ));
    rows.push((
        "SQZ MCP".to_string(),
        command_version_status("sqz-mcp", "--version"),
    ));
    rows.push((
        "token-savior".to_string(),
        command_version_status("token-savior", "--version"),
    ));
    rows.push((
        "claw-compactor".to_string(),
        command_probe_status(
            "claw-compactor",
            command_capability_probe_args("claw-compactor"),
        ),
    ));
    rows.push((
        "Caveman assets".to_string(),
        match prodex_caveman_assets::verify_embedded_caveman_assets() {
            Ok(report) => format!(
                "ok (codex files={}, claude files={}, skills={})",
                report.codex_plugin_files, report.claude_plugin_files, report.skill_files
            ),
            Err(err) => format!("fail ({err:#})"),
        },
    ));
    rows.push(("Prodex home".to_string(), paths.root.display().to_string()));
    rows.push((
        "Shared CODEX_HOME".to_string(),
        paths.shared_codex_root.display().to_string(),
    ));
    rows
}

pub(crate) fn handle_setup(args: SetupArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let install_rows = collect_install_check_rows(&paths);
    let asset_report = prodex_caveman_assets::verify_embedded_caveman_assets();

    if args.json {
        let value = serde_json::json!({
            "dry_run": args.dry_run,
            "verify_assets": args.verify_assets,
            "planned_actions": setup_planned_actions(&paths),
            "install_checks": install_rows
                .iter()
                .map(|(name, status)| serde_json::json!({ "name": name, "status": status }))
                .collect::<Vec<_>>(),
            "asset_verification": match asset_report {
                Ok(report) => serde_json::json!({
                    "status": "ok",
                    "codex_plugin_files": report.codex_plugin_files,
                    "claude_plugin_files": report.claude_plugin_files,
                    "skill_files": report.skill_files,
                }),
                Err(err) => serde_json::json!({ "status": "fail", "error": format!("{err:#}") }),
            },
        });
        print_stdout_line(
            &serde_json::to_string_pretty(&value).context("failed to serialize setup report")?,
        );
        return Ok(());
    }

    let title = if args.dry_run {
        "Setup Dry Run"
    } else {
        "Setup"
    };
    print_panel(title, &setup_planned_actions(&paths));
    print_blank_line();
    print_panel("Install Checks", &install_rows);

    if !args.dry_run {
        fs::create_dir_all(&paths.root)
            .with_context(|| format!("failed to create {}", paths.root.display()))?;
        fs::create_dir_all(&paths.managed_profiles_root).with_context(|| {
            format!("failed to create {}", paths.managed_profiles_root.display())
        })?;
    }

    if args.verify_assets {
        asset_report.context("embedded Caveman/Super asset verification failed")?;
    }

    Ok(())
}

fn setup_planned_actions(paths: &AppPaths) -> Vec<(String, String)> {
    vec![
        (
            "Prodex home".to_string(),
            format!("ensure directory {}", paths.root.display()),
        ),
        (
            "Profiles root".to_string(),
            format!("ensure directory {}", paths.managed_profiles_root.display()),
        ),
        (
            "Caveman assets".to_string(),
            "verify embedded plugin manifests and skill frontmatter".to_string(),
        ),
        (
            "Optional tools".to_string(),
            "probe codex, claude, rtk, sqz-mcp, token-savior, claw-compactor".to_string(),
        ),
    ]
}

fn collect_capabilities() -> Vec<ProdexCapability> {
    vec![
        capability("codex", "runtime", Some("codex"), "Codex CLI frontend"),
        capability("claude", "runtime", Some("claude"), "Claude Code frontend"),
        capability(
            "caveman",
            "overlay",
            None,
            "embedded Caveman Codex/Claude plugin assets",
        ),
        capability(
            "rtk",
            "optimizer",
            Some("rtk"),
            "upstream shell-output token reduction",
        ),
        capability(
            "sqz",
            "optimizer",
            Some("sqz-mcp"),
            "downstream context reuse MCP",
        ),
        capability(
            "token-savior",
            "optimizer",
            Some("token-savior"),
            "symbol navigation MCP",
        ),
        capability(
            "claw-compactor",
            "optimizer",
            Some("claw-compactor"),
            "local deterministic code-summary aid",
        ),
        ProdexCapability {
            name: "smart-context",
            category: "runtime",
            status: "built-in".to_string(),
            command: None,
            description: "runtime proxy context compaction and rehydration".to_string(),
        },
        ProdexCapability {
            name: "runtime-doctor",
            category: "diagnostics",
            status: "built-in".to_string(),
            command: None,
            description: "runtime log and pressure diagnostics".to_string(),
        },
    ]
}

fn capability(
    name: &'static str,
    category: &'static str,
    command: Option<&'static str>,
    description: &'static str,
) -> ProdexCapability {
    ProdexCapability {
        name,
        category,
        status: command
            .map(command_available_status)
            .unwrap_or("built-in")
            .to_string(),
        command,
        description: description.to_string(),
    }
}

fn command_available_status(command: &str) -> &'static str {
    if Command::new(command)
        .args(command_capability_probe_args(command))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
    {
        "available"
    } else {
        "missing"
    }
}

fn command_capability_probe_args(command: &str) -> &'static [&'static str] {
    match command {
        "claw-compactor" => &["--help"],
        _ => &["--version"],
    }
}

fn command_version_status(command: impl AsRef<std::ffi::OsStr>, version_arg: &str) -> String {
    let command = command.as_ref();
    match Command::new(command).arg(version_arg).output() {
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
        Err(err) => format!("missing ({})", err.kind()),
    }
}

fn command_probe_status(command: impl AsRef<std::ffi::OsStr>, args: &[&str]) -> String {
    match Command::new(command.as_ref()).args(args).output() {
        Ok(output) if output.status.success() => "ok (available)".to_string(),
        Ok(output) => format!("warn (exit {})", output.status),
        Err(err) => format!("missing ({})", err.kind()),
    }
}

fn command_status(command: impl AsRef<std::ffi::OsStr>, args: &[&str]) -> String {
    match Command::new(command.as_ref()).args(args).output() {
        Ok(output) if output.status.success() => "ok".to_string(),
        Ok(output) => format!("warn (exit {})", output.status),
        Err(err) => format!("missing ({})", err.kind()),
    }
}

#[cfg(test)]
mod tests {
    use super::command_capability_probe_args;

    #[test]
    fn claw_compactor_capability_probe_uses_help() {
        assert_eq!(command_capability_probe_args("claw-compactor"), &["--help"]);
        assert_eq!(command_capability_probe_args("rtk"), &["--version"]);
    }
}
