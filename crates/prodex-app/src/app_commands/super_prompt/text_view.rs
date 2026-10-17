//! Ratatui-only rendering for the Super free-text prompt.

use super::{
    Block, Borders, Constraint, Direction, Layout, Line, Paragraph, Span, Wrap, bounded_tui_text,
    tui_border_style, tui_connected_footer_block, tui_connected_header_block, tui_detail_style,
    tui_hint_style, tui_primary_style, tui_success_style, tui_title_style,
};

pub(super) fn render_super_text_prompt(
    frame: &mut ratatui::Frame<'_>,
    title: &str,
    value: &str,
    validation_error: Option<&str>,
    escape_returns_none: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let header = Paragraph::new(Line::from(vec![
        Span::styled("Prodex Super", tui_title_style()),
        Span::raw("  "),
        Span::styled(title, tui_detail_style()),
    ]))
    .block(tui_connected_header_block(tui_border_style()));
    frame.render_widget(header, chunks[0]);
    let value_display = bounded_tui_text(value, frame.area().width.saturating_sub(4));
    let mut lines = vec![Line::from(vec![
        Span::styled(value_display, tui_primary_style()),
        Span::styled("▌", tui_success_style()),
    ])];
    if let Some(error) = validation_error {
        lines.push(Line::from(Span::styled(
            bounded_tui_text(error, frame.area().width.saturating_sub(4)),
            tui_hint_style(),
        )));
    }
    let body = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT)
                .border_style(tui_border_style()),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(body, chunks[1]);
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("enter", tui_success_style()),
        Span::raw(" accept  "),
        Span::styled("esc", tui_hint_style()),
        Span::raw(if escape_returns_none {
            " back"
        } else {
            " cancel"
        }),
    ]))
    .block(tui_connected_footer_block(tui_border_style()));
    frame.render_widget(footer, chunks[2]);
}
