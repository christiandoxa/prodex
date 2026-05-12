use crate::print::print_stdout_line;
use crate::terminal::current_cli_width;
use crate::text::{text_width, wrap_text};
use crate::{CLI_LABEL_WIDTH, CLI_MAX_LABEL_WIDTH, CLI_MIN_LABEL_WIDTH};

pub fn section_header(title: &str) -> String {
    section_header_with_width(title, current_cli_width())
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
    for line in panel_lines_with_layout(title, fields, current_cli_width()) {
        print_stdout_line(&line);
    }
}

pub fn render_panel(title: &str, fields: &[(String, String)]) -> String {
    panel_lines_with_layout(title, fields, current_cli_width()).join("\n")
}
