use anyhow::Result;
use crossterm::terminal;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::{
    AppPaths, AppState, AppStateIoExt, CleanupArgs, CleanupOlderThan,
    ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS, ProdexCleanupOptions,
    perform_prodex_cleanup_with_options, print_panel, runtime_proxy_log_dir,
};
use terminal_ui::{text_width, tui_border_style, tui_secondary_style, tui_title_style};

pub(crate) fn handle_cleanup(args: CleanupArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let runtime_log_dir = runtime_proxy_log_dir();
    let cleanup_options = cleanup_options_from_args(&args);
    let summary = perform_prodex_cleanup_with_options(&paths, &mut state, cleanup_options)?;

    let fields = vec![
        ("Prodex root".to_string(), paths.root.display().to_string()),
        (
            "Orphan threshold".to_string(),
            cleanup_orphan_threshold_label(
                &args,
                cleanup_options.orphan_managed_profile_retention_seconds,
            ),
        ),
        (
            "Duplicate profiles".to_string(),
            summary.duplicate_profiles_removed.to_string(),
        ),
        (
            "Duplicate managed homes".to_string(),
            summary.duplicate_managed_profile_homes_removed.to_string(),
        ),
        (
            "Runtime logs".to_string(),
            format!(
                "{} removed from {}",
                summary.runtime_logs_removed,
                runtime_log_dir.display()
            ),
        ),
        (
            "Runtime pointer".to_string(),
            if summary.stale_runtime_log_pointer_removed > 0 {
                "removed stale latest-pointer file".to_string()
            } else {
                "clean".to_string()
            },
        ),
        (
            "Temp login homes".to_string(),
            summary.stale_login_dirs_removed.to_string(),
        ),
        (
            "Orphan managed homes".to_string(),
            summary.orphan_managed_profile_dirs_removed.to_string(),
        ),
        (
            "Transient root files".to_string(),
            summary.transient_root_files_removed.to_string(),
        ),
        (
            "Stale root temp files".to_string(),
            summary.stale_root_temp_files_removed.to_string(),
        ),
        (
            "Chat history".to_string(),
            "left untouched (Codex-managed)".to_string(),
        ),
        (
            "Dead broker leases".to_string(),
            summary.dead_runtime_broker_leases_removed.to_string(),
        ),
        (
            "Dead broker registries".to_string(),
            summary.dead_runtime_broker_registries_removed.to_string(),
        ),
        (
            "Total removed".to_string(),
            summary.total_removed().to_string(),
        ),
    ];
    print_cleanup_panel(&fields)?;
    Ok(())
}

fn print_cleanup_panel(fields: &[(String, String)]) -> Result<()> {
    let height = cleanup_tui_height(fields);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        print_panel("Cleanup", fields);
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::styled("Prodex Cleanup", tui_title_style())).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(tui_border_style()),
        );
        frame.render_widget(header, chunks[0]);
        let body = Paragraph::new(cleanup_tui_text(fields))
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

fn cleanup_tui_height(fields: &[(String, String)]) -> u16 {
    let rows = fields.len().saturating_add(5).max(8);
    let terminal_height = terminal::size()
        .map(|(_, height)| usize::from(height))
        .unwrap_or(24);
    rows.min(terminal_height).max(1) as u16
}

fn cleanup_tui_text(fields: &[(String, String)]) -> Text<'static> {
    let label_width = fields
        .iter()
        .map(|(label, _)| text_width(label))
        .max()
        .unwrap_or(0)
        .min(24);
    Text::from(
        fields
            .iter()
            .map(|(label, value)| {
                Line::from(vec![
                    Span::styled(
                        format!(
                            "{label}{} ",
                            " ".repeat(label_width.saturating_sub(text_width(label)))
                        ),
                        tui_secondary_style().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        value.clone(),
                        Style::default().fg(cleanup_value_color(value)),
                    ),
                ])
            })
            .collect::<Vec<_>>(),
    )
}

fn cleanup_value_color(value: &str) -> Color {
    if value.starts_with('0') || value.contains("clean") || value.contains("left untouched") {
        Color::Green
    } else if value.contains("removed") || value.parse::<usize>().unwrap_or(0) > 0 {
        Color::Red
    } else {
        Color::Reset
    }
}

fn cleanup_options_from_args(args: &CleanupArgs) -> ProdexCleanupOptions {
    ProdexCleanupOptions {
        orphan_managed_profile_retention_seconds: if args.aggressive {
            0
        } else {
            args.older_than
                .map(CleanupOlderThan::seconds)
                .unwrap_or(ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS)
        },
    }
}

fn cleanup_orphan_threshold_label(args: &CleanupArgs, seconds: i64) -> String {
    let mode = if args.aggressive {
        "aggressive"
    } else if args.older_than.is_some() {
        "explicit"
    } else {
        "default"
    };
    format!("{} ({mode})", cleanup_duration_label(seconds))
}

fn cleanup_duration_label(seconds: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;

    if seconds % DAY == 0 {
        return format!("{}d", seconds / DAY);
    }
    if seconds % HOUR == 0 {
        return format!("{}h", seconds / HOUR);
    }
    if seconds % MINUTE == 0 {
        return format!("{}m", seconds / MINUTE);
    }
    format!("{seconds}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_tui_text_contains_fields() {
        let fields = vec![
            (
                "Runtime logs".to_string(),
                "2 removed from /tmp".to_string(),
            ),
            ("Chat history".to_string(), "left untouched".to_string()),
        ];
        let text = format!("{:?}", cleanup_tui_text(&fields));
        assert!(text.contains("Runtime logs"));
        assert!(text.contains("left untouched"));
    }

    #[test]
    fn cleanup_value_color_highlights_removed_counts() {
        assert_eq!(cleanup_value_color("0"), Color::Green);
        assert_eq!(cleanup_value_color("2"), Color::Red);
        assert_eq!(
            cleanup_value_color("removed stale latest-pointer file"),
            Color::Red
        );
    }
}
