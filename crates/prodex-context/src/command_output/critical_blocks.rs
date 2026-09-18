use super::*;

pub(super) fn push_head_tail_lines(
    output: &mut Vec<String>,
    lines: &[String],
    limit: usize,
    max_line_chars: usize,
    omitted_label: &str,
    prefix: &str,
) {
    let limit = limit.max(1);
    if lines.len() <= limit {
        for line in lines {
            output.push(format!(
                "{prefix}{}",
                truncate_command_line(line, max_line_chars)
            ));
        }
        return;
    }
    let head = limit.div_ceil(2);
    let tail = limit.saturating_sub(head);
    for line in lines.iter().take(head) {
        output.push(format!(
            "{prefix}{}",
            truncate_command_line(line, max_line_chars)
        ));
    }
    output.push(format!(
        "{prefix}[... omitted {} {omitted_label} ...]",
        lines.len().saturating_sub(head + tail)
    ));
    for line in lines.iter().skip(lines.len().saturating_sub(tail)) {
        output.push(format!(
            "{prefix}{}",
            truncate_command_line(line, max_line_chars)
        ));
    }
}

fn critical_preserve_line(line: &str) -> bool {
    count_critical_signals(line).total() > 0
}

pub(super) fn smart_truncate_command_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
    }
    let max_lines = options.max_lines.max(1);
    if lines.len() <= max_lines {
        return prodex_mojo_core::context::truncate_command_output(
            input,
            max_lines,
            options.head_lines,
            options.tail_lines,
            options.max_line_chars,
        )
        .unwrap_or_else(|error| panic!("Mojo command-output truncation failed: {error:?}"));
    }

    let critical = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| critical_preserve_line(line))
        .collect::<Vec<_>>();
    if critical.is_empty() {
        return prodex_mojo_core::context::truncate_command_output(
            input,
            max_lines,
            options.head_lines,
            options.tail_lines,
            options.max_line_chars,
        )
        .unwrap_or_else(|error| panic!("Mojo command-output truncation failed: {error:?}"));
    }

    let mut output = Vec::with_capacity(max_lines);
    output.push(format!(
        "sum: output lines={}, critical={}",
        lines.len(),
        critical.len()
    ));
    if max_lines > 1 {
        output.push("critical lines:".to_string());
    }
    let budget = max_lines.saturating_sub(output.len()).max(1);
    for (index, line) in critical.iter().take(budget) {
        output.push(format!(
            "{}: {}",
            index.saturating_add(1),
            truncate_command_line(line, options.max_line_chars)
        ));
    }
    if critical.len() > budget && !output.is_empty() {
        let last = output.len().saturating_sub(1);
        output[last] = format!(
            "[... {} additional critical lines omitted ...]",
            critical.len() - budget + 1
        );
    }
    lines_to_text(output)
}

#[cfg(test)]
pub(crate) fn is_generated_compaction_header_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("pcs:")
        || lower.starts_with("# prodex context saver:")
        || lower.starts_with("sum:")
        || lower.starts_with("rust/cargo summary:")
        || lower.starts_with("diagnostic summary:")
        || lower.starts_with("success output summary:")
        || lower.starts_with("command output summary:")
        || lower.starts_with("baseline compaction:")
        || lower.starts_with("base:")
        || lower.starts_with("intent matches:")
        || lower.starts_with("int:")
}

pub(super) fn finalize_compacted_command_output(
    kind: CommandOutputKind,
    original: &str,
    mut lines: Vec<String>,
    options: &CommandOutputCompactOptions,
) -> String {
    let mut output = vec![format!(
        "pcs: {} ({}->{})",
        kind.label(),
        count_text_lines(original),
        lines.len(),
    )];
    output.append(&mut lines);
    let text = lines_to_text(output);
    if count_text_lines(&text) > options.max_lines.saturating_add(1).max(2) {
        smart_truncate_command_output(&text, options)
    } else {
        text
    }
}

pub(crate) fn normalize_command_output(input: &str) -> String {
    prodex_mojo_core::context::normalize_command_output(input)
        .unwrap_or_else(|error| panic!("Mojo command-output normalization failed: {error:?}"))
}

pub(crate) fn command_lines(input: &str) -> Vec<&str> {
    input
        .trim_end_matches('\n')
        .split('\n')
        .filter(|line| !(line.is_empty() && input.is_empty()))
        .collect()
}

pub(crate) fn count_text_lines(input: &str) -> usize {
    if input.is_empty() {
        0
    } else {
        input.trim_end_matches('\n').split('\n').count()
    }
}

pub(super) fn lines_to_text(lines: Vec<String>) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

pub(super) fn truncate_command_line(line: &str, max_chars: usize) -> String {
    prodex_mojo_core::context::truncate_command_output(line, 1, 1, 0, max_chars.max(24))
        .unwrap_or_else(|error| panic!("Mojo command-line truncation failed: {error:?}"))
        .trim_end_matches('\n')
        .to_string()
}
