use crate::print::{print_stdout_line, print_wrapped_stderr};
use crate::terminal::current_cli_width;
use crate::text::{fit_cell, text_width, wrap_text};
use crate::{CLI_LABEL_WIDTH, CLI_MAX_LABEL_WIDTH, CLI_MIN_LABEL_WIDTH};
use std::io;

pub fn section_header(title: &str) -> String {
    section_header_with_width(title, current_cli_width())
}

pub fn section_header_with_width(title: &str, total_width: usize) -> String {
    let prefix = format!("[ {title} ] ");
    let width = text_width(&prefix);
    if width >= total_width {
        return fit_cell(&prefix, total_width);
    }
    format!("{prefix}{}", "=".repeat(total_width - width))
}

pub fn panel_label_width(fields: &[(String, String)], total_width: usize) -> usize {
    let longest = fields
        .iter()
        .map(|(label, _)| text_width(label) + 1)
        .max()
        .unwrap_or(CLI_LABEL_WIDTH);
    let max_by_width = total_width.saturating_sub(20).clamp(1, CLI_MAX_LABEL_WIDTH);
    let preferred_cap = (total_width / 4).clamp(1, CLI_MAX_LABEL_WIDTH);
    let cap = max_by_width.min(preferred_cap);
    longest.clamp(CLI_MIN_LABEL_WIDTH.min(cap), cap)
}

pub fn format_field_lines_with_layout(
    label: &str,
    value: &str,
    total_width: usize,
    label_width: usize,
) -> Vec<String> {
    let label = format!("{label}:");
    let value_width = total_width.saturating_sub(label_width + 1).max(1);
    wrap_text(value, value_width)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let field_label = if index == 0 { label.as_str() } else { "" };
            let padding = " ".repeat(label_width.saturating_sub(text_width(field_label)));
            format!("{field_label}{padding} {line}")
        })
        .collect()
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

pub fn print_panel(title: &str, fields: &[(String, String)]) -> io::Result<()> {
    for line in panel_lines_with_layout(title, fields, current_cli_width()) {
        print_stdout_line(&line)?;
    }
    Ok(())
}

pub fn render_panel(title: &str, fields: &[(String, String)]) -> String {
    panel_lines_with_layout(title, fields, current_cli_width()).join("\n")
}

pub fn render_text_panel(title: &str, body: &str) -> String {
    let mut lines = vec![section_header(title)];
    lines.extend(body.lines().map(str::to_string));
    lines.join("\n")
}

pub fn print_stderr_panel(title: &str, messages: &[String]) -> io::Result<()> {
    print_wrapped_stderr(&section_header(title))?;
    for message in messages {
        print_wrapped_stderr(message)?;
    }
    Ok(())
}
