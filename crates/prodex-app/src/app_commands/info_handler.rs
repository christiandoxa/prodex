use anyhow::Result;
use chrono::Local;
use crossterm::terminal;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use terminal_ui::{
    format_field_lines_with_layout, text_width, tui_border_style, tui_connected_header_block,
    tui_secondary_style, tui_title_style,
};

use crate::{
    AppPaths, AppState, AppStateIoExt, InfoArgs, InfoQuotaWindow, RuntimeConfig, codex_cli_version,
    collect_active_runtime_log_paths, collect_info_quota_aggregate,
    collect_info_runtime_load_summary, collect_info_token_usage_summary,
    collect_recent_runtime_log_paths, collect_runtime_broker_metrics_targets,
    collect_runtime_tuning_snapshot, estimate_info_runway, format_audit_logs_summary,
    format_info_codex_version, format_info_load_summary, format_info_pool_remaining,
    format_info_process_summary, format_info_prodex_version,
    format_info_provider_capabilities_summary, format_info_provider_summary,
    format_info_quota_data_summary, format_info_runway, format_info_token_usage_summary,
    format_runtime_broker_metrics_targets, format_runtime_logs_summary,
    format_runtime_policy_summary, format_runtime_proxy_contract_summary,
    format_runtime_proxy_preset, format_runtime_tuning_budgets, format_runtime_tuning_transport,
    format_runtime_tuning_workers, format_secret_backend_summary, print_panel,
    runtime_policy_summary, try_collect_prodex_processes,
};

pub(crate) fn handle_info(args: InfoArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let policy_summary = runtime_policy_summary()?;
    let runtime_config = RuntimeConfig::from_env_policy_and_cli(&paths)?;
    let runtime_tuning = collect_runtime_tuning_snapshot(&runtime_config);
    let runtime_metrics_targets = collect_runtime_broker_metrics_targets(&paths);
    let now = Local::now().timestamp();
    let prodex_version_summary = format_info_prodex_version(&paths)?;
    let codex_version_summary = format_info_codex_version(&paths, codex_cli_version())?;
    let quota = collect_info_quota_aggregate(&paths, &state, now);
    let process_result = try_collect_prodex_processes();
    let process_summary = match &process_result {
        Ok(processes) => format_info_process_summary(processes),
        Err(err) => format!("Unavailable ({err})"),
    };
    let processes = process_result.unwrap_or_default();
    let runtime_logs = collect_active_runtime_log_paths(&processes);
    let runtime_load = collect_info_runtime_load_summary(&runtime_logs, now);
    let runtime_process_count = processes.iter().filter(|process| process.runtime).count();
    let five_hour_runway = estimate_info_runway(
        &runtime_load.observations,
        InfoQuotaWindow::FiveHour,
        quota.five_hour_pool_remaining,
        now,
    );
    let weekly_runway = estimate_info_runway(
        &runtime_load.observations,
        InfoQuotaWindow::Weekly,
        quota.weekly_pool_remaining,
        now,
    );

    let token_summary = args
        .tokens
        .then(|| collect_info_token_usage_summary(&collect_recent_runtime_log_paths(8)));

    let mut fields = vec![
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
        (
            "Providers".to_string(),
            format_info_provider_summary(&state.profiles),
        ),
        (
            "Provider routes".to_string(),
            format_info_provider_capabilities_summary(&state.profiles),
        ),
        (
            "Runtime policy".to_string(),
            format_runtime_policy_summary(policy_summary.as_ref()),
        ),
        ("Runtime preset".to_string(), format_runtime_proxy_preset()),
        (
            "Runtime proxy contract".to_string(),
            format_runtime_proxy_contract_summary(),
        ),
        (
            "Secret backend".to_string(),
            format_secret_backend_summary(),
        ),
        ("Runtime logs".to_string(), format_runtime_logs_summary()),
        ("Audit logs".to_string(), format_audit_logs_summary()),
        (
            "Runtime metrics".to_string(),
            format_runtime_broker_metrics_targets(&runtime_metrics_targets),
        ),
        (
            "Runtime workers".to_string(),
            format_runtime_tuning_workers(&runtime_tuning),
        ),
        (
            "Runtime budgets".to_string(),
            format_runtime_tuning_budgets(&runtime_tuning),
        ),
        (
            "Runtime transport".to_string(),
            format_runtime_tuning_transport(&runtime_tuning),
        ),
        ("Prodex version".to_string(), prodex_version_summary),
        ("Codex version".to_string(), codex_version_summary),
        ("Prodex processes".to_string(), process_summary),
        (
            "Recent load".to_string(),
            format_info_load_summary(&runtime_load, runtime_process_count),
        ),
        (
            "Codex quota data".to_string(),
            format_info_quota_data_summary(&quota),
        ),
        (
            "Codex 5h pool".to_string(),
            format_info_pool_remaining(
                quota.five_hour_pool_remaining,
                quota.five_hour_profiles_with_data,
                quota.earliest_five_hour_reset_at,
            ),
        ),
        (
            "Codex weekly pool".to_string(),
            format_info_pool_remaining(
                quota.weekly_pool_remaining,
                quota.weekly_profiles_with_data,
                quota.earliest_weekly_reset_at,
            ),
        ),
        (
            "Codex 5h runway".to_string(),
            format_info_runway(
                quota.five_hour_profiles_with_data,
                quota.five_hour_pool_remaining,
                quota.earliest_five_hour_reset_at,
                five_hour_runway.as_ref(),
                now,
            ),
        ),
        (
            "Codex weekly runway".to_string(),
            format_info_runway(
                quota.weekly_profiles_with_data,
                quota.weekly_pool_remaining,
                quota.earliest_weekly_reset_at,
                weekly_runway.as_ref(),
                now,
            ),
        ),
    ];
    if let Some(token_summary) = token_summary.as_ref() {
        fields.push((
            "Token usage".to_string(),
            format_info_token_usage_summary(token_summary),
        ));
    }
    print_info_panel(&fields)?;
    Ok(())
}

fn print_info_panel(fields: &[(String, String)]) -> Result<()> {
    let height = info_panel_tui_height(fields);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        print_panel("Info", fields)?;
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());

        let header = Paragraph::new(Line::from(vec![
            Span::styled("Prodex Info", tui_title_style()),
            Span::raw("  "),
            Span::styled(format!("{} field(s)", fields.len()), tui_secondary_style()),
        ]))
        .block(tui_connected_header_block(tui_border_style()));
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(info_panel_tui_text(fields))
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

fn info_panel_tui_height(fields: &[(String, String)]) -> u16 {
    let terminal_width = terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(80);
    info_panel_tui_height_for_width(fields, terminal_width)
}

fn info_panel_tui_height_for_width(fields: &[(String, String)], width: usize) -> u16 {
    let label_width = info_panel_tui_label_width(fields);
    let content_width = width.saturating_sub(2).max(1);
    let rows = fields
        .iter()
        .map(|(label, value)| {
            format_field_lines_with_layout(label, value, content_width, label_width).len()
        })
        .sum::<usize>()
        .saturating_add(4)
        .max(8);
    rows.min(usize::from(u16::MAX)) as u16
}

fn info_panel_tui_text(fields: &[(String, String)]) -> Text<'static> {
    let label_width = info_panel_tui_label_width(fields);
    let mut lines = Vec::new();
    for (label, value) in fields {
        let color = info_panel_value_color(label, value);
        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "{label}{} ",
                    " ".repeat(label_width.saturating_sub(text_width(label)))
                ),
                tui_secondary_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(value.clone(), Style::default().fg(color)),
        ]));
    }
    Text::from(lines)
}

fn info_panel_tui_label_width(fields: &[(String, String)]) -> usize {
    fields
        .iter()
        .map(|(label, _)| text_width(label))
        .max()
        .unwrap_or(0)
        .min(24)
}

fn info_panel_value_color(label: &str, value: &str) -> Color {
    let lower = value.to_ascii_lowercase();
    if lower.contains("error")
        || lower.contains("missing")
        || lower.contains("unavailable")
        || lower.contains("exhausted")
        || lower.contains("critical")
        || lower.contains("thin")
        || lower.contains("warning")
    {
        Color::Red
    } else if lower.contains("ready")
        || lower.contains("healthy")
        || lower.contains("up to date")
        || label == "Active profile"
    {
        Color::Green
    } else if label.contains("Runtime") || label.contains("Codex") {
        Color::Cyan
    } else {
        Color::Reset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_panel_tui_text_contains_fields() {
        let fields = vec![
            ("Active profile".to_string(), "main".to_string()),
            ("Runtime logs".to_string(), "ready".to_string()),
        ];
        let text = format!("{:?}", info_panel_tui_text(&fields));
        assert!(text.contains("Active profile"));
        assert!(text.contains("main"));
        assert!(text.contains("Runtime logs"));
    }

    #[test]
    fn info_panel_value_color_highlights_status() {
        assert_eq!(info_panel_value_color("x", "missing file"), Color::Red);
        assert_eq!(info_panel_value_color("x", "critical load"), Color::Red);
        assert_eq!(
            info_panel_value_color("Active profile", "main"),
            Color::Green
        );
    }

    #[test]
    fn info_panel_height_does_not_clip_fields_to_terminal_height() {
        let fields = (0..23)
            .map(|index| (format!("Field {index}"), "value ".repeat(10)))
            .collect::<Vec<_>>();

        let wide_height = info_panel_tui_height_for_width(&fields, 100);
        assert!(wide_height > 24);
        assert!(info_panel_tui_height_for_width(&fields, 20) > wide_height);
    }
}
