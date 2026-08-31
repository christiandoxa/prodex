use super::PublicMcpEndpoint;
use base64::Engine;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::io::{self, Write};
use terminal_ui::{chunk_token, text_width};

pub(super) fn labeled_value_lines(
    label: &str,
    value: &str,
    width: usize,
    label_style: Style,
    value_style: Style,
) -> Vec<Line<'static>> {
    if width == 0 {
        return Vec::new();
    }

    let prefix = format!("{label}: ");
    let prefix_width = text_width(&prefix);
    if prefix_width < width {
        let value_width = width - prefix_width;
        let indent = " ".repeat(prefix_width);
        let mut chunks = chunk_token(value, value_width).into_iter();
        let first = chunks.next().unwrap_or_default();
        let mut lines = vec![Line::from(vec![
            Span::styled(prefix, label_style),
            Span::styled(first, value_style),
        ])];
        lines.extend(chunks.map(|chunk| {
            Line::from(vec![
                Span::raw(indent.clone()),
                Span::styled(chunk, value_style),
            ])
        }));
        return lines;
    }

    let label_lines = chunk_token(&prefix, width)
        .into_iter()
        .map(|chunk| Line::from(Span::styled(chunk, label_style)))
        .collect::<Vec<_>>();
    let indent_width = width.min(2);
    let value_width = width.saturating_sub(indent_width).max(1);
    let indent = " ".repeat(width - value_width);
    label_lines
        .into_iter()
        .chain(chunk_token(value, value_width).into_iter().map(|chunk| {
            Line::from(vec![
                Span::raw(indent.clone()),
                Span::styled(chunk, value_style),
            ])
        }))
        .collect()
}

pub(super) fn text_lines(value: &str, width: usize, style: Style) -> Vec<Line<'static>> {
    terminal_ui::wrap_text(value, width)
        .into_iter()
        .map(|line| Line::styled(line, style))
        .collect()
}

pub(super) fn is_stop_key(key: KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc
    ) || (key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c' | 'C')))
}

pub(super) fn copy_public_url_to_clipboard(public_url: &PublicMcpEndpoint) -> io::Result<()> {
    copy_public_url_to_clipboard_with(public_url, write_clipboard_escape)
}

pub(super) fn copy_public_url_to_clipboard_with(
    public_url: &PublicMcpEndpoint,
    write_clipboard: impl FnOnce(&[u8]) -> io::Result<()>,
) -> io::Result<()> {
    write_clipboard(public_url.as_str().as_bytes())
}

fn write_clipboard_escape(bytes: &[u8]) -> io::Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let mut stderr = io::stderr().lock();
    stderr.write_all(b"\x1b]52;c;")?;
    stderr.write_all(encoded.as_bytes())?;
    stderr.write_all(b"\x07")?;
    stderr.flush()
}
