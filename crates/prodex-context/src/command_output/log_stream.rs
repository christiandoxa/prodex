use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn compact_log_stream_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
    }

    let level_counts = count_log_stream_levels(&lines);
    let preserved = collect_log_stream_preserved_lines(&lines, options);
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let mut output = Vec::<String>::new();
    output.push(format!("pcs: logs ({}->sum)", count_text_lines(input)));
    output.push(format!(
        "sum: logs lines={}, non_empty={}, preserved={}",
        lines.len(),
        non_empty,
        preserved.len()
    ));
    if !level_counts.is_empty() {
        output.push(format_count_map("levels", &level_counts, 10));
    }
    if !preserved.is_empty() {
        output.push("preserved:".to_string());
        for line in preserved {
            output.push(line);
        }
    }

    let text = lines_to_text(output);
    if critical_signal_self_check(input, &text).passed() {
        text
    } else {
        smart_truncate_command_output(input, options)
    }
}

pub(super) fn looks_like_log_stream_output(lines: &[&str]) -> bool {
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    if non_empty < 8 {
        return false;
    }
    let counts = count_log_stream_levels(lines);
    let log_level_lines = counts.values().sum::<usize>();
    let routine = ["info", "debug", "trace"]
        .iter()
        .map(|level| counts.get(*level).copied().unwrap_or_default())
        .sum::<usize>();
    let critical = ["warn", "error", "fatal"]
        .iter()
        .map(|level| counts.get(*level).copied().unwrap_or_default())
        .sum::<usize>();
    log_level_lines.saturating_mul(2) >= non_empty && (routine >= 4 || critical > 0)
}

fn count_log_stream_levels(lines: &[&str]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for line in lines {
        if let Some(level) = log_stream_level_label(line) {
            *counts.entry(level.to_string()).or_default() += 1;
        }
    }
    counts
}

fn collect_log_stream_preserved_lines(
    lines: &[&str],
    options: &CommandOutputCompactOptions,
) -> Vec<String> {
    let mut preserved = Vec::<String>::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index];
        if !is_log_stream_preserve_line(line) {
            index += 1;
            continue;
        }

        preserved.push(line.to_string());
        let mut next = index + 1;
        let mut context = Vec::<&str>::new();
        while next < lines.len() && is_log_stream_stack_context_line(lines[next]) {
            context.push(lines[next]);
            next += 1;
        }
        push_log_stream_stack_context_lines(&mut preserved, &context, options);
        index = next;
    }
    preserved
}

fn push_log_stream_stack_context_lines(
    output: &mut Vec<String>,
    context: &[&str],
    options: &CommandOutputCompactOptions,
) {
    if context.is_empty() {
        return;
    }

    let limit = log_stream_stack_context_line_limit(options);
    if context.len() <= limit {
        output.extend(context.iter().map(|line| (*line).to_string()));
        return;
    }

    let mut selected = BTreeSet::<usize>::new();
    for (index, line) in context.iter().enumerate() {
        if is_critical_preserve_line(line) {
            selected.insert(index);
        }
    }

    let remaining = limit.saturating_sub(selected.len());
    let head = remaining.div_ceil(2);
    let tail = remaining.saturating_sub(head);
    for index in 0..head.min(context.len()) {
        selected.insert(index);
    }
    for index in context.len().saturating_sub(tail)..context.len() {
        selected.insert(index);
    }

    let mut omitted = 0usize;
    for (index, line) in context.iter().enumerate() {
        if selected.contains(&index) {
            if omitted > 0 {
                output.push(format!(
                    "[... omitted {omitted} non-critical stack context lines ...]"
                ));
                omitted = 0;
            }
            output.push((*line).to_string());
        } else {
            omitted += 1;
        }
    }
    if omitted > 0 {
        output.push(format!(
            "[... omitted {omitted} non-critical stack context lines ...]"
        ));
    }
}

fn log_stream_stack_context_line_limit(options: &CommandOutputCompactOptions) -> usize {
    options.max_lines.max(18).saturating_div(3).clamp(6, 18)
}

fn is_log_stream_preserve_line(line: &str) -> bool {
    matches!(
        log_stream_level_label(line),
        Some("warn" | "error" | "fatal")
    ) || is_critical_preserve_line(line)
        || is_log_stream_critical_text_line(line)
}

fn is_log_stream_stack_context_line(line: &str) -> bool {
    if line.trim().is_empty() || log_stream_level_label(line).is_some() {
        return false;
    }
    let trimmed = line.trim_start();
    line.chars().next().is_some_and(char::is_whitespace)
        || trimmed.starts_with("at ")
        || trimmed.starts_with("File ")
        || trimmed.starts_with("Caused by:")
        || trimmed.starts_with("Traceback ")
        || trimmed.starts_with("Stack trace:")
        || trimmed.starts_with("stack trace:")
        || count_file_location_signals(line) > 0
        || is_exception_signal_line(line)
}

fn is_log_stream_critical_text_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.contains("critical") || lower.contains("panic") || lower.contains("exception")
}

pub(crate) fn is_log_level_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    matches!(log_stream_level_label(line), Some("error" | "fatal"))
        || lower.starts_with("error ")
        || lower.starts_with("warn ")
        || lower.starts_with("warning ")
        || lower.starts_with("fatal ")
        || lower.starts_with("[error]")
        || lower.starts_with("[warn]")
        || lower.contains(" error ")
        || lower.contains(" fatal ")
        || lower.contains(" warn ")
}

fn log_stream_level_label(line: &str) -> Option<&'static str> {
    let lower = line.trim_start().to_ascii_lowercase();
    for (level, label) in [
        ("fatal", "fatal"),
        ("error", "error"),
        ("warn", "warn"),
        ("warning", "warn"),
        ("info", "info"),
        ("debug", "debug"),
        ("trace", "trace"),
    ] {
        if contains_json_log_level(&lower, level)
            || contains_kv_log_level(&lower, level)
            || starts_with_log_level_token(&lower, level)
            || contains_delimited_log_level(&lower, level)
        {
            return Some(label);
        }
    }
    None
}

fn contains_json_log_level(lower: &str, level: &str) -> bool {
    for field in ["level", "severity", "status"] {
        if lower.contains(&format!("\"{field}\":\"{level}\""))
            || lower.contains(&format!("\"{field}\": \"{level}\""))
        {
            return true;
        }
    }
    false
}

fn contains_kv_log_level(lower: &str, level: &str) -> bool {
    for field in ["level", "severity", "status"] {
        if lower.contains(&format!("{field}={level}"))
            || lower.contains(&format!("{field}: {level}"))
        {
            return true;
        }
    }
    false
}

fn starts_with_log_level_token(lower: &str, level: &str) -> bool {
    lower.starts_with(&format!("[{level}]"))
        || lower.starts_with(&format!("{level} "))
        || lower.starts_with(&format!("{level}:"))
        || lower.starts_with(&format!("{level}\t"))
}

fn contains_delimited_log_level(lower: &str, level: &str) -> bool {
    lower.contains(&format!(" {level} "))
        || lower.contains(&format!(" {level}:"))
        || lower.contains(&format!(" [{level}]"))
        || lower.contains(&format!(" {level}\t"))
}
