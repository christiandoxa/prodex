use super::*;

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_critical_signal_counts_for_line(line: &str) -> CriticalSignalCounts {
    let line = rust_unprefix_numbered_critical_line(line);
    let mut counts = CriticalSignalCounts::default();
    if rust_is_error_signal_line(line) {
        counts.errors += 1;
    }
    counts.file_locations += rust_count_file_location_signals(line);
    if rust_is_diff_hunk_line(line) {
        counts.diff_hunks += 1;
    }
    if rust_is_test_failure_signal_line(line) {
        counts.test_failures += 1;
    }
    if is_rust_exit_status_line(line) {
        counts.exit_codes += 1;
    }
    if rust_is_stack_signal_line(line) {
        counts.stack_markers += 1;
    }
    if rust_is_diagnostic_signal_line(line) {
        counts.rust_diagnostics += 1;
    }
    counts
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_unprefix_numbered_critical_line(line: &str) -> &str {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix('L') else {
        return line;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || !rest[digits..].starts_with(": ") {
        return line;
    }
    rest[digits + 2..].trim_start()
}

#[cfg(all(test, feature = "mojo"))]
pub(crate) fn critical_signal_counts_for_line_for_test(line: &str) -> CriticalSignalCounts {
    rust_critical_signal_counts_for_line(line)
}

#[cfg(not(feature = "mojo"))]
pub(super) fn critical_signal_line_multiset(input: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for line in command_lines(input) {
        if rust_critical_signal_counts_for_line(line).is_empty() {
            continue;
        }
        counts
            .entry(critical_signal_line_key(line))
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
    }
    counts
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn critical_signal_line_key(line: &str) -> String {
    line.trim().to_string()
}

#[cfg(not(feature = "mojo"))]
pub(super) fn critical_signal_range_around_line(
    line_index: usize,
    line_count: usize,
    context_lines: usize,
    max_range_lines: usize,
) -> CriticalSignalLineRange {
    let max_range_lines = max_range_lines.max(1);
    let signal_line = line_index + 1;
    let mut start = signal_line.saturating_sub(context_lines).max(1);
    let mut end = signal_line.saturating_add(context_lines).min(line_count);

    while end.saturating_sub(start).saturating_add(1) > max_range_lines {
        if signal_line.saturating_sub(start) > end.saturating_sub(signal_line) {
            start += 1;
        } else {
            end = end.saturating_sub(1);
        }
    }

    CriticalSignalLineRange { start, end }
}

#[cfg(not(feature = "mojo"))]
pub(super) fn merge_critical_signal_ranges(
    mut ranges: Vec<CriticalSignalLineRange>,
    max_ranges: usize,
) -> Vec<CriticalSignalLineRange> {
    if ranges.is_empty() || max_ranges == 0 {
        return Vec::new();
    }

    ranges.sort_by_key(|range| (range.start, range.end));
    let mut merged = Vec::<CriticalSignalLineRange>::new();
    for range in ranges {
        if let Some(last) = merged.last_mut()
            && range.start < last.end
        {
            last.end = last.end.max(range.end);
            continue;
        }
        if merged.len() >= max_ranges {
            break;
        }
        merged.push(range);
    }
    merged
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_is_error_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if has_zero_only_summary_count(&lower, &["error", "errors"]) {
        return false;
    }
    if lower.starts_with("error:")
        || lower.starts_with("error[")
        || lower.starts_with("error ")
        || lower.starts_with("error\t")
        || lower.starts_with("fatal:")
        || lower.starts_with("panic:")
        || lower.starts_with("npm err!")
        || lower.starts_with("npm error")
        || lower.starts_with("pnpm error")
        || lower.starts_with("yarn error")
        || lower.starts_with("bun error")
        || lower.starts_with('#') && lower.contains(" error")
        || lower.starts_with("failed ")
        || lower.starts_with("fail ")
        || trimmed.starts_with("E   ")
        || lower.starts_with("thread '") && lower.contains("' panicked at")
        || is_rust_panic_line(line)
        || is_typescript_diagnostic_line(line)
        || is_eslint_diagnostic_line(line)
        || is_junit_xml_failure_line(line)
        || is_exception_signal_line(line)
        || is_log_level_signal_line(line)
    {
        return true;
    }

    rust_contains_jsonish_error_key(trimmed)
        || lower.contains(" status=error")
        || lower.starts_with("status=error")
        || lower.contains(" level=error")
        || lower.starts_with("level=error")
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_contains_jsonish_error_key(line: &str) -> bool {
    line.contains("\"error\"")
        || line.contains("'error'")
        || line.contains("\\\"error\\\"")
        || line.contains("\"type\":\"error\"")
        || line.contains("\"type\": \"error\"")
        || line.contains("\\\"type\\\":\\\"error\\\"")
        || line.contains("\\\"type\\\": \\\"error\\\"")
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_count_file_location_signals(line: &str) -> usize {
    let token_locations = line
        .split_whitespace()
        .filter(|token| rust_token_contains_file_location(token))
        .count();
    let python_location = usize::from(rust_contains_python_file_location(line));
    let paren_location = usize::from(rust_contains_paren_file_location(line));
    token_locations + python_location + paren_location
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_token_contains_file_location(token: &str) -> bool {
    let token = token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}'
        )
    });
    let token = token.trim_end_matches([':', '.']);
    if token.contains("://") || !token.chars().any(|ch| ch.is_ascii_digit()) {
        return false;
    }

    let mut segments = token.rsplitn(3, ':');
    let tail = segments.next().unwrap_or_default().trim_end_matches('.');
    let middle = segments.next().unwrap_or_default();
    let path = segments.next().unwrap_or_default();

    if !tail.chars().all(|ch| ch.is_ascii_digit()) || tail.is_empty() {
        return false;
    }

    if middle.chars().all(|ch| ch.is_ascii_digit()) && !middle.is_empty() {
        return looks_like_location_path(path);
    }

    looks_like_location_path(middle)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_contains_python_file_location(line: &str) -> bool {
    let trimmed = line.trim_start();
    let (quote, rest) = if let Some(rest) = trimmed.strip_prefix("File \"") {
        ('"', rest)
    } else if let Some(rest) = trimmed.strip_prefix("File '") {
        ('\'', rest)
    } else {
        return false;
    };
    let Some((path, after_path)) = rest.split_once(quote) else {
        return false;
    };
    if !looks_like_location_path(path) || !after_path.contains(", line ") {
        return false;
    }
    let Some((_, after_line)) = after_path.split_once(", line ") else {
        return false;
    };
    after_line
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_contains_paren_file_location(line: &str) -> bool {
    line.split_whitespace()
        .any(rust_token_contains_paren_file_location)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_token_contains_paren_file_location(token: &str) -> bool {
    let token = token.trim_matches(|ch: char| {
        matches!(ch, '"' | '\'' | '`' | ',' | ';' | '[' | ']' | '{' | '}')
    });
    let Some((path, rest)) = token.split_once('(') else {
        return false;
    };
    let Some((location, _)) = rest.split_once(')') else {
        return false;
    };
    let Some((line, column)) = location.split_once(',') else {
        return false;
    };
    looks_like_location_path(path)
        && !line.trim().is_empty()
        && line.trim().chars().all(|ch| ch.is_ascii_digit())
        && !column.trim().is_empty()
        && column.trim().chars().all(|ch| ch.is_ascii_digit())
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_looks_like_location_path(path: &str) -> bool {
    let path = path.trim_matches(|ch: char| matches!(ch, '<' | '>' | '-' | ':' | ' '));
    path.contains('/')
        || path.contains('\\')
        || path.rsplit('/').next().is_some_and(|name| {
            name.rsplit_once('.').is_some_and(|(_, ext)| {
                !ext.is_empty()
                    && ext.len() <= 12
                    && ext
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
            })
        })
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_is_diff_hunk_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("@@ ") && trimmed[3..].contains("@@")
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_is_test_failure_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    rust_failed_test_name(trimmed).is_some()
        || rust_failure_separator_name(trimmed).is_some()
        || generic_failed_test_name(trimmed).is_some()
        || is_rust_failure_summary_line(trimmed)
        || trimmed.starts_with("test result: FAILED")
        || trimmed.starts_with("failures:")
        || trimmed.contains(" ... FAILED")
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_is_stack_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    is_rust_backtrace_start(trimmed)
        || trimmed.starts_with("Traceback (most recent call last):")
        || trimmed.starts_with("Stack trace:")
        || trimmed.starts_with("stack trace:")
        || trimmed.starts_with("Backtrace:")
        || trimmed.starts_with("Caused by:")
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rust_is_diagnostic_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    rust_diagnostic_severity(trimmed).is_some()
        || trimmed.starts_with("--> ")
        || trimmed.starts_with("::: ")
        || trimmed.starts_with("= note:")
        || trimmed.starts_with("= help:")
        || trimmed.starts_with("help:")
        || trimmed.starts_with("note:")
        || trimmed.starts_with("warning:")
        || trimmed.starts_with("warning[")
        || trimmed.contains("clippy::")
}
