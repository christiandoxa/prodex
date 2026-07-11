use super::*;

pub(super) fn is_node_stack_error_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("at ") && count_file_location_signals(trimmed) > 0
}

pub(super) fn collect_rust_diagnostic_block(
    lines: &[&str],
    start: usize,
    max_lines: usize,
) -> (RustCriticalBlock, usize) {
    let mut end = start + 1;
    while end < lines.len() {
        let line = lines[end];
        if rust_diagnostic_severity(line).is_some()
            || rust_failed_test_name(line).is_some()
            || rust_failure_separator_name(line).is_some()
        {
            break;
        }
        if rust_noise_label(line).is_some() && !is_rust_success_summary_line(line) {
            break;
        }
        if line.trim().is_empty()
            && end + 1 < lines.len()
            && rust_diagnostic_severity(lines[end + 1]).is_some()
        {
            break;
        }
        end += 1;
        if end.saturating_sub(start) >= max_lines.saturating_mul(4) {
            break;
        }
    }

    let label = format!("diagnostic: {}", rust_critical_label(lines[start]));
    (
        RustCriticalBlock {
            label,
            lines: compact_rust_block_lines(&lines[start..end], max_lines),
        },
        end,
    )
}

pub(super) fn collect_rust_failure_block(
    lines: &[&str],
    start: usize,
    max_lines: usize,
) -> (RustCriticalBlock, usize) {
    let mut end = start + 1;
    while end < lines.len() {
        let line = lines[end];
        if end > start + 1
            && (rust_diagnostic_severity(line).is_some()
                || rust_failed_test_name(line).is_some()
                || rust_failure_separator_name(line).is_some())
        {
            break;
        }
        if rust_noise_label(line).is_some() && !is_rust_success_summary_line(line) {
            break;
        }
        end += 1;
        if end.saturating_sub(start) >= max_lines.saturating_mul(4) {
            break;
        }
    }

    let label = if let Some(test_name) =
        rust_failed_test_name(lines[start]).or_else(|| rust_failure_separator_name(lines[start]))
    {
        format!("failed test: {test_name}")
    } else if is_rust_backtrace_start(lines[start]) {
        "stack backtrace".to_string()
    } else {
        format!("panic/error: {}", rust_critical_label(lines[start]))
    };
    (
        RustCriticalBlock {
            label,
            lines: compact_rust_block_lines(&lines[start..end], max_lines),
        },
        end,
    )
}

pub(super) fn collect_diagnostic_block(
    lines: &[&str],
    start: usize,
    max_lines: usize,
) -> (CommandCriticalBlock, usize) {
    let mut end = start + 1;
    while end < lines.len() {
        let line = lines[end];
        if end > start + 1 && is_diagnostic_block_start(line) {
            break;
        }
        if diagnostic_noise_label(line).is_some() && !is_diagnostic_key_line(line) {
            break;
        }
        end += 1;
        if end.saturating_sub(start) >= max_lines.saturating_mul(4) {
            break;
        }
    }

    (
        CommandCriticalBlock {
            label: diagnostic_critical_label(lines[start]),
            lines: compact_command_block_lines(&lines[start..end], max_lines),
        },
        end,
    )
}

pub(super) fn compact_rust_block_lines(lines: &[&str], max_lines: usize) -> Vec<String> {
    compact_command_block_lines(lines, max_lines)
}

pub(super) fn compact_command_block_lines(lines: &[&str], max_lines: usize) -> Vec<String> {
    if lines.len() <= max_lines {
        return lines.iter().map(|line| (*line).to_string()).collect();
    }

    let head = max_lines.saturating_mul(2).div_ceil(3).max(1);
    let tail = max_lines.saturating_sub(head).saturating_sub(1);
    let omitted = lines.len().saturating_sub(head + tail);
    let mut output = lines
        .iter()
        .take(head)
        .map(|line| (*line).to_string())
        .collect::<Vec<_>>();
    output.push(format!("[... omitted {omitted} lines inside block ...]"));
    if tail > 0 {
        output.extend(
            lines
                .iter()
                .skip(lines.len().saturating_sub(tail))
                .map(|line| (*line).to_string()),
        );
    }
    output
}

pub(super) fn rust_critical_label(line: &str) -> String {
    truncate_command_line(line.trim(), 96)
}

pub(super) fn diagnostic_critical_label(line: &str) -> String {
    if is_stack_signal_line(line) {
        return "stack trace".to_string();
    }
    if let Some(test_name) = generic_failed_test_name(line) {
        return format!("failed test: {test_name}");
    }
    if is_exception_signal_line(line) {
        return format!("exception: {}", truncate_command_line(line.trim(), 96));
    }
    format!("diagnostic: {}", truncate_command_line(line.trim(), 96))
}

pub(super) fn push_labeled_lines(
    output: &mut Vec<String>,
    label: &str,
    lines: &[String],
    limit: usize,
) {
    if lines.is_empty() {
        return;
    }

    output.push(format!("{label} ({}):", lines.len()));
    for line in lines.iter().take(limit.max(1)) {
        output.push(format!("  {line}"));
    }
    if lines.len() > limit.max(1) {
        output.push(format!(
            "  [... {} more {label} ...]",
            lines.len() - limit.max(1)
        ));
    }
}

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

pub(super) fn push_unique_line(lines: &mut Vec<String>, line: &str) {
    if !line.is_empty() && !lines.iter().any(|existing| existing == line) {
        lines.push(line.to_string());
    }
}

pub(super) fn push_unique_truncated_line(lines: &mut Vec<String>, line: &str, max_chars: usize) {
    let line = truncate_command_line(line.trim(), max_chars);
    push_unique_line(lines, &line);
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
    let mut output = Vec::new();
    if lines.len() <= max_lines {
        for line in lines {
            output.push(truncate_command_line(line, options.max_line_chars));
        }
        return lines_to_text(output);
    }

    if let Some(output) = smart_truncate_with_critical_lines(&lines, options, max_lines) {
        return output;
    }

    let (head, tail) = bounded_head_tail(options, max_lines);
    for line in lines.iter().take(head) {
        output.push(truncate_command_line(line, options.max_line_chars));
    }
    output.push(format!(
        "[... omitted {} lines ...]",
        lines.len().saturating_sub(head + tail)
    ));
    for line in lines.iter().skip(lines.len().saturating_sub(tail)) {
        output.push(truncate_command_line(line, options.max_line_chars));
    }
    lines_to_text(output)
}

pub(super) fn smart_truncate_with_critical_lines(
    lines: &[&str],
    options: &CommandOutputCompactOptions,
    max_lines: usize,
) -> Option<String> {
    let critical = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| is_critical_preserve_line(line))
        .map(|(index, line)| (index, *line))
        .collect::<Vec<_>>();
    if critical.is_empty() {
        return None;
    }

    if max_lines <= 12 {
        let mut output = Vec::new();
        output.push(format!(
            "sum: output lines={}, critical={}",
            lines.len(),
            critical.len()
        ));
        output.push("critical lines:".to_string());
        push_numbered_critical_lines(
            &mut output,
            &critical,
            max_lines.saturating_sub(2).max(1),
            options,
        );
        return Some(lines_to_text(output));
    }

    let section_overhead = 4usize;
    let context_budget = max_lines.saturating_sub(section_overhead).max(1);
    let mut head = options
        .head_lines
        .min(context_budget.saturating_div(4).max(1));
    let mut tail = options
        .tail_lines
        .min(context_budget.saturating_div(4).max(1));
    while head.saturating_add(tail) >= context_budget && tail > 0 {
        tail -= 1;
    }
    while head.saturating_add(tail) >= context_budget && head > 0 {
        head -= 1;
    }
    let critical_budget = context_budget.saturating_sub(head + tail).max(1);

    let mut output = Vec::new();
    output.push(format!(
        "sum: output lines={}, critical={}",
        lines.len(),
        critical.len()
    ));
    output.push("head:".to_string());
    for line in lines.iter().take(head) {
        output.push(truncate_command_line(line, options.max_line_chars));
    }
    output.push("critical lines:".to_string());

    let filtered_critical = critical
        .iter()
        .filter(|(index, _)| *index >= head && *index < lines.len().saturating_sub(tail))
        .copied()
        .collect::<Vec<_>>();
    push_numbered_critical_lines(&mut output, &filtered_critical, critical_budget, options);

    output.push("tail:".to_string());
    for line in lines.iter().skip(lines.len().saturating_sub(tail)) {
        output.push(truncate_command_line(line, options.max_line_chars));
    }

    Some(lines_to_text(output))
}

pub(super) fn push_numbered_critical_lines(
    output: &mut Vec<String>,
    critical: &[(usize, &str)],
    budget: usize,
    options: &CommandOutputCompactOptions,
) {
    if critical.is_empty() || budget == 0 {
        return;
    }

    if critical.len() <= budget {
        for (index, line) in critical {
            output.push(format!(
                "  L{}: {}",
                index + 1,
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
        return;
    }

    if budget == 1 {
        if let Some((index, line)) = critical.first() {
            output.push(format!(
                "  L{}: {}",
                index + 1,
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
        return;
    }

    let mut candidates = critical.to_vec();
    candidates.sort_by_key(|(index, line)| (critical_preserve_priority(line), *index));
    let selected_budget = budget.saturating_sub(1).max(1);
    let mut selected = Vec::new();
    let mut seen = BTreeMap::<String, ()>::new();
    let failure_present = critical
        .iter()
        .any(|(_, line)| is_failure_first_critical_line(line));
    for candidate in candidates {
        if failure_present && is_warning_only_signal_line(candidate.1) {
            continue;
        }
        let key = critical_line_selection_key(candidate.1);
        if seen.insert(key, ()).is_some() {
            continue;
        }
        selected.push(candidate);
        if selected.len() >= selected_budget {
            break;
        }
    }
    selected.sort_by_key(|(index, _)| *index);

    for (index, line) in &selected {
        output.push(format!(
            "  L{}: {}",
            index + 1,
            truncate_command_line(line.trim(), options.max_line_chars)
        ));
    }
    output.push(format!(
        "  [... omitted {} critical lines ...]",
        critical.len().saturating_sub(selected.len())
    ));
}

pub(super) fn critical_line_selection_key(line: &str) -> String {
    line.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

pub(super) fn critical_preserve_priority(line: &str) -> u8 {
    if is_generated_compaction_header_line(line) {
        return 5;
    }
    if is_warning_only_signal_line(line) {
        return 4;
    }
    if is_error_signal_line(line)
        || is_test_failure_signal_line(line)
        || is_diagnostic_failure_summary_line(line)
        || is_rust_exit_status_line(line)
    {
        return 0;
    }
    if count_file_location_signals(line) > 0 || is_stack_signal_line(line) {
        return 1;
    }
    if is_rust_diagnostic_signal_line(line) || is_log_level_signal_line(line) {
        return 2;
    }
    3
}

pub(super) fn is_failure_first_critical_line(line: &str) -> bool {
    !is_warning_only_signal_line(line)
        && (is_error_signal_line(line)
            || is_test_failure_signal_line(line)
            || is_diagnostic_failure_summary_line(line)
            || is_rust_exit_status_line(line))
}

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
        || lower.starts_with("diagnostics (")
        || lower.starts_with("locations (")
        || lower.starts_with("failed tests (")
        || lower.starts_with("exit statuses (")
        || lower.starts_with("key lines (")
        || lower.starts_with("critical blocks:")
}

pub(super) fn is_warning_only_signal_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    (lower.starts_with("warning")
        || lower.contains(" warning ")
        || lower.contains("warning ts")
        || lower.contains(": warning ts")
        || lower.contains(" - warning ts"))
        && !lower.contains("error")
        && !lower.contains("failed")
        && !lower.contains("panicked")
}

pub(super) fn finalize_compacted_command_output(
    kind: CommandOutputKind,
    original: &str,
    mut lines: Vec<String>,
    options: &CommandOutputCompactOptions,
) -> String {
    let original_lines = count_text_lines(original);
    let body_lines = lines.len();
    let mut output = Vec::new();
    output.push(format!(
        "pcs: {} ({}->{})",
        kind.label(),
        original_lines,
        body_lines,
    ));
    output.append(&mut lines);
    let text = lines_to_text(output);
    if count_text_lines(&text) > options.max_lines.saturating_add(1).max(2) {
        smart_truncate_command_output(&text, options)
    } else {
        text
    }
}

pub(crate) fn normalize_command_output(input: &str) -> String {
    let stripped = strip_ansi_codes(input);
    let mut lines = stripped
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect::<Vec<_>>();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines_to_text(lines)
}

pub(super) fn strip_ansi_codes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for code in chars.by_ref() {
                        if ('@'..='~').contains(&code) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    let mut previous = '\0';
                    for code in chars.by_ref() {
                        if code == '\u{7}' || (previous == '\u{1b}' && code == '\\') {
                            break;
                        }
                        previous = code;
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
        } else if ch == '\r' {
            if !matches!(chars.peek(), Some('\n')) {
                output.push('\n');
            }
        } else {
            output.push(ch);
        }
    }
    output
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
    let max_chars = max_chars.max(24);
    let char_count = line.chars().count();
    if char_count <= max_chars {
        return line.to_string();
    }

    let tail_chars = 16.min(max_chars / 3);
    let head_chars = max_chars.saturating_sub(tail_chars).saturating_sub(24);
    let head = line.chars().take(head_chars).collect::<String>();
    let tail = line
        .chars()
        .rev()
        .take(tail_chars)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!(
        "{head} [... {} chars omitted ...] {tail}",
        char_count.saturating_sub(head_chars + tail_chars),
    )
}

pub(super) fn bounded_head_tail(
    options: &CommandOutputCompactOptions,
    max_lines: usize,
) -> (usize, usize) {
    let requested_head = options.head_lines.min(max_lines);
    let requested_tail = options
        .tail_lines
        .min(max_lines.saturating_sub(requested_head));
    if requested_head + requested_tail == 0 {
        (max_lines, 0)
    } else {
        (requested_head, requested_tail)
    }
}
