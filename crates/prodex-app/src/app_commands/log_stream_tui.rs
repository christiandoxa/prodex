use super::log::{LogStreamItem, TranscriptEvent};
use super::log_format::render_text_body;
use super::log_tui::{LogTuiHeaderDetail, LogTuiState, contains_ignore_ascii_case, visible_text};
use super::log_upstream_payload::{UpstreamPayloadEvent, render_upstream_payload_lines};
use prodex_app_reports::InfoTokenUsageEvent;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::collections::VecDeque;
use terminal_ui::{
    text_width, tui_accent_style, tui_border_style, tui_metric_style, tui_muted_style,
    tui_primary_style, tui_title_style, tui_tool_style,
};

pub(super) fn log_snapshot_tui_height(
    transcript: Option<&TranscriptEvent>,
    upstream_payload: Option<&UpstreamPayloadEvent>,
    token_usage: Option<&InfoTokenUsageEvent>,
) -> u16 {
    let body_rows = if transcript.is_none() && upstream_payload.is_none() && token_usage.is_none() {
        1
    } else {
        let transcript_rows = transcript
            .map(|event| render_text_body(&event.text, 100).len().saturating_add(2))
            .unwrap_or(0);
        let upstream_rows = upstream_payload
            .map(|event| {
                render_upstream_payload_lines(&event.payload, 100)
                    .len()
                    .saturating_add(3)
            })
            .unwrap_or(0);
        let usage_rows = token_usage.map(|_| 3).unwrap_or(0);
        transcript_rows
            .saturating_add(upstream_rows)
            .saturating_add(usage_rows)
            .saturating_add(1)
    };
    body_rows.saturating_add(4).clamp(8, 28) as u16
}

pub(super) fn render_log_snapshot_tui(
    frame: &mut ratatui::Frame<'_>,
    items: &VecDeque<LogStreamItem>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(frame.area());

    let header = Paragraph::new(Line::from(vec![
        Span::styled("Prodex Log Snapshot", tui_title_style()),
        Span::raw("  "),
        Span::styled(format!("{} event(s)", items.len()), tui_muted_style()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(tui_border_style()),
    );
    frame.render_widget(header, chunks[0]);

    let body = Paragraph::new(log_stream_tui_text(
        items,
        usize::from(chunks[1].height),
        usize::from(chunks[1].width).saturating_sub(2).max(20),
    ))
    .block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
            .border_style(tui_border_style()),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[1]);
}

pub(super) fn render_log_stream_tui(
    frame: &mut ratatui::Frame<'_>,
    items: &VecDeque<LogStreamItem>,
    state: &LogTuiState,
    header_detail: Option<&LogTuiHeaderDetail>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let matches = matching_log_stream_items(items, state.query()).len();
    let count = match state.query() {
        Some(_) => format!("{matches}/{} match(es)", items.len()),
        None => format!("{} event(s)", items.len()),
    };

    let title = "Prodex Log Stream";
    let count_width = text_width(&count);
    let mut header_spans = vec![
        Span::styled(title, tui_title_style()),
        Span::raw("  "),
        Span::styled(count, tui_muted_style()),
    ];
    if let Some(detail) = header_detail {
        let header_width = usize::from(chunks[0].width).saturating_sub(2);
        let used = text_width(title) + 2 + count_width;
        let detail_width = header_width.saturating_sub(used + 2);
        if detail_width > 0 {
            header_spans.push(Span::raw("  "));
            header_spans.push(Span::styled(
                detail.render(detail_width),
                tui_primary_style(),
            ));
        }
    }
    let header = Paragraph::new(Line::from(header_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(tui_border_style()),
    );
    frame.render_widget(header, chunks[0]);

    let body = Paragraph::new(log_stream_tui_text_for_view(
        items,
        usize::from(chunks[1].height),
        usize::from(chunks[1].width).saturating_sub(2).max(20),
        state.query(),
        state.scroll_from_bottom(),
    ))
    .block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(tui_border_style()),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[1]);

    let footer = Paragraph::new(Line::styled(
        state.footer_text("live transcript + token usage | q quit"),
        tui_title_style(),
    ))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(tui_border_style()),
    );
    frame.render_widget(footer, chunks[2]);
}

pub(super) fn log_stream_tui_text(
    items: &VecDeque<LogStreamItem>,
    max_lines: usize,
    body_width: usize,
) -> Text<'static> {
    log_stream_tui_text_for_view(items, max_lines, body_width, None, 0)
}

fn log_stream_tui_text_for_view(
    items: &VecDeque<LogStreamItem>,
    max_lines: usize,
    body_width: usize,
    query: Option<&str>,
    scroll_from_bottom: usize,
) -> Text<'static> {
    if items.is_empty() {
        return Text::from(Line::styled(
            "Waiting for transcript or token usage events...",
            tui_muted_style(),
        ));
    }
    let matching = matching_log_stream_items(items, query);
    if matching.is_empty() {
        return Text::from(Line::styled(
            "No log events match search.",
            tui_muted_style(),
        ));
    }

    let mut lines = Vec::new();
    for item in matching {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        push_log_stream_item_lines(&mut lines, item, body_width);
    }
    visible_text(lines, max_lines, scroll_from_bottom)
}

fn matching_log_stream_items<'a>(
    items: &'a VecDeque<LogStreamItem>,
    query: Option<&str>,
) -> Vec<&'a LogStreamItem> {
    items
        .iter()
        .filter(|item| query.is_none_or(|query| log_stream_item_matches(item, query)))
        .collect()
}

fn log_stream_item_matches(item: &LogStreamItem, query: &str) -> bool {
    let text = match item {
        LogStreamItem::Transcript(event) => {
            format!("{} {} {}", event.timestamp, event.source, event.text)
        }
        LogStreamItem::TokenUsage(event) => format!(
            "{} {} {} {} {} {} {} {} {}",
            event.timestamp,
            event
                .request
                .map(|value| value.to_string())
                .unwrap_or_default(),
            event.transport,
            event.profile,
            event.source,
            event.input_tokens,
            event.cached_input_tokens,
            event.output_tokens,
            event.reasoning_tokens
        ),
        LogStreamItem::UpstreamPayload(event) => format!(
            "{} {} {} {} {} {} {} {} {}",
            event.timestamp,
            event
                .request
                .map(|value| value.to_string())
                .unwrap_or_default(),
            event.transport,
            event.profile,
            event.route,
            event.bytes,
            event.logged_bytes,
            event.truncated,
            event.payload
        ),
    };
    contains_ignore_ascii_case(&text, query)
}

fn push_log_stream_item_lines(
    lines: &mut Vec<Line<'static>>,
    item: &LogStreamItem,
    body_width: usize,
) {
    match item {
        LogStreamItem::Transcript(event) => {
            lines.push(Line::from(vec![
                Span::styled(event.timestamp.clone(), tui_muted_style()),
                Span::raw(" "),
                Span::styled(
                    format!("stream {}", event.source),
                    Style::default()
                        .fg(log_stream_source_color(&event.source))
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            for line in render_text_body(&event.text, body_width) {
                lines.push(Line::styled(line, tui_primary_style()));
            }
        }
        LogStreamItem::TokenUsage(event) => {
            let request = event
                .request
                .map(|request| request.to_string())
                .unwrap_or_else(|| "-".to_string());
            lines.push(Line::from(vec![
                Span::styled(event.timestamp.clone(), tui_muted_style()),
                Span::raw(" "),
                Span::styled(
                    "stream usage",
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("profile ", tui_muted_style()),
                Span::styled(event.profile.clone(), tui_primary_style()),
                Span::styled(" request ", tui_muted_style()),
                Span::styled(request, tui_primary_style()),
                Span::styled(" transport ", tui_muted_style()),
                Span::styled(event.transport.clone(), tui_primary_style()),
                Span::styled(" source ", tui_muted_style()),
                Span::styled(event.source.clone(), tui_primary_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("sent ", tui_muted_style()),
                Span::styled(event.input_tokens.to_string(), tui_metric_style()),
                Span::styled(" cached ", tui_muted_style()),
                Span::styled(event.cached_input_tokens.to_string(), tui_accent_style()),
                Span::styled(" received ", tui_muted_style()),
                Span::styled(event.output_tokens.to_string(), tui_metric_style()),
                Span::styled(" reasoning ", tui_muted_style()),
                Span::styled(event.reasoning_tokens.to_string(), tui_tool_style()),
            ]));
        }
        LogStreamItem::UpstreamPayload(event) => {
            let request = event
                .request
                .map(|request| request.to_string())
                .unwrap_or_else(|| "-".to_string());
            lines.push(Line::from(vec![
                Span::styled(event.timestamp.clone(), tui_muted_style()),
                Span::raw(" "),
                Span::styled(
                    "stream payload",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("profile ", tui_muted_style()),
                Span::styled(event.profile.clone(), tui_primary_style()),
                Span::styled(" request ", tui_muted_style()),
                Span::styled(request, tui_primary_style()),
                Span::styled(" transport ", tui_muted_style()),
                Span::styled(event.transport.clone(), tui_primary_style()),
                Span::styled(" route ", tui_muted_style()),
                Span::styled(event.route.clone(), tui_primary_style()),
            ]));
            for line in render_upstream_payload_lines(&event.payload, body_width) {
                lines.push(Line::styled(line, tui_primary_style()));
            }
        }
    }
}

fn log_stream_source_color(source: &str) -> Color {
    match source {
        "user" => Color::Green,
        "assistant" => Color::Cyan,
        "reasoning" => Color::Yellow,
        "turn-context" | "session-context" => Color::Blue,
        "prompt-engineering" => Color::LightBlue,
        "tool-output" => Color::Magenta,
        source if source.starts_with("tool-call:") => Color::Magenta,
        _ => Color::Reset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(text: Text<'static>) -> String {
        text.lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn stream_text_can_filter_and_scroll() {
        let items = VecDeque::from([
            LogStreamItem::Transcript(TranscriptEvent {
                timestamp: "2026-07-06 10:00:00".to_string(),
                source: "assistant".to_string(),
                text: "alpha first".to_string(),
            }),
            LogStreamItem::Transcript(TranscriptEvent {
                timestamp: "2026-07-06 10:00:01".to_string(),
                source: "assistant".to_string(),
                text: "beta second".to_string(),
            }),
        ]);

        let filtered = rendered(log_stream_tui_text_for_view(
            &items,
            20,
            80,
            Some("beta"),
            0,
        ));
        assert!(!filtered.contains("alpha first"));
        assert!(filtered.contains("beta second"));

        let scrolled = rendered(log_stream_tui_text_for_view(
            &items,
            2,
            80,
            None,
            usize::MAX,
        ));
        assert!(scrolled.contains("alpha first"));
        assert!(!scrolled.contains("beta second"));
    }
}
