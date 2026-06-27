pub(crate) use prodex_update_notice::*;

use anyhow::{Context, Result};
use prodex_cli::Commands;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use terminal_ui::{
    print_stderr_panel, tui_border_style, tui_hint_style, tui_primary_style, tui_secondary_style,
    tui_success_style, tui_title_style,
};

pub(crate) fn show_update_notice_if_available(command: &Commands) -> Result<()> {
    if !prodex_update_notice::should_emit_update_notice(command) {
        return Ok(());
    }

    let paths = prodex_core::AppPaths::discover()?;
    let prodex_update_notice::ProdexVersionStatus::UpdateAvailable(latest_version) =
        prodex_update_notice::prodex_version_status(&paths)?
    else {
        return Ok(());
    };

    let current_version = prodex_update_notice::current_prodex_version();
    let update_command = prodex_update_notice::prodex_update_command_for_version(&latest_version);
    if print_update_notice_tui(current_version, &latest_version, &update_command).is_err() {
        print_stderr_panel(
            "Update Available",
            &[
                format!(
                    "A newer prodex release is available: {} -> {}",
                    current_version, latest_version
                ),
                format!("Update with: {update_command}"),
            ],
        );
    }
    Ok(())
}

fn print_update_notice_tui(
    current_version: &str,
    latest_version: &str,
    update_command: &str,
) -> Result<()> {
    let Some(mut terminal) = crate::try_inline_stderr_terminal(7) else {
        anyhow::bail!("stderr is not an inline-capable terminal");
    };
    terminal
        .draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(frame.area());
            let header = Paragraph::new(Line::from(vec![
                Span::styled("Prodex Update", tui_title_style()),
                Span::raw("  "),
                Span::styled("available", tui_hint_style()),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(tui_border_style()),
            );
            frame.render_widget(header, chunks[0]);

            let body = Paragraph::new(update_notice_tui_lines(
                current_version,
                latest_version,
                update_command,
            ))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
            frame.render_widget(body, chunks[1]);
        })
        .context("failed to draw update notice TUI")?;
    let _ = terminal.show_cursor();
    Ok(())
}

fn update_notice_tui_lines(
    current_version: &str,
    latest_version: &str,
    update_command: &str,
) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("Current ", tui_secondary_style()),
            Span::styled(current_version.to_string(), tui_primary_style()),
            Span::styled(" Latest ", tui_secondary_style()),
            Span::styled(latest_version.to_string(), tui_success_style()),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Update  ", tui_secondary_style()),
            Span::styled(update_command.to_string(), tui_hint_style()),
        ]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_notice_tui_lines_contain_versions_and_command() {
        let rendered = update_notice_tui_lines("0.1.0", "0.2.0", "npm install prodex")
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("0.1.0"));
        assert!(rendered.contains("0.2.0"));
        assert!(rendered.contains("npm install prodex"));
    }
}
