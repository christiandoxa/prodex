use anyhow::{Context, Result};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use terminal_ui::{
    tui_border_style, tui_connected_header_block, tui_primary_style, tui_secondary_style,
    tui_title_style,
};

use super::{collect_super_tool_statuses, render_super_tool_statuses};
use crate::{
    CavemanArgs, ChildProcessPlan, ProdexUpdateArgs, RuntimeLaunchRequest, RuntimeProxyEndpoint,
    SUPER_LOCAL_PROVIDER_ID, codex_bin, codex_cli_config_override_value, codex_cli_profile_v2_name,
    prepare_runtime_launch_dry_run, preview_deepseek_provider_codex_args,
    preview_external_provider_catalog_codex_args, preview_gemini_provider_codex_args,
    preview_local_provider_catalog_codex_args, profile_openai_compatible_codex_args,
    runtime_caveman_extract_launch_prefixes, runtime_caveman_extract_presidio_prefix,
    runtime_launch_cli_gemini_thinking_budget_tokens,
    runtime_launch_cli_model_context_window_tokens, runtime_launch_openai_spark_context_codex_args,
    validate_credential_free_http_url,
};
pub(crate) use prodex_runtime_launch::{
    RuntimeLaunchDryRunChild, extract_prodex_dry_run_flag, prepare_codex_launch_args,
    prodex_dry_run_requested, remove_upstream_proxy_env,
};

pub(crate) const PROVIDER_SECRET_ENV_KEYS: [&str; 12] = [
    "OPENAI_API_KEYS",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEYS",
    "ANTHROPIC_API_KEY",
    "DEEPSEEK_API_KEYS",
    "DEEPSEEK_API_KEY",
    "GEMINI_API_KEYS",
    "GEMINI_API_KEY",
    "GOOGLE_API_KEYS",
    "GOOGLE_API_KEY",
    "GITHUB_COPILOT_API_KEYS",
    "GITHUB_COPILOT_API_KEY",
];

pub(crate) fn codex_child_plan(codex_home: PathBuf, args: Vec<OsString>) -> ChildProcessPlan {
    prodex_runtime_launch::codex_child_plan(codex_bin(), codex_home, args, SUPER_LOCAL_PROVIDER_ID)
}

pub(crate) fn codex_tui_child_plan(codex_home: PathBuf, args: Vec<OsString>) -> ChildProcessPlan {
    prodex_runtime_launch::codex_tui_child_plan(
        codex_bin(),
        codex_home,
        args,
        SUPER_LOCAL_PROVIDER_ID,
    )
}

pub(crate) fn remove_provider_secret_env(child: &mut ChildProcessPlan) {
    let mut removed = BTreeSet::<OsString>::from_iter(child.removed_env.iter().cloned());
    removed.extend(PROVIDER_SECRET_ENV_KEYS.into_iter().map(OsString::from));
    child.removed_env = removed.into_iter().collect();
}

pub(crate) fn run_child_plan(
    plan: &ChildProcessPlan,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<ExitStatus> {
    run_child_plan_inner(plan, runtime_proxy, None)
}

pub(crate) fn run_child_plan_with_monitor(
    plan: &ChildProcessPlan,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    monitor: &mut dyn FnMut() -> bool,
) -> Result<ExitStatus> {
    run_child_plan_inner(plan, runtime_proxy, Some(monitor))
}

fn run_child_plan_inner(
    plan: &ChildProcessPlan,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    mut monitor: Option<&mut dyn FnMut() -> bool>,
) -> Result<ExitStatus> {
    cleanup_codex_arg0_temp_dirs_best_effort(&plan.codex_home);
    let _session_lock = prodex_shared_codex_fs::lock_codex_sessions_for_child(&plan.codex_home)?;
    let mut command = Command::new(&plan.binary);
    command.args(&plan.args).env("CODEX_HOME", &plan.codex_home);
    for key in &plan.removed_env {
        command.env_remove(key);
    }
    for (key, value) in &plan.extra_env {
        command.env(key, value);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to execute {}", plan.binary.to_string_lossy()))?;
    let _child_runtime_broker_lease = match runtime_proxy {
        Some(proxy) => match proxy.create_child_lease(child.id()) {
            Ok(lease) => Some(lease),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
            }
        },
        None => None,
    };
    let status = match monitor.as_mut() {
        Some(monitor) => wait_for_monitored_child(&mut child, monitor),
        None => child.wait(),
    }
    .with_context(|| format!("failed to wait for {}", plan.binary.to_string_lossy()))?;
    Ok(status)
}

fn wait_for_monitored_child(
    child: &mut Child,
    monitor: &mut dyn FnMut() -> bool,
) -> io::Result<ExitStatus> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if monitor() {
            terminate_child_gracefully(child)?;
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if let Some(status) = child.try_wait()? {
                    return Ok(status);
                }
                if Instant::now() >= deadline {
                    child.kill()?;
                    return child.wait();
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(unix)]
fn terminate_child_gracefully(child: &mut Child) -> io::Result<()> {
    let status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() || child.try_wait()?.is_some() {
        Ok(())
    } else {
        child.kill()
    }
}

#[cfg(windows)]
fn terminate_child_gracefully(child: &mut Child) -> io::Result<()> {
    let status = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() || child.try_wait()?.is_some() {
        Ok(())
    } else {
        child.kill()
    }
}

#[cfg(not(any(unix, windows)))]
fn terminate_child_gracefully(child: &mut Child) -> io::Result<()> {
    child.kill()
}

pub(crate) fn cleanup_codex_arg0_temp_dirs_best_effort(codex_home: &Path) {
    let arg0_root = codex_home.join("tmp").join("arg0");
    if let Err(err) = cleanup_codex_arg0_temp_dirs(&arg0_root) {
        let _ = err;
    }
}

fn cleanup_codex_arg0_temp_dirs(arg0_root: &Path) -> io::Result<()> {
    if !arg0_path_is_regular_dir(arg0_root)? {
        return Ok(());
    }
    let entries = match fs::read_dir(arg0_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            repair_codex_arg0_permissions_best_effort(arg0_root);
            fs::read_dir(arg0_root)?
        }
        Err(err) => return Err(err),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !arg0_path_is_regular_dir(&path)? || !arg0_dir_name_is_owned(&path) {
            continue;
        }
        let Some(_lock_file) = try_lock_codex_arg0_dir(&path)? else {
            continue;
        };
        if let Err(err) = fs::remove_dir_all(&path) {
            if err.kind() == io::ErrorKind::NotFound {
                continue;
            }
            repair_codex_arg0_permissions_best_effort(&path);
            match fs::remove_dir_all(&path) {
                Ok(()) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
    }
    Ok(())
}

fn arg0_path_is_regular_dir(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(!metadata.file_type().is_symlink() && metadata.is_dir()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

fn arg0_dir_name_is_owned(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("codex-arg0"))
}

fn try_lock_codex_arg0_dir(dir: &Path) -> io::Result<Option<File>> {
    let lock_path = dir.join(".lock");
    let lock_file = match File::options().read(true).write(true).open(&lock_path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            repair_codex_arg0_permissions_best_effort(dir);
            match File::options().read(true).write(true).open(&lock_path) {
                Ok(file) => file,
                Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Ok(None),
                Err(err) => return Err(err),
            }
        }
        Err(err) => return Err(err),
    };
    match lock_file.try_lock() {
        Ok(()) => Ok(Some(lock_file)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

#[cfg(unix)]
fn repair_codex_arg0_permissions_best_effort(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    if metadata.is_dir() {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                repair_codex_arg0_permissions_best_effort(&entry.path());
            }
        }
    } else if metadata.is_file() {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
}

#[cfg(not(unix))]
fn repair_codex_arg0_permissions_best_effort(_path: &Path) {}

#[cfg(unix)]
pub(crate) fn handle_prodex_update(_args: ProdexUpdateArgs) -> Result<()> {
    let running_exe = std::env::current_exe().context("failed to locate current prodex binary")?;
    let mut child = Command::new("sh")
        .arg("-s")
        .arg("--")
        .env("PRODEX_RUNNING_EXE", running_exe)
        .env("PRODEX_MIGRATE", "1")
        .env("PRODEX_NON_INTERACTIVE", "1")
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to start the embedded Prodex installer with sh")?;
    child
        .stdin
        .take()
        .context("failed to open Prodex installer stdin")?
        .write_all(include_bytes!("../../../../install.sh"))
        .context("failed to send the embedded Prodex installer to sh")?;
    let status = child
        .wait()
        .context("failed to wait for Prodex installer")?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("Prodex installer exited with {status}")
    }
}

#[cfg(windows)]
pub(crate) fn handle_prodex_update(_args: ProdexUpdateArgs) -> Result<()> {
    let running_exe = std::env::current_exe().context("failed to locate current prodex binary")?;
    let mut child = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "-",
        ])
        .env("PRODEX_RUNNING_EXE", running_exe)
        .env("PRODEX_MIGRATE", "1")
        .env("PRODEX_NON_INTERACTIVE", "1")
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to start the embedded Prodex installer with PowerShell")?;
    child
        .stdin
        .take()
        .context("failed to open Prodex installer stdin")?
        .write_all(include_bytes!("../../../../install.ps1"))
        .context("failed to send the embedded Prodex installer to PowerShell")?;
    let status = child
        .wait()
        .context("failed to wait for Prodex installer")?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("Prodex installer exited with {status}")
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn handle_prodex_update(_args: ProdexUpdateArgs) -> Result<()> {
    anyhow::bail!(
        "prodex update supports macOS, Linux, and Windows; download a binary from https://github.com/christiandoxa/prodex/releases/latest"
    )
}

pub(crate) fn exit_with_status(status: ExitStatus) -> Result<()> {
    std::process::exit(status.code().unwrap_or(1));
}

pub(crate) fn handle_caveman_dry_run(args: CavemanArgs) -> Result<()> {
    if let Some(base_url) = args.base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    let (rtk_enabled, super_optimizer_overlay, codex_args) =
        runtime_caveman_extract_launch_prefixes(&args.codex_args);
    let (presidio_enabled, codex_args) = runtime_caveman_extract_presidio_prefix(codex_args);
    let (_, codex_args) = extract_prodex_dry_run_flag(&codex_args);
    let (codex_args, include_code_review) =
        prepare_codex_launch_args(&codex_args, args.full_access);
    let model_provider_override = codex_cli_config_override_value(&codex_args, "model_provider");
    let profile_v2_name = codex_cli_profile_v2_name(&codex_args);
    let model_context_window_tokens = runtime_launch_cli_model_context_window_tokens(&codex_args);
    let gemini_thinking_budget_tokens =
        runtime_launch_cli_gemini_thinking_budget_tokens(&codex_args);
    let resolved_harness = prodex_provider_core::resolve_harness_mode(args.harness, None);
    let request = RuntimeLaunchRequest {
        profile: args.profile.as_deref(),
        allow_auto_rotate: !args.no_auto_rotate,
        auto_redeem: args.auto_redeem,
        skip_quota_check: args.skip_quota_check,
        base_url: args.base_url.as_deref(),
        upstream_no_proxy: args.no_proxy,
        include_code_review,
        smart_context_enabled: args.smart_context,
        presidio_redaction_enabled: presidio_enabled,
        model_context_window_tokens,
        gemini_thinking_budget_tokens,
        force_runtime_proxy: false,
        model_provider_override: model_provider_override.as_deref(),
        profile_v2_name: profile_v2_name.as_deref(),
        external_provider: args
            .external_provider
            .map(crate::SuperExternalProvider::as_str),
        external_provider_api_key: args.external_provider_api_key.as_deref(),
    };
    let mut extra_report = format!(
        "Prodex overlay: rtk={}; super={}",
        if rtk_enabled { "enabled" } else { "disabled" },
        if super_optimizer_overlay {
            "enabled"
        } else {
            "disabled"
        }
    );
    if super_optimizer_overlay {
        let paths = crate::AppPaths::discover()?;
        let statuses = collect_super_tool_statuses(&paths, presidio_enabled);
        extra_report.push('\n');
        extra_report.push_str(&render_super_tool_statuses(&statuses));
    }
    print_runtime_launch_dry_run(
        "caveman",
        request,
        RuntimeLaunchDryRunChild::Caveman { codex_args },
        Some(resolved_harness),
        Some(&extra_report),
    )
}

pub(crate) fn print_runtime_launch_dry_run(
    flow: &str,
    request: RuntimeLaunchRequest<'_>,
    child: RuntimeLaunchDryRunChild,
    resolved_harness: Option<prodex_provider_core::ResolvedHarnessMode>,
    extra_report: Option<&str>,
) -> Result<()> {
    let upstream_no_proxy = request.upstream_no_proxy;
    let presidio_redaction_enabled = request.presidio_redaction_enabled;
    let prepared = prepare_runtime_launch_dry_run(request)?;
    let local_provider_bridge = prepared
        .runtime_proxy
        .as_ref()
        .and_then(|proxy| proxy.local_model_provider_id.as_deref())
        .is_some();
    let runtime_proxy = runtime_proxy_codex_endpoint(prepared.runtime_proxy.as_ref());
    let child = profile_openai_compatible_dry_run_child(&prepared.codex_home, child)?;
    let plan = prodex_runtime_launch::runtime_launch_dry_run_plan(
        codex_bin(),
        &prepared.codex_home,
        &prepared.paths.managed_profiles_root,
        runtime_proxy,
        upstream_no_proxy,
        SUPER_LOCAL_PROVIDER_ID,
        child,
    );
    let mut output = prodex_runtime_launch::runtime_launch_dry_run_report(
        flow,
        &prepared.codex_home,
        runtime_proxy,
        &plan,
    )?;
    if let Some(extra_report) = extra_report {
        output.push_str(extra_report);
        output.push('\n');
    }
    if local_provider_bridge && let Some(harness) = resolved_harness {
        output.push_str(&runtime_launch_harness_dry_run_line(&harness));
    }
    output.push_str(&format!(
        "Presidio redaction: {}",
        if presidio_redaction_enabled {
            "enabled"
        } else {
            "disabled"
        }
    ));
    output.push('\n');
    print_runtime_launch_dry_run_report(flow, &output)?;
    Ok(())
}

fn runtime_launch_harness_dry_run_line(
    harness: &prodex_provider_core::ResolvedHarnessMode,
) -> String {
    format!(
        "Harness: requested={} resolved={} source={} reason={}\n",
        harness.requested,
        harness.effective,
        harness.source.id(),
        harness.reason
    )
}

fn print_runtime_launch_dry_run_report(flow: &str, output: &str) -> Result<()> {
    let height = runtime_launch_dry_run_tui_height(output);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        print!("{output}");
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Prodex Dry Run", tui_title_style()),
            Span::raw("  "),
            Span::styled(flow.to_string(), tui_secondary_style()),
        ]))
        .block(tui_connected_header_block(tui_border_style()));
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(runtime_launch_dry_run_tui_text(output))
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

fn runtime_launch_dry_run_tui_height(output: &str) -> u16 {
    let rows = output.lines().count().saturating_add(5).clamp(8, 32);
    rows as u16
}

fn runtime_launch_dry_run_tui_text(output: &str) -> Text<'static> {
    Text::from(
        output
            .lines()
            .map(|line| {
                if line.ends_with(':') {
                    Line::from(Span::styled(line.to_string(), tui_title_style()))
                } else if let Some((label, value)) = line.split_once(':') {
                    let value_color = runtime_launch_dry_run_value_color(label, value);
                    Line::from(vec![
                        Span::styled(
                            format!("{label}:"),
                            tui_secondary_style().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            value.trim_start().to_string(),
                            Style::default().fg(value_color),
                        ),
                    ])
                } else {
                    Line::from(Span::styled(line.to_string(), tui_primary_style()))
                }
            })
            .collect::<Vec<_>>(),
    )
}

fn runtime_launch_dry_run_value_color(label: &str, value: &str) -> Color {
    let lower_label = label.to_ascii_lowercase();
    let lower_value = value.to_ascii_lowercase();
    if lower_value.contains("disabled") || lower_value.contains("removed") {
        Color::Red
    } else if lower_value.contains("enabled")
        || lower_value.contains("ready")
        || lower_value.contains("ok")
        || lower_label.contains("proxy")
    {
        Color::Green
    } else if lower_label.contains("command")
        || lower_label.contains("binary")
        || lower_label.contains("codex_home")
    {
        Color::Cyan
    } else {
        Color::Reset
    }
}

fn profile_openai_compatible_dry_run_child(
    codex_home: &Path,
    child: RuntimeLaunchDryRunChild,
) -> Result<RuntimeLaunchDryRunChild> {
    match child {
        RuntimeLaunchDryRunChild::Codex { codex_args } => {
            let codex_args =
                runtime_launch_openai_spark_context_codex_args(codex_home, &codex_args)?;
            let codex_args = profile_openai_compatible_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_local_provider_catalog_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_external_provider_catalog_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_deepseek_provider_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_gemini_provider_codex_args(codex_home, &codex_args)?;
            Ok(RuntimeLaunchDryRunChild::Codex { codex_args })
        }
        RuntimeLaunchDryRunChild::Caveman { codex_args } => {
            let codex_args =
                runtime_launch_openai_spark_context_codex_args(codex_home, &codex_args)?;
            let codex_args = profile_openai_compatible_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_local_provider_catalog_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_external_provider_catalog_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_deepseek_provider_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_gemini_provider_codex_args(codex_home, &codex_args)?;
            Ok(RuntimeLaunchDryRunChild::Caveman { codex_args })
        }
    }
}

fn runtime_proxy_codex_endpoint(
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Option<prodex_runtime_launch::RuntimeProxyCodexEndpoint<'_>> {
    runtime_proxy.map(|proxy| prodex_runtime_launch::RuntimeProxyCodexEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: &proxy.openai_mount_path,
        local_model_provider_id: proxy.local_model_provider_id.as_deref(),
        force_http_responses: proxy.force_http_responses,
        realtime_ws_base_url: proxy.realtime_ws_base_url.as_deref(),
        realtime_ws_model: proxy.realtime_ws_model.as_deref(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_launch_dry_run_tui_text_keeps_report_content() {
        let text = runtime_launch_dry_run_tui_text(
            "Command: codex\nRuntime proxy: enabled\nPresidio redaction: disabled\n",
        );
        let rendered = text
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Command: codex"));
        assert!(rendered.contains("Runtime proxy: enabled"));
        assert!(rendered.contains("Presidio redaction: disabled"));
    }

    #[test]
    fn harness_dry_run_line_reports_immutable_resolution() {
        let resolved = prodex_provider_core::resolve_harness_mode(
            Some(prodex_provider_core::HarnessMode::Minimal),
            None,
        );

        let line = runtime_launch_harness_dry_run_line(&resolved);

        assert!(line.contains("requested=minimal"), "{line}");
        assert!(line.contains("resolved=minimal"), "{line}");
        assert!(line.contains("source=cli"), "{line}");
        assert!(line.contains("reason=explicit CLI selection"), "{line}");
    }

    #[test]
    fn runtime_launch_dry_run_value_color_highlights_status() {
        assert_eq!(
            runtime_launch_dry_run_value_color("Runtime proxy", "enabled"),
            Color::Green
        );
        assert_eq!(
            runtime_launch_dry_run_value_color("Presidio redaction", "disabled"),
            Color::Red
        );
        assert_eq!(
            runtime_launch_dry_run_value_color("Command", "codex"),
            Color::Cyan
        );
    }

    #[test]
    fn dry_run_rejects_profile_url_secret_before_building_child_plan() {
        let root = std::env::temp_dir().join(format!(
            "prodex-dry-run-url-boundary-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(".prodex-profile.toml"),
            "openai_compatible_base_url = 'https://user:dry-run-plan-secret-sentinel@example.test/v1'\n",
        )
        .unwrap();

        let error = profile_openai_compatible_dry_run_child(
            &root,
            RuntimeLaunchDryRunChild::Codex {
                codex_args: Vec::new(),
            },
        )
        .unwrap_err()
        .to_string();

        assert!(
            error.contains("no credentials, query, or fragment"),
            "{error}"
        );
        assert!(!error.contains("secret-sentinel"), "{error}");
        let _ = fs::remove_dir_all(root);
    }
}
