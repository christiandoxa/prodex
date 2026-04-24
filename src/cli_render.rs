use super::*;

pub(super) fn section_header(title: &str) -> String {
    section_header_with_width(title, current_cli_width())
}

pub(super) fn section_header_with_width(title: &str, total_width: usize) -> String {
    let prefix = format!("[ {title} ] ");
    let width = text_width(&prefix);
    if width >= total_width {
        return prefix;
    }

    format!("{prefix}{}", "=".repeat(total_width - width))
}

pub(super) fn text_width(value: &str) -> usize {
    value.chars().count()
}

pub(super) fn fit_cell(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    if text_width(value) <= width {
        return value.to_string();
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let mut output = String::new();
    for ch in value.chars().take(width - 3) {
        output.push(ch);
    }
    output.push_str("...");
    output
}

pub(super) fn chunk_token(token: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if token.is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in token.chars() {
        current.push(ch);
        if text_width(&current) >= width {
            chunks.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

pub(super) fn wrap_text(input: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for paragraph in input.lines() {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            for piece in chunk_token(word, width) {
                if current.is_empty() {
                    current.push_str(&piece);
                } else if text_width(&current) + 1 + text_width(&piece) <= width {
                    current.push(' ');
                    current.push_str(&piece);
                } else {
                    lines.push(std::mem::take(&mut current));
                    current.push_str(&piece);
                }
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

pub(super) fn current_cli_width() -> usize {
    terminal_width_chars()
        .unwrap_or(CLI_WIDTH)
        .max(CLI_MIN_WIDTH)
}

pub(super) fn terminal_size_override_usize(env_key: &str) -> Option<usize> {
    env::var(env_key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

pub(super) fn terminal_dimensions_from_tty() -> Option<(usize, usize)> {
    let tty = fs::File::open("/dev/tty").ok()?;
    let output = Command::new("stty").arg("size").stdin(tty).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8(output.stdout).ok()?;
    let mut parts = text.split_whitespace();
    let rows = parts.next()?.parse::<usize>().ok()?;
    let cols = parts.next()?.parse::<usize>().ok()?;
    Some((rows, cols))
}

pub(super) fn terminal_width_chars() -> Option<usize> {
    terminal_size_override_usize("PRODEX_TERM_COLUMNS")
        .or_else(|| terminal_dimensions_from_tty().map(|(_, cols)| cols))
}

pub(super) fn terminal_height_lines() -> Option<usize> {
    terminal_size_override_usize("PRODEX_TERM_LINES")
        .or_else(|| terminal_size_override_usize("LINES"))
        .or_else(|| terminal_dimensions_from_tty().map(|(rows, _)| rows))
}

pub(super) struct FieldRowsBuilder {
    rows: Vec<(String, String)>,
}

impl FieldRowsBuilder {
    pub(super) fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub(super) fn push(&mut self, label: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.rows.push((label.into(), value.into()));
        self
    }

    pub(super) fn extend(&mut self, rows: impl IntoIterator<Item = (String, String)>) -> &mut Self {
        self.rows.extend(rows);
        self
    }

    pub(super) fn build(self) -> Vec<(String, String)> {
        self.rows
    }
}

pub(super) struct PanelBuilder {
    title: String,
    fields: FieldRowsBuilder,
}

impl PanelBuilder {
    pub(super) fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            fields: FieldRowsBuilder::new(),
        }
    }

    pub(super) fn push(&mut self, label: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.fields.push(label, value);
        self
    }

    pub(super) fn extend(&mut self, rows: impl IntoIterator<Item = (String, String)>) -> &mut Self {
        self.fields.extend(rows);
        self
    }

    pub(super) fn render(self) -> String {
        let fields = self.fields.build();
        render_panel(&self.title, &fields)
    }
}

pub(super) fn panel_label_width(fields: &[(String, String)], total_width: usize) -> usize {
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

pub(super) fn format_field_lines_with_layout(
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

pub(super) fn print_panel(title: &str, fields: &[(String, String)]) {
    for line in panel_lines_with_layout(title, fields, current_cli_width()) {
        print_stdout_line(&line);
    }
}

pub(super) fn render_panel(title: &str, fields: &[(String, String)]) -> String {
    panel_lines_with_layout(title, fields, current_cli_width()).join("\n")
}

pub(super) fn print_stdout_text(message: &str) {
    print!("{message}");
}

pub(super) fn print_stdout_line(message: &str) {
    println!("{message}");
}

pub(super) fn print_blank_line() {
    println!();
}

pub(super) fn print_stderr_line(message: &str) {
    eprintln!("{message}");
}

pub(super) fn print_stderr_prompt(prompt: &str) -> Result<()> {
    eprint!("{prompt}");
    io::stderr().flush().context("failed to flush prompt")
}

pub(super) fn print_wrapped_stderr(message: &str) {
    for line in wrap_text(message, current_cli_width()) {
        print_stderr_line(&line);
    }
}
