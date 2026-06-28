use crate::print::{print_stdout_line, print_wrapped_stderr};
use crate::terminal::current_cli_width;
use crate::text::{text_width, wrap_text};
use crate::{CLI_LABEL_WIDTH, CLI_MAX_LABEL_WIDTH, CLI_MIN_LABEL_WIDTH};
use ratatui::Terminal;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::{self, IsTerminal};

pub fn section_header(title: &str) -> String {
    section_header_with_width(title, current_cli_width())
}

pub fn tui_title_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn tui_secondary_style() -> Style {
    Style::default().fg(Color::Gray)
}

pub fn tui_muted_style() -> Style {
    tui_secondary_style()
}

pub fn tui_detail_style() -> Style {
    tui_secondary_style()
}

pub fn tui_primary_style() -> Style {
    Style::default()
}

pub fn tui_border_style() -> Style {
    Style::default().fg(Color::Cyan)
}

pub fn tui_hint_style() -> Style {
    Style::default().fg(Color::Cyan)
}

pub fn tui_success_style() -> Style {
    Style::default().fg(Color::Green)
}

pub fn tui_error_style() -> Style {
    Style::default().fg(Color::Red)
}

pub fn tui_metric_style() -> Style {
    tui_success_style()
}

pub fn tui_accent_style() -> Style {
    Style::default().fg(Color::LightCyan)
}

pub fn tui_tool_style() -> Style {
    Style::default().fg(Color::LightMagenta)
}

pub fn section_header_with_width(title: &str, total_width: usize) -> String {
    let prefix = format!("[ {title} ] ");
    let width = text_width(&prefix);
    if width >= total_width {
        return prefix;
    }

    format!("{prefix}{}", "=".repeat(total_width - width))
}

pub struct FieldRowsBuilder {
    rows: Vec<(String, String)>,
}

impl FieldRowsBuilder {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn push(&mut self, label: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.rows.push((label.into(), value.into()));
        self
    }

    pub fn extend(&mut self, rows: impl IntoIterator<Item = (String, String)>) -> &mut Self {
        self.rows.extend(rows);
        self
    }

    pub fn build(self) -> Vec<(String, String)> {
        self.rows
    }
}

impl Default for FieldRowsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PanelBuilder {
    title: String,
    fields: FieldRowsBuilder,
}

impl PanelBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            fields: FieldRowsBuilder::new(),
        }
    }

    pub fn push(&mut self, label: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.fields.push(label, value);
        self
    }

    pub fn extend(&mut self, rows: impl IntoIterator<Item = (String, String)>) -> &mut Self {
        self.fields.extend(rows);
        self
    }

    pub fn render(self) -> String {
        let fields = self.fields.build();
        render_panel(&self.title, &fields)
    }
}

pub fn panel_label_width(fields: &[(String, String)], total_width: usize) -> usize {
    let longest = fields
        .iter()
        .map(|(label, _)| text_width(label) + 1)
        .max()
        .unwrap_or(CLI_LABEL_WIDTH);
    let max_by_width = total_width
        .saturating_sub(20)
        .clamp(CLI_MIN_LABEL_WIDTH, CLI_MAX_LABEL_WIDTH);
    let preferred_cap = (total_width / 4).clamp(CLI_MIN_LABEL_WIDTH, CLI_MAX_LABEL_WIDTH);
    longest.clamp(CLI_MIN_LABEL_WIDTH, max_by_width.min(preferred_cap))
}

pub fn format_field_lines_with_layout(
    label: &str,
    value: &str,
    total_width: usize,
    label_width: usize,
) -> Vec<String> {
    let label = format!("{label}:");
    let value_width = total_width.saturating_sub(label_width + 1).max(1);
    let wrapped = wrap_text(value, value_width);
    let mut lines = Vec::new();

    for (index, line) in wrapped.into_iter().enumerate() {
        let field_label = if index == 0 { label.as_str() } else { "" };
        lines.push(format!(
            "{field_label:<label_w$} {line}",
            label_w = label_width
        ));
    }

    lines
}

fn panel_lines_with_layout(
    title: &str,
    fields: &[(String, String)],
    total_width: usize,
) -> Vec<String> {
    let label_width = panel_label_width(fields, total_width);
    let mut lines = vec![section_header_with_width(title, total_width)];
    for (label, value) in fields {
        lines.extend(format_field_lines_with_layout(
            label,
            value,
            total_width,
            label_width,
        ));
    }
    lines
}

pub fn print_panel(title: &str, fields: &[(String, String)]) {
    let total_width = current_cli_width();
    let lines = panel_lines_with_layout(title, fields, total_width);
    if print_panel_tui_stdout(title, &lines).is_ok() {
        return;
    }
    for line in lines {
        print_stdout_line(&line);
    }
}

pub fn render_panel(title: &str, fields: &[(String, String)]) -> String {
    panel_lines_with_layout(title, fields, current_cli_width()).join("\n")
}

pub fn render_text_panel(title: &str, body: &str) -> String {
    let mut lines = vec![section_header(title)];
    lines.extend(body.lines().map(str::to_string));
    lines.join("\n")
}

pub fn print_text_panel(title: &str, body: &str) {
    let body_lines = body.lines().map(str::to_string).collect::<Vec<_>>();
    if print_panel_tui_stdout(title, &body_lines).is_ok() {
        return;
    }
    print_stdout_line(&section_header(title));
    for line in body.lines() {
        print_stdout_line(line);
    }
}

pub fn print_stderr_panel(title: &str, messages: &[String]) {
    if print_panel_tui_stderr(title, messages).is_ok() {
        return;
    }
    print_wrapped_stderr(&section_header(title));
    for message in messages {
        print_wrapped_stderr(message);
    }
}

fn print_panel_tui_stdout(title: &str, lines: &[String]) -> anyhow::Result<()> {
    if !io::stdout().is_terminal() || std::env::var_os("CODEX_CI").is_some() {
        anyhow::bail!("stdout is not an interactive terminal");
    }

    let height = panel_inline_height(lines.len().saturating_add(2));
    let mut terminal = Terminal::with_options(
        CrosstermBackend::new(io::stdout()),
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        },
    )?;
    draw_panel_terminal(&mut terminal, title, &lines[1..])?;
    terminal.show_cursor()?;
    Ok(())
}

fn print_panel_tui_stderr(title: &str, messages: &[String]) -> anyhow::Result<()> {
    if !io::stderr().is_terminal() || std::env::var_os("CODEX_CI").is_some() {
        anyhow::bail!("stderr is not an interactive terminal");
    }

    let body_lines = messages
        .iter()
        .flat_map(|message| wrap_text(message, current_cli_width().saturating_sub(4).max(1)))
        .collect::<Vec<_>>();
    let height = panel_inline_height(body_lines.len().saturating_add(4));
    let mut terminal = Terminal::with_options(
        CrosstermBackend::new(io::stderr()),
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        },
    )?;
    draw_panel_terminal(&mut terminal, title, &body_lines)?;
    terminal.show_cursor()?;
    Ok(())
}

fn panel_inline_height(lines: usize) -> u16 {
    lines.clamp(4, usize::from(u16::MAX)) as u16
}

fn draw_panel_terminal<B: Backend>(
    terminal: &mut Terminal<B>,
    title: &str,
    body_lines: &[String],
) -> anyhow::Result<()>
where
    B::Error: Send + Sync + 'static,
{
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![Span::styled(
            title.to_string(),
            tui_title_style(),
        )]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(tui_border_style()),
        );
        frame.render_widget(header, chunks[0]);

        let rendered_body_lines = body_lines
            .iter()
            .map(|line| Line::from(line.clone()))
            .collect::<Vec<_>>();
        let body = Paragraph::new(Text::from(rendered_body_lines))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    Ok(())
}

pub fn draw_status_panel_terminal<B: Backend>(
    terminal: &mut Terminal<B>,
    title: &str,
    phase: &str,
    label: &str,
    message: &str,
) -> anyhow::Result<()>
where
    B::Error: Send + Sync + 'static,
{
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled(title.to_string(), tui_title_style()),
            Span::raw("  "),
            Span::styled(phase.to_string(), tui_secondary_style()),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(tui_border_style()),
        );
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(Line::from(vec![
            Span::styled(format!("{label} "), tui_secondary_style()),
            Span::styled(message.to_string(), tui_primary_style()),
        ]))
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .border_style(tui_border_style()),
        )
        .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    Ok(())
}
