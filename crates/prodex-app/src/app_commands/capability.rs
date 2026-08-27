use crate::{
    agy_bin, claude_bin, codex_bin, copilot_bin, gemini_bin, kiro_bin,
    runtime_smart_context_offline_self_test,
};
use anyhow::{Context, Result, bail};
use crossterm::terminal;
use prodex_cli::{CapabilityCommands, CapabilityListArgs, SetupArgs};
use prodex_core::AppPaths;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use redaction::redaction_redact_secret_like_text;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::process::Command;
use std::time::Duration;
use terminal_ui::{
    print_panel, print_stdout_line, text_width, tui_border_style, tui_connected_header_block,
    tui_secondary_style, tui_title_style,
};

#[path = "capability/setup_optional_tools.rs"]
mod setup_optional_tools;
use setup_optional_tools::{
    collect_setup_optional_tool_health, ensure_optional_tools_installed, setup_optional_tool_rows,
    setup_optional_tool_verification_json,
};

#[derive(Debug, Clone)]
struct CapabilityPanel {
    title: String,
    fields: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct ProdexCapability {
    name: &'static str,
    category: &'static str,
    status: String,
    command: Option<String>,
    description: String,
}

fn capability_redacted_detail(value: &str) -> String {
    redaction_redact_secret_like_text(value)
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}
#[cfg(test)]
fn capability_failed_status(err: &anyhow::Error) -> String {
    format!("fail ({})", capability_redacted_detail(&format!("{err:#}")))
}

pub(crate) fn handle_capability(command: CapabilityCommands) -> Result<()> {
    match command {
        CapabilityCommands::List(args) => handle_capability_list(args),
        CapabilityCommands::SuperDoctor(args) => super::super_doctor::handle_super_doctor(args),
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
        )?;
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
    print_capability_panel("Capabilities", &fields)?;
    Ok(())
}

pub(crate) fn collect_install_check_rows(paths: &AppPaths) -> Vec<(String, String)> {
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

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct SuperToolStatus {
    pub(crate) name: &'static str,
    pub(crate) check: &'static str,
    pub(crate) ready: bool,
    pub(crate) status: String,
    pub(crate) detail: String,
}

pub(crate) fn collect_super_tool_statuses(
    paths: &AppPaths,
    check_presidio: bool,
) -> Vec<SuperToolStatus> {
    let mut rows = prodex_optional_tools::OptionalToolSet::super_defaults()
        .iter()
        .map(optional_tool_super_status)
        .chain([smart_context_super_status()])
        .collect::<Vec<_>>();

    rows.push(if check_presidio {
        presidio_tool_status(paths)
    } else {
        SuperToolStatus {
            name: "presidio",
            check: "opt-in",
            ready: true,
            status: "disabled (not checked)".to_string(),
            detail: "Presidio is checked only when `prodex s doctor --presidio` or `prodex s --presidio` is used".to_string(),
        }
    });
    rows
}

fn smart_context_super_status() -> SuperToolStatus {
    match runtime_smart_context_offline_self_test() {
        Ok(result) => SuperToolStatus {
            name: "smart-context",
            check: "offline-self-test",
            ready: true,
            status: format!("ok (tokenizer={})", result.tokenizer_family),
            detail: result.detail,
        },
        Err(error) => SuperToolStatus {
            name: "smart-context",
            check: "offline-self-test",
            ready: false,
            status: "degraded".to_string(),
            detail: capability_redacted_detail(&format!("{error:#}")),
        },
    }
}

fn optional_tool_super_status(id: prodex_optional_tools::OptionalToolId) -> SuperToolStatus {
    let health = prodex_optional_tools::optional_tool_status(id);
    let mut detail = health.detail;
    if let Some(version) = &health.version {
        detail.push_str(&format!("; version={version}"));
    }
    detail.push_str(&format!(
        "; recommended={}",
        prodex_optional_tools::optional_tool_recommended_version(id)
    ));
    SuperToolStatus {
        name: id.as_str(),
        check: "optional-tool-registry",
        ready: health.status == prodex_optional_tools::ToolHealthStatus::Installed,
        status: optional_tool_health_status(&health),
        detail: capability_redacted_detail(&detail),
    }
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
    capability_redacted_detail(&fields.join(", "))
}

pub(crate) fn handle_setup(args: SetupArgs) -> Result<()> {
    if args.dry_run && args.verify_tools {
        bail!("--dry-run cannot be combined with --verify-tools");
    }
    let paths = AppPaths::discover()?;
    let install_rows = if args.dry_run {
        collect_install_check_rows_passive(&paths)
    } else {
        collect_install_check_rows(&paths)
    };
    let optional_tools =
        (args.verify_tools && !args.dry_run).then(collect_setup_optional_tool_health);
    let standalone_caveman = (args.json && !args.verify_tools && !args.dry_run).then(|| {
        prodex_optional_tools::optional_tool_status(prodex_optional_tools::OptionalToolId::Caveman)
    });

    if !args.dry_run {
        fs::create_dir_all(&paths.root)
            .with_context(|| format!("failed to create {}", paths.root.display()))?;
        fs::create_dir_all(&paths.managed_profiles_root).with_context(|| {
            format!("failed to create {}", paths.managed_profiles_root.display())
        })?;
    }

    if args.json {
        let value = serde_json::json!({
            "dry_run": args.dry_run,
            "verify_optional_tools": args.verify_tools,
            "planned_actions": setup_planned_actions(&paths),
            "install_checks": install_rows
                .iter()
                .map(|(name, status)| serde_json::json!({ "name": name, "status": status }))
                .collect::<Vec<_>>(),
            "optional_tool_verification": setup_optional_tool_verification_json(
                optional_tools.as_deref().and_then(|tools| {
                    tools.iter().find(|tool| {
                        tool.id == prodex_optional_tools::OptionalToolId::Caveman
                    })
                }).or(standalone_caveman.as_ref()),
                optional_tools.as_deref(),
                args.verify_tools,
                args.dry_run,
            ),
        });
        print_stdout_line(
            &serde_json::to_string_pretty(&value).context("failed to serialize setup report")?,
        )?;
        if args.verify_tools {
            let tools = optional_tools.as_deref().context(
                "optional-tool verification is not executed during --dry-run; rerun without --dry-run",
            )?;
            ensure_optional_tools_installed(tools)?;
        }
        return Ok(());
    }

    let title = if args.dry_run {
        "Setup Dry Run"
    } else {
        "Setup"
    };
    let mut panels = vec![
        CapabilityPanel {
            title: title.to_string(),
            fields: setup_planned_actions(&paths),
        },
        CapabilityPanel {
            title: "Install Checks".to_string(),
            fields: install_rows.clone(),
        },
    ];
    if args.verify_tools {
        panels.push(CapabilityPanel {
            title: "Optional Tool Checks".to_string(),
            fields: optional_tools.as_deref().map_or_else(
                || {
                    prodex_optional_tools::OptionalToolSet::super_defaults()
                        .iter()
                        .map(|id| (id.to_string(), "not checked (dry-run)".to_string()))
                        .collect()
                },
                setup_optional_tool_rows,
            ),
        });
    }
    print_capability_panels(&panels)?;

    if args.verify_tools {
        let tools = optional_tools.as_deref().context(
            "optional-tool verification is not executed during --dry-run; rerun without --dry-run",
        )?;
        ensure_optional_tools_installed(tools)?;
    }

    Ok(())
}

fn collect_install_check_rows_passive(paths: &AppPaths) -> Vec<(String, String)> {
    let mut rows = [
        ("Codex CLI", codex_bin()),
        ("Claude Code", claude_bin()),
        ("Gemini CLI", gemini_bin()),
        ("GitHub Copilot CLI", copilot_bin()),
        ("Kiro CLI", kiro_bin()),
        ("Antigravity CLI", agy_bin()),
        ("RTK", OsString::from("rtk")),
        ("Node.js", OsString::from("node")),
        ("npx", OsString::from("npx")),
        ("codebase-memory-mcp", OsString::from("codebase-memory-mcp")),
    ]
    .into_iter()
    .map(|(name, command)| {
        let status =
            prodex_core::resolve_binary_path_in_path(&command, env::var_os("PATH").as_deref())
                .map_or_else(
                    || "missing".to_string(),
                    |path| format!("available ({})", path.display()),
                );
        (name.to_string(), status)
    })
    .collect::<Vec<_>>();
    rows.push((
        "Codex auth".to_string(),
        "not checked (dry-run)".to_string(),
    ));
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

fn ensure_optional_tool_installed(health: &prodex_optional_tools::ToolHealth) -> Result<()> {
    if health.status == prodex_optional_tools::ToolHealthStatus::Installed {
        Ok(())
    } else {
        bail!(
            "optional tool {} is {}: {}",
            health.id,
            optional_tool_health_status(health),
            capability_redacted_detail(&health.detail)
        )
    }
}

pub(crate) fn print_capability_panel(title: &str, fields: &[(String, String)]) -> Result<()> {
    print_capability_panels(&[CapabilityPanel {
        title: title.to_string(),
        fields: fields.to_vec(),
    }])
}

fn print_capability_panels(panels: &[CapabilityPanel]) -> Result<()> {
    let height = capability_tui_height(panels);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        for panel in panels {
            print_panel(&panel.title, &panel.fields)?;
        }
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::styled("Prodex Capabilities", tui_title_style()))
            .block(tui_connected_header_block(tui_border_style()));
        frame.render_widget(header, chunks[0]);
        let body = Paragraph::new(capability_tui_text(panels))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    let _ = terminal.show_cursor();
    Ok(())
}

fn capability_tui_height(panels: &[CapabilityPanel]) -> u16 {
    let rows = capability_tui_text(panels)
        .lines
        .len()
        .saturating_add(4)
        .max(4);
    let terminal_height = terminal::size()
        .map(|(_, height)| usize::from(height))
        .unwrap_or(24);
    rows.min(terminal_height).max(1) as u16
}

fn capability_tui_text(panels: &[CapabilityPanel]) -> Text<'static> {
    let mut lines = Vec::new();
    for panel in panels {
        lines.push(Line::styled(panel.title.clone(), tui_title_style()));
        let label_width = panel
            .fields
            .iter()
            .map(|(label, _)| text_width(label))
            .max()
            .unwrap_or(0)
            .min(24);
        for (label, value) in &panel.fields {
            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "{label}{} ",
                        " ".repeat(label_width.saturating_sub(text_width(label)))
                    ),
                    tui_secondary_style().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    value.clone(),
                    Style::default().fg(capability_value_color(value)),
                ),
            ]));
        }
    }
    Text::from(lines)
}

fn capability_value_color(value: &str) -> Color {
    let lower = value.to_ascii_lowercase();
    if lower.contains("fail")
        || lower.contains("unavailable")
        || lower.contains("disabled")
        || lower.contains("not checked")
    {
        Color::Red
    } else if lower.contains("ok") || lower.contains("built-in") || lower.contains("ensure") {
        Color::Green
    } else {
        Color::Reset
    }
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
            "Optional tools".to_string(),
            "resolve and validate external tool installations without modifying them".to_string(),
        ),
        (
            "Super tools".to_string(),
            "probe codex, claude, gemini, copilot, kiro, agy, rtk, npx, Codebase Memory MCP, Playwright MCP, Ponytail, and Presidio".to_string(),
        ),
    ]
}

fn collect_capabilities() -> Vec<ProdexCapability> {
    vec![
        capability("codex", "runtime", Some(codex_bin()), "Codex CLI frontend"),
        capability("claude", "runtime", Some(claude_bin()), "Claude frontend"),
        capability("gemini", "runtime", Some(gemini_bin()), "Gemini proxy"),
        capability(
            "copilot",
            "runtime",
            Some(copilot_bin()),
            "GitHub Copilot CLI frontend through the Prodex Responses proxy",
        ),
        capability(
            "kiro",
            "runtime",
            Some(kiro_bin()),
            "Kiro CLI and ACP bridge",
        ),
        capability("antigravity", "runtime", Some(agy_bin()), "Antigravity CLI"),
        optional_tool_capability(
            prodex_optional_tools::OptionalToolId::Caveman,
            "optional-plugin",
            "validated external Caveman installation for Codex and Claude",
        ),
        optional_tool_capability(
            prodex_optional_tools::OptionalToolId::Rtk,
            "optimizer",
            "upstream shell-output token reduction",
        ),
        optional_tool_capability(
            prodex_optional_tools::OptionalToolId::CodebaseMemoryMcp,
            "optimizer",
            "structural codebase graph MCP",
        ),
        optional_tool_capability(
            prodex_optional_tools::OptionalToolId::PlaywrightMcp,
            "optimizer",
            "isolated headless browser automation MCP",
        ),
        optional_tool_capability(
            prodex_optional_tools::OptionalToolId::Ponytail,
            "optimizer-plugin",
            "managed checkout loaded as a Codex plugin in Prodex overlays",
        ),
        smart_context_capability(),
        ProdexCapability {
            name: "runtime-doctor",
            category: "diagnostics",
            status: "built-in".to_string(),
            command: None,
            description: "runtime log and pressure diagnostics".to_string(),
        },
    ]
}

fn smart_context_capability() -> ProdexCapability {
    match runtime_smart_context_offline_self_test() {
        Ok(result) => ProdexCapability {
            name: "smart-context",
            category: "runtime",
            status: format!("available ({})", result.tokenizer_family),
            command: None,
            description: result.detail,
        },
        Err(error) => ProdexCapability {
            name: "smart-context",
            category: "runtime",
            status: "degraded".to_string(),
            command: None,
            description: capability_redacted_detail(&format!("{error:#}")),
        },
    }
}

fn optional_tool_capability(
    id: prodex_optional_tools::OptionalToolId,
    category: &'static str,
    description: &'static str,
) -> ProdexCapability {
    let health = prodex_optional_tools::optional_tool_status(id);
    ProdexCapability {
        name: id.as_str(),
        category,
        status: optional_tool_health_status(&health),
        command: health.path.map(|path| path.display().to_string()),
        description: description.to_string(),
    }
}

fn capability(
    name: &'static str,
    category: &'static str,
    command: Option<OsString>,
    description: &'static str,
) -> ProdexCapability {
    ProdexCapability {
        name,
        category,
        status: command
            .as_deref()
            .map(|command| command_available_status(name, command))
            .unwrap_or("built-in")
            .to_string(),
        command: command.map(|command| command.to_string_lossy().into_owned()),
        description: description.to_string(),
    }
}

fn command_available_status(capability_name: &str, command: &OsStr) -> &'static str {
    let mut probe = Command::new(command);
    probe.args(command_capability_probe_args(capability_name));
    if crate::command_probe_output(&mut probe, capability_name)
        .is_ok_and(|output| output.status.success())
    {
        "available"
    } else {
        "missing"
    }
}

fn command_capability_probe_args(command: &str) -> &'static [&'static str] {
    match command {
        "codebase-memory-mcp" => &["--help"],
        _ => &["--version"],
    }
}

fn command_version_status(command: impl AsRef<std::ffi::OsStr>, version_arg: &str) -> String {
    let command = command.as_ref();
    let mut probe = Command::new(command);
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
        Err(err) => format!("missing ({})", capability_redacted_detail(&err.to_string())),
    }
}

fn command_probe_status(command: impl AsRef<std::ffi::OsStr>, args: &[&str]) -> String {
    let command = command.as_ref();
    let mut probe = Command::new(command);
    probe.args(args);
    match crate::command_probe_output(&mut probe, &command.to_string_lossy()) {
        Ok(output) if output.status.success() => "ok (available)".to_string(),
        Ok(output) => format!("warn (exit {})", output.status),
        Err(err) => format!("missing ({})", capability_redacted_detail(&err.to_string())),
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

fn presidio_tool_status(paths: &AppPaths) -> SuperToolStatus {
    let recommended = prodex_optional_tools::optional_tool_recommended_version(
        prodex_optional_tools::OptionalToolId::Presidio,
    );
    let config = match crate::presidio_runtime::runtime_presidio_redaction_config(paths) {
        Ok(config) => config,
        Err(err) => {
            return SuperToolStatus {
                name: "presidio",
                check: "Presidio Analyzer/Anonymizer health",
                ready: false,
                status: "fail (config)".to_string(),
                detail: capability_redacted_detail(&format!("{err:#}; recommended={recommended}")),
            };
        }
    };

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            return SuperToolStatus {
                name: "presidio",
                check: "Presidio Analyzer/Anonymizer health",
                ready: false,
                status: "fail (http client)".to_string(),
                detail: capability_redacted_detail(&format!("{err}; recommended={recommended}")),
            };
        }
    };
    let analyzer = presidio_health(&client, &config.analyzer_url);
    let anonymizer = presidio_health(&client, &config.anonymizer_url);
    let ready = analyzer.0 && anonymizer.0;
    SuperToolStatus {
        name: "presidio",
        check: "Presidio Analyzer/Anonymizer health",
        ready,
        status: if ready {
            "ok".to_string()
        } else {
            "fail".to_string()
        },
        detail: format!(
            "analyzer={} {}; anonymizer={} {}; fail_mode={}; recommended={}",
            config.analyzer_url,
            analyzer.1,
            config.anonymizer_url,
            anonymizer.1,
            if config.fail_closed { "closed" } else { "open" },
            recommended,
        ),
    }
}

fn presidio_health(client: &reqwest::blocking::Client, base_url: &str) -> (bool, String) {
    let url = format!("{}/health", base_url.trim_end_matches('/'));
    match client.get(url).send() {
        Ok(response) if response.status().is_success() => (true, "ok".to_string()),
        Ok(response) => (false, format!("status {}", response.status())),
        Err(err) => (false, capability_redacted_detail(&err.to_string())),
    }
}

fn command_status(command: impl AsRef<std::ffi::OsStr>, args: &[&str]) -> String {
    let command = command.as_ref();
    let mut probe = Command::new(command);
    probe.args(args);
    match crate::command_probe_output(&mut probe, &command.to_string_lossy()) {
        Ok(output) if output.status.success() => "ok".to_string(),
        Ok(output) => format!("warn (exit {})", output.status),
        Err(err) => format!("missing ({})", capability_redacted_detail(&err.to_string())),
    }
}

#[cfg(test)]
#[path = "../../tests/src/app_commands/capability.rs"]
mod tests;
