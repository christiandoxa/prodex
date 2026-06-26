use anyhow::Result;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::{self, IsTerminal};
use terminal_ui::print_panel;

pub(super) fn print_gateway_status(
    listen_addr: std::net::SocketAddr,
    provider_name: &str,
    auth_required: bool,
) {
    let fields = vec![
        ("URL".to_string(), format!("http://{listen_addr}")),
        ("Provider".to_string(), provider_name.to_string()),
        ("Auth required".to_string(), auth_required.to_string()),
        (
            "Endpoints".to_string(),
            "/v1/responses, /v1/chat/completions, /v1/embeddings, /v1/images/*, /v1/audio/*, /v1/batches, /v1/rerank, /v1/a2a, /v1/messages".to_string(),
        ),
        ("Models".to_string(), "/v1/models".to_string()),
    ];

    if !io::stdout().is_terminal() {
        println!(
            "Prodex gateway listening on http://{} provider={} auth_required={} endpoints=/v1/responses,/v1/chat/completions,/v1/embeddings,/v1/images/*,/v1/audio/*,/v1/batches,/v1/rerank,/v1/a2a,/v1/messages models=/v1/models",
            listen_addr, provider_name, auth_required
        );
        return;
    }

    if print_gateway_status_tui(&fields).is_err() {
        print_panel("Gateway", &fields);
    }
}

fn print_gateway_status_tui(fields: &[(String, String)]) -> Result<()> {
    let Some(mut terminal) = crate::try_inline_stdout_terminal(gateway_status_tui_height(fields))
    else {
        anyhow::bail!("stdout is not an inline-capable terminal");
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                "Prodex Gateway",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("running", Style::default().fg(Color::Green)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(gateway_status_tui_text(fields))
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

fn gateway_status_tui_height(fields: &[(String, String)]) -> u16 {
    (fields.len() + 4).clamp(7, 16) as u16
}

fn gateway_status_tui_text(fields: &[(String, String)]) -> Text<'static> {
    let lines = fields
        .iter()
        .map(|(label, value)| {
            Line::from(vec![
                Span::styled(
                    format!("{label:>13} "),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(value.clone(), Style::default().fg(Color::White)),
            ])
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}
