use anyhow::Result;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::{self, IsTerminal};
use std::path::Path;

pub(crate) use prodex_runtime_gemini_cli_compat::{
    GeminiSettingsSource, gemini_cli_config_home_for, gemini_settings_source_paths_for,
    gemini_settings_sources, parse_gemini_settings_json,
};

pub(crate) fn prepare_gemini_cli_compat(codex_home: &Path) -> Result<()> {
    prodex_runtime_gemini_cli_compat::prepare_gemini_cli_compat(codex_home)
}

pub(crate) fn handle_gemini_compat_refresh(args: crate::GeminiCompatRefreshArgs) -> Result<()> {
    prepare_gemini_cli_compat(&args.codex_home)?;
    print_gemini_compat_refresh_status(&args.codex_home)?;
    Ok(())
}

fn print_gemini_compat_refresh_status(codex_home: &Path) -> Result<()> {
    if !io::stdout().is_terminal() {
        println!(
            "Gemini CLI compatibility refreshed in {}",
            codex_home.display()
        );
        return Ok(());
    }

    let Some(mut terminal) = crate::try_inline_stdout_terminal(7) else {
        println!(
            "Gemini CLI compatibility refreshed in {}",
            codex_home.display()
        );
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                "Gemini CLI Compatibility",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("refreshed", Style::default().fg(Color::Green)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(gemini_compat_status_tui_text(codex_home))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    let _ = terminal.show_cursor();
    Ok(())
}

fn gemini_compat_status_tui_text(codex_home: &Path) -> Text<'static> {
    Text::from(Line::from(vec![
        Span::styled("CODEX_HOME ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            codex_home.display().to_string(),
            Style::default().fg(Color::White),
        ),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn gemini_compat_status_tui_text_contains_codex_home() {
        let codex_home = PathBuf::from("/tmp/prodex-gemini");
        let text = gemini_compat_status_tui_text(&codex_home);
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

        assert!(rendered.contains("CODEX_HOME"));
        assert!(rendered.contains("/tmp/prodex-gemini"));
    }
}
