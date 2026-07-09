use anyhow::{Context, Result};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use terminal_ui::{
    tui_border_style, tui_connected_header_block, tui_primary_style, tui_secondary_style,
    tui_title_style,
};

use super::{
    SuperMemoryStatusMode, collect_super_tool_statuses_with_memory_mode, render_super_tool_statuses,
};
use crate::{
    CavemanArgs, ChildProcessPlan, CodexUpdateArgs, RuntimeLaunchRequest, RuntimeProxyEndpoint,
    SUPER_LOCAL_PROVIDER_ID, codex_bin, codex_cli_config_override_value, codex_cli_profile_v2_name,
    prepare_runtime_launch_dry_run, preview_deepseek_provider_codex_args,
    preview_external_provider_catalog_codex_args, preview_gemini_provider_codex_args,
    preview_local_provider_catalog_codex_args, profile_openai_compatible_codex_args,
    runtime_caveman_extract_launch_prefixes, runtime_caveman_extract_presidio_prefix,
    runtime_launch_cli_gemini_thinking_budget_tokens,
    runtime_launch_cli_model_context_window_tokens, runtime_launch_openai_spark_context_codex_args,
};
pub(crate) use prodex_runtime_launch::{
    RuntimeLaunchDryRunChild, default_child_removed_env, extract_prodex_dry_run_flag,
    prepare_codex_launch_args, prodex_dry_run_requested, remove_upstream_proxy_env,
};

pub(crate) fn codex_child_plan(codex_home: PathBuf, args: Vec<OsString>) -> ChildProcessPlan {
    prodex_runtime_launch::codex_child_plan(codex_bin(), codex_home, args, SUPER_LOCAL_PROVIDER_ID)
}

pub(crate) fn run_child_plan(
    plan: &ChildProcessPlan,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<ExitStatus> {
    cleanup_codex_arg0_temp_dirs_best_effort(&plan.codex_home);
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
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for {}", plan.binary.to_string_lossy()))?;
    Ok(status)
}

pub(crate) fn cleanup_codex_arg0_temp_dirs_best_effort(codex_home: &Path) {
    let arg0_root = codex_home.join("tmp").join("arg0");
    if let Err(err) = cleanup_codex_arg0_temp_dirs(&arg0_root) {
        let _ = err;
    }
}

fn cleanup_codex_arg0_temp_dirs(arg0_root: &Path) -> io::Result<()> {
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
        if !path.is_dir() || !arg0_dir_name_is_owned(&path) {
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
    if metadata.is_dir() {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                repair_codex_arg0_permissions_best_effort(&entry.path());
            }
        }
    } else {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
}

#[cfg(not(unix))]
fn repair_codex_arg0_permissions_best_effort(_path: &Path) {}

pub(crate) fn run_codex_direct_passthrough(args: Vec<OsString>) -> Result<ExitStatus> {
    let binary = codex_bin();
    let mut command = Command::new(&binary);
    command.args(args);
    for key in default_child_removed_env() {
        command.env_remove(key);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to execute {}", binary.to_string_lossy()))?;
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for {}", binary.to_string_lossy()))?;
    Ok(status)
}

pub(crate) fn handle_codex_update(args: CodexUpdateArgs) -> Result<()> {
    let mut command_args = Vec::with_capacity(args.codex_args.len() + 1);
    command_args.push(OsString::from("update"));
    command_args.extend(args.codex_args);
    exit_with_status(run_codex_direct_passthrough(command_args)?)
}

pub(crate) fn exit_with_status(status: ExitStatus) -> Result<()> {
    std::process::exit(status.code().unwrap_or(1));
}

pub(crate) fn handle_caveman_dry_run(args: CavemanArgs) -> Result<()> {
    let (rtk_enabled, super_optimizer_overlay, memory_prefix_enabled, codex_args) =
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
    let memory_enabled =
        memory_prefix_enabled || args.memory_backend == crate::SuperMemoryBackend::Mem0;
    let memory_backend = match (memory_enabled, args.memory_backend) {
        (false, _) => "disabled",
        (true, crate::SuperMemoryBackend::Sqlite) => "local sqlite",
        (true, crate::SuperMemoryBackend::Mem0) => "managed Mem0 Docker (would start)",
    };
    let mut extra_report = format!(
        "Prodex overlay: rtk={}; super={}\nMemory backend: {}",
        if rtk_enabled { "enabled" } else { "disabled" },
        if super_optimizer_overlay {
            "enabled"
        } else {
            "disabled"
        },
        memory_backend,
    );
    if super_optimizer_overlay {
        let paths = crate::AppPaths::discover()?;
        let memory_mode = if memory_enabled {
            SuperMemoryStatusMode::Selected(args.memory_backend)
        } else {
            SuperMemoryStatusMode::Disabled
        };
        let statuses =
            collect_super_tool_statuses_with_memory_mode(&paths, presidio_enabled, memory_mode);
        extra_report.push('\n');
        extra_report.push_str(&render_super_tool_statuses(&statuses));
    }
    print_runtime_launch_dry_run(
        "caveman",
        request,
        RuntimeLaunchDryRunChild::Caveman { codex_args },
        Some(&extra_report),
    )
}

pub(crate) fn print_runtime_launch_dry_run(
    flow: &str,
    request: RuntimeLaunchRequest<'_>,
    child: RuntimeLaunchDryRunChild,
    extra_report: Option<&str>,
) -> Result<()> {
    let upstream_no_proxy = request.upstream_no_proxy;
    let presidio_redaction_enabled = request.presidio_redaction_enabled;
    let prepared = prepare_runtime_launch_dry_run(request)?;
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
    );
    if let Some(extra_report) = extra_report {
        output.push_str(extra_report);
        output.push('\n');
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
                runtime_launch_openai_spark_context_codex_args(codex_home, &codex_args);
            let codex_args = profile_openai_compatible_codex_args(codex_home, &codex_args);
            let codex_args = preview_local_provider_catalog_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_external_provider_catalog_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_deepseek_provider_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_gemini_provider_codex_args(codex_home, &codex_args)?;
            Ok(RuntimeLaunchDryRunChild::Codex { codex_args })
        }
        RuntimeLaunchDryRunChild::Caveman { codex_args } => {
            let codex_args =
                runtime_launch_openai_spark_context_codex_args(codex_home, &codex_args);
            let codex_args = profile_openai_compatible_codex_args(codex_home, &codex_args);
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
        realtime_ws_base_url: proxy.realtime_ws_base_url.as_deref(),
        realtime_ws_model: proxy.realtime_ws_model.as_deref(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
