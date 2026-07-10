use crate::{claude_bin, codex_bin, kiro_bin};
use anyhow::{Context, Result, bail};
use crossterm::terminal;
use prodex_cli::{CapabilityCommands, CapabilityListArgs, SetupArgs, SuperDoctorArgs};
use prodex_core::AppPaths;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use terminal_ui::{
    print_panel, print_stdout_line, text_width, tui_border_style, tui_connected_header_block,
    tui_secondary_style, tui_title_style,
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
    command: Option<&'static str>,
    description: String,
}

pub(crate) fn handle_capability(command: CapabilityCommands) -> Result<()> {
    match command {
        CapabilityCommands::List(args) => handle_capability_list(args),
        CapabilityCommands::SuperDoctor(args) => handle_super_doctor(args),
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
        version_check_row("Kiro CLI", kiro_bin(), "--version"),
        version_check_row("RTK", "rtk", "--version"),
        probe_check_row("codebase-memory-mcp"),
    ];
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

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct SuperToolStatus {
    name: &'static str,
    check: &'static str,
    ready: bool,
    status: String,
    detail: String,
}

pub(crate) fn collect_super_tool_statuses(
    paths: &AppPaths,
    check_presidio: bool,
) -> Vec<SuperToolStatus> {
    let asset_report = prodex_caveman_assets::verify_embedded_caveman_assets();
    let mut rows = vec![
        SuperToolStatus {
            name: "caveman-assets",
            check: "embedded-assets",
            ready: asset_report.is_ok(),
            status: match asset_report {
                Ok(report) => format!(
                    "ok (codex files={}, claude files={}, skills={})",
                    report.codex_plugin_files, report.claude_plugin_files, report.skill_files
                ),
                Err(err) => format!("fail ({err:#})"),
            },
            detail: "Caveman/Super embedded assets are required for the Prodex overlay home"
                .to_string(),
        },
        command_tool_status("rtk", "rtk --version", "rtk", &["--version"]),
        command_tool_status("rtk-gain", "rtk gain", "rtk", &["gain"]),
        optimizer_tool_status(
            "codebase-memory-mcp",
            "codebase-memory-mcp MCP tools/list",
            "codebase-memory-mcp",
        ),
        ponytail_tool_status(paths),
        SuperToolStatus {
            name: "smart-context",
            check: "built-in",
            ready: true,
            status: "ok (built-in)".to_string(),
            detail: "Runtime proxy Smart Context Autopilot is built into Prodex".to_string(),
        },
    ];

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

pub(crate) fn render_super_tool_statuses(statuses: &[SuperToolStatus]) -> String {
    let mut rendered = String::from("Optimizers:\n");
    for status in statuses {
        rendered.push_str(&format!("  {}: {}\n", status.name, status.status));
    }
    rendered.trim_end().to_string()
}

fn handle_super_doctor(args: SuperDoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let statuses = collect_super_tool_statuses(&paths, args.presidio);
    let ready = statuses.iter().all(|status| status.ready);

    if args.json {
        let value = serde_json::json!({
            "ready": ready,
            "strict": args.strict,
            "presidio_checked": args.presidio,
            "tools": statuses,
        });
        print_stdout_line(
            &serde_json::to_string_pretty(&value)
                .context("failed to serialize Super doctor report")?,
        );
    } else {
        let fields = statuses
            .iter()
            .map(|status| {
                (
                    status.name.to_string(),
                    format!("{}; {}; {}", status.status, status.check, status.detail),
                )
            })
            .collect::<Vec<_>>();
        print_capability_panel("Super Doctor", &fields)?;
    }

    if args.strict && !ready {
        bail!("Super optimizer doctor found unavailable tools");
    }
    Ok(())
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
    print_capability_panels(&[
        CapabilityPanel {
            title: title.to_string(),
            fields: setup_planned_actions(&paths),
        },
        CapabilityPanel {
            title: "Install Checks".to_string(),
            fields: install_rows.clone(),
        },
    ])?;

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

fn print_capability_panel(title: &str, fields: &[(String, String)]) -> Result<()> {
    print_capability_panels(&[CapabilityPanel {
        title: title.to_string(),
        fields: fields.to_vec(),
    }])
}

fn print_capability_panels(panels: &[CapabilityPanel]) -> Result<()> {
    let height = capability_tui_height(panels);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        for panel in panels {
            print_panel(&panel.title, &panel.fields);
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
            "Caveman assets".to_string(),
            "verify embedded plugin manifests and skill frontmatter".to_string(),
        ),
        (
            "Super tools".to_string(),
            "probe codex, claude, rtk, codebase-memory-mcp, ponytail, and Presidio".to_string(),
        ),
    ]
}

fn collect_capabilities() -> Vec<ProdexCapability> {
    vec![
        capability("codex", "runtime", Some("codex"), "Codex CLI frontend"),
        capability("claude", "runtime", Some("claude"), "Claude Code frontend"),
        capability(
            "caveman",
            "mode-assets",
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
            "codebase-memory-mcp",
            "optimizer",
            Some("codebase-memory-mcp"),
            "structural codebase graph MCP",
        ),
        capability(
            "ponytail",
            "optimizer-plugin",
            None,
            "managed checkout loaded as a Codex plugin in Prodex overlays",
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
        .is_ok_and(|status| status.success())
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

fn ponytail_tool_status(_paths: &AppPaths) -> SuperToolStatus {
    let candidates = [
        env::var_os("PRODEX_OPTIMIZERS_HOME").map(PathBuf::from),
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .map(|path| path.join("prodex-optimizers")),
        dirs_home_dir().map(|home| home.join(".local").join("share").join("prodex-optimizers")),
    ];
    let checkout = candidates
        .into_iter()
        .flatten()
        .map(|root| root.join("ponytail"))
        .find(|checkout| {
            checkout.join(".codex-plugin").join("plugin.json").is_file()
                && checkout
                    .join("hooks")
                    .join("claude-codex-hooks.json")
                    .is_file()
                && checkout.join("skills").is_dir()
        });
    match checkout {
        Some(path) => SuperToolStatus {
            name: "ponytail",
            check: "managed checkout",
            ready: true,
            status: format!("ok ({})", path.display()),
            detail: "Ponytail checkout will be installed into Prodex overlay plugin cache"
                .to_string(),
        },
        None => SuperToolStatus {
            name: "ponytail",
            check: "managed checkout",
            ready: false,
            status: "missing".to_string(),
            detail: "expected checkout at $PRODEX_OPTIMIZERS_HOME/ponytail, $XDG_DATA_HOME/prodex-optimizers/ponytail, or ~/.local/share/prodex-optimizers/ponytail; install with `git clone https://github.com/DietrichGebert/ponytail.git ~/.local/share/prodex-optimizers/ponytail`"
                .to_string(),
        },
    }
}

fn dirs_home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

fn command_tool_status(
    name: &'static str,
    check: &'static str,
    command: &str,
    args: &[&str],
) -> SuperToolStatus {
    command_path_tool_status(
        name,
        check,
        PathBuf::from(command),
        args,
        format!("{command} was not found on PATH"),
    )
}

fn optimizer_tool_status(
    name: &'static str,
    check: &'static str,
    command: &str,
) -> SuperToolStatus {
    match find_optimizer_command_for_super_status(command) {
        Some(path) => SuperToolStatus {
            name,
            check,
            ready: true,
            status: format!("ok ({})", path.display()),
            detail: format!("{command} is invokable from the Super overlay"),
        },
        None => SuperToolStatus {
            name,
            check,
            ready: false,
            status: "missing".to_string(),
            detail: format!("{command} was not found on PATH or in managed optimizer roots"),
        },
    }
}

fn command_path_tool_status(
    name: &'static str,
    check: &'static str,
    command: PathBuf,
    args: &[&str],
    missing_detail: String,
) -> SuperToolStatus {
    match Command::new(&command).args(args).output() {
        Ok(output) if output.status.success() => SuperToolStatus {
            name,
            check,
            ready: true,
            status: format!("ok ({})", first_output_line(&output).unwrap_or("available")),
            detail: format!("command={}", command.display()),
        },
        Ok(output) => SuperToolStatus {
            name,
            check,
            ready: false,
            status: format!("fail (exit {})", output.status),
            detail: first_output_line(&output)
                .unwrap_or("command exited without diagnostic output")
                .to_string(),
        },
        Err(err) => SuperToolStatus {
            name,
            check,
            ready: false,
            status: format!("missing ({})", err.kind()),
            detail: missing_detail,
        },
    }
}

fn first_output_line(output: &std::process::Output) -> Option<&str> {
    let stdout = std::str::from_utf8(&output.stdout).ok();
    let stderr = std::str::from_utf8(&output.stderr).ok();
    stdout
        .into_iter()
        .flat_map(str::lines)
        .chain(stderr.into_iter().flat_map(str::lines))
        .map(str::trim)
        .find(|line| !line.is_empty())
}

fn find_optimizer_command_for_super_status(command: &str) -> Option<PathBuf> {
    find_managed_optimizer_command_for_super_status(command)
        .or_else(|| find_path_optimizer_command_for_super_status(command))
}

fn find_managed_optimizer_command_for_super_status(command: &str) -> Option<PathBuf> {
    managed_optimizer_roots_for_super_status()
        .into_iter()
        .flat_map(|root| managed_optimizer_command_candidates_for_super_status(&root, command))
        .find(|candidate| optimizer_command_ready_for_super_status(command, candidate))
}

fn find_path_optimizer_command_for_super_status(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(command);
        if optimizer_command_ready_for_super_status(command, &candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{command}.exe"));
            if optimizer_command_ready_for_super_status(command, &candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn managed_optimizer_roots_for_super_status() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(path) = env::var_os("PRODEX_OPTIMIZERS_HOME") {
        push_unique_path(&mut roots, PathBuf::from(path));
    }
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        push_unique_path(&mut roots, PathBuf::from(path).join("prodex-optimizers"));
    }
    if let Some(home) = dirs_home_dir() {
        push_unique_path(
            &mut roots,
            home.join(".local").join("share").join("prodex-optimizers"),
        );
    }
    roots
}

fn managed_optimizer_command_candidates_for_super_status(
    root: &Path,
    command: &str,
) -> Vec<PathBuf> {
    let mut candidates = vec![root.join(command)];
    match command {
        "codebase-memory-mcp" => {
            let checkout = root.join("codebase-memory-mcp");
            candidates.push(checkout.join(command));
            candidates.push(checkout.join("build").join("c").join(command));
            candidates.push(checkout.join("bin").join(command));
        }
        _ => {}
    }
    candidates
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn optimizer_command_ready_for_super_status(command: &str, path: &Path) -> bool {
    prodex_caveman_assets::super_optimizer_command_ready(command, path)
}

fn presidio_tool_status(paths: &AppPaths) -> SuperToolStatus {
    let config = match crate::presidio_runtime::runtime_presidio_redaction_config(paths) {
        Ok(config) => config,
        Err(err) => {
            return SuperToolStatus {
                name: "presidio",
                check: "Presidio Analyzer/Anonymizer health",
                ready: false,
                status: "fail (config)".to_string(),
                detail: format!("{err:#}"),
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
                detail: err.to_string(),
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
            "analyzer={} {}; anonymizer={} {}; fail_mode={}",
            config.analyzer_url,
            analyzer.1,
            config.anonymizer_url,
            anonymizer.1,
            if config.fail_closed { "closed" } else { "open" }
        ),
    }
}

fn presidio_health(client: &reqwest::blocking::Client, base_url: &str) -> (bool, String) {
    let url = format!("{}/health", base_url.trim_end_matches('/'));
    match client.get(url).send() {
        Ok(response) if response.status().is_success() => (true, "ok".to_string()),
        Ok(response) => (false, format!("status {}", response.status())),
        Err(err) => (false, err.to_string()),
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
#[path = "../../tests/src/app_commands/capability.rs"]
mod tests;
