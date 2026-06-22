pub(super) fn current_log_width() -> usize {
    terminal_ui::current_cli_width().max(60)
}

pub(super) fn render_log_block(
    timestamp: &str,
    title: &str,
    meta: &[(&str, String)],
    body: &[String],
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(render_header(timestamp, title, width));
    if !meta.is_empty() {
        lines.extend(render_meta(meta, width));
    }
    if !body.is_empty() {
        lines.extend(render_body(body, width));
    }
    lines
}

pub(super) fn render_text_body(text: &str, width: usize) -> Vec<String> {
    let body_width = width.saturating_sub(4).max(20);
    terminal_ui::wrap_text(text, body_width)
}

fn render_header(timestamp: &str, title: &str, width: usize) -> String {
    let prefix = format!("[{timestamp}] {title}");
    if terminal_ui::text_width(&prefix) >= width.saturating_sub(1) {
        return prefix;
    }
    let fill = width.saturating_sub(terminal_ui::text_width(&prefix) + 1);
    format!("{prefix} {}", "-".repeat(fill))
}

fn render_meta(meta: &[(&str, String)], width: usize) -> Vec<String> {
    let text = meta
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("  ");
    terminal_ui::wrap_text(&text, width.saturating_sub(2).max(20))
        .into_iter()
        .map(|line| format!("  {line}"))
        .collect()
}

fn render_body(body: &[String], width: usize) -> Vec<String> {
    let body_width = width.saturating_sub(4).max(20);
    let mut lines = Vec::new();
    for block in body {
        for line in terminal_ui::wrap_text(block, body_width) {
            lines.push(format!("  | {line}"));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_block_with_header_meta_and_body() {
        let lines = render_log_block(
            "2026-06-22T01:00:00Z",
            "stream assistant",
            &[
                ("profile", "main".to_string()),
                ("request", "7".to_string()),
            ],
            &["hello terminal".to_string()],
            72,
        );
        let rendered = lines.join("\n");

        assert!(rendered.contains("[2026-06-22T01:00:00Z] stream assistant"));
        assert!(rendered.contains("profile=main"));
        assert!(rendered.contains("request=7"));
        assert!(rendered.contains("| hello terminal"));
    }
}
