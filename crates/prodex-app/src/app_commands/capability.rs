use crate::{claude_bin, codex_bin, default_memory_store_path, memory_store_ready};
use anyhow::{Context, Result, bail};
use prodex_cli::{CapabilityCommands, CapabilityListArgs, SetupArgs, SuperDoctorArgs};
use prodex_core::AppPaths;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
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
    print_panel("Capabilities", &fields);
    Ok(())
}

pub(crate) fn collect_install_check_rows(paths: &AppPaths) -> Vec<(String, String)> {
    let mut rows = vec![
        (
            "Codex CLI".to_string(),
            command_version_status(codex_bin(), "--version"),
        ),
        (
            "Codex auth".to_string(),
            command_status(codex_bin(), &["login", "status"]),
        ),
        (
            "Claude Code".to_string(),
            command_version_status(claude_bin(), "--version"),
        ),
        (
            "RTK".to_string(),
            command_version_status("rtk", "--version"),
        ),
        (
            "SQZ MCP".to_string(),
            command_version_status("sqz-mcp", "--version"),
        ),
        (
            "token-savior".to_string(),
            command_version_status("token-savior", "--version"),
        ),
        (
            "claw-compactor".to_string(),
            command_probe_status(
                "claw-compactor",
                command_capability_probe_args("claw-compactor"),
            ),
        ),
    ];
    let memory_store = default_memory_store_path(paths);
    rows.push((
        "prodex-memory".to_string(),
        match memory_store_ready(&memory_store) {
            Ok(()) => format!("ok (local sqlite {})", memory_store.display()),
            Err(err) => format!("fail ({err:#})"),
        },
    ));
    rows.push((
        "prodex-inspect".to_string(),
        "ok (built-in read-only MCP)".to_string(),
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
        command_tool_status(
            "sqz-mcp",
            "sqz-mcp --transport stdio --help",
            "sqz-mcp",
            &["--transport", "stdio", "--help"],
        ),
        command_tool_status(
            "token-savior",
            "token-savior --version",
            "token-savior",
            &["--version"],
        ),
        command_tool_status(
            "claw-compactor",
            "claw-compactor --help",
            "claw-compactor",
            &["--help"],
        ),
        SuperToolStatus {
            name: "prodex-inspect",
            check: "built-in",
            ready: true,
            status: "ok (read-only MCP)".to_string(),
            detail: "Read-only MCP diagnostics for Prodex status, profiles, and latest runtime log"
                .to_string(),
        },
        memory_tool_status(paths),
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
        print_panel("Super Doctor", &fields);
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
            "probe codex, claude, rtk, sqz-mcp, token-savior, claw-compactor, prodex-inspect, prodex-memory"
                .to_string(),
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
            name: "prodex-inspect",
            category: "diagnostics",
            status: "built-in".to_string(),
            command: Some("prodex __inspect-mcp"),
            description: "read-only MCP diagnostics for Prodex status, profiles, and runtime logs"
                .to_string(),
        },
        ProdexCapability {
            name: "prodex-memory",
            category: "memory",
            status: "built-in".to_string(),
            command: Some("prodex __memory-mcp"),
            description: "local-first SQLite memory MCP without Mem0 Cloud auth".to_string(),
        },
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

fn memory_tool_status(paths: &AppPaths) -> SuperToolStatus {
    let store = default_memory_store_path(paths);
    match memory_store_ready(&store) {
        Ok(()) => SuperToolStatus {
            name: "prodex-memory",
            check: "local SQLite memory store",
            ready: true,
            status: "ok (local sqlite)".to_string(),
            detail: format!(
                "store={}; Mem0 Cloud disabled; no MEM0_API_KEY required",
                store.display()
            ),
        },
        Err(err) => SuperToolStatus {
            name: "prodex-memory",
            check: "local SQLite memory store",
            ready: false,
            status: "fail".to_string(),
            detail: format!("{err:#}"),
        },
    }
}

fn command_tool_status(
    name: &'static str,
    check: &'static str,
    command: &str,
    args: &[&str],
) -> SuperToolStatus {
    match Command::new(command).args(args).output() {
        Ok(output) if output.status.success() => SuperToolStatus {
            name,
            check,
            ready: true,
            status: format!("ok ({})", first_output_line(&output).unwrap_or("available")),
            detail: command_path_detail(command),
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
            detail: format!("{command} was not found on PATH"),
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

fn command_path_detail(command: &str) -> String {
    find_path_command(command)
        .map(|path| format!("command={}", path.display()))
        .unwrap_or_else(|| format!("command={command}"))
}

fn find_path_command(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(command);
        if executable_file(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{command}.exe"));
            if executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn executable_file(path: &Path) -> bool {
    path.is_file()
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
mod tests {
    use super::command_capability_probe_args;

    #[test]
    fn claw_compactor_capability_probe_uses_help() {
        assert_eq!(command_capability_probe_args("claw-compactor"), &["--help"]);
        assert_eq!(command_capability_probe_args("rtk"), &["--version"]);
    }
}
