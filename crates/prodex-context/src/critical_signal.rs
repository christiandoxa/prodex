use serde::Serialize;
use std::collections::BTreeMap;

use crate::{
    command_lines, generic_failed_test_name, has_zero_only_summary_count,
    is_eslint_diagnostic_line, is_exception_signal_line, is_junit_xml_failure_line,
    is_log_level_signal_line, is_rust_backtrace_start, is_rust_exit_status_line,
    is_rust_failure_summary_line, is_rust_panic_line, is_typescript_diagnostic_line,
    normalize_command_output, rust_diagnostic_severity, rust_failed_test_name,
    rust_failure_separator_name,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CriticalSignalCounts {
    pub errors: usize,
    pub file_locations: usize,
    pub diff_hunks: usize,
    pub test_failures: usize,
    pub exit_codes: usize,
    pub stack_markers: usize,
    pub rust_diagnostics: usize,
}

impl CriticalSignalCounts {
    pub fn total(self) -> usize {
        self.errors
            + self.file_locations
            + self.diff_hunks
            + self.test_failures
            + self.exit_codes
            + self.stack_markers
            + self.rust_diagnostics
    }

    pub fn is_empty(self) -> bool {
        self.total() == 0
    }

    fn saturating_loss(self, after: Self) -> Self {
        Self {
            errors: self.errors.saturating_sub(after.errors),
            file_locations: self.file_locations.saturating_sub(after.file_locations),
            diff_hunks: self.diff_hunks.saturating_sub(after.diff_hunks),
            test_failures: self.test_failures.saturating_sub(after.test_failures),
            exit_codes: self.exit_codes.saturating_sub(after.exit_codes),
            stack_markers: self.stack_markers.saturating_sub(after.stack_markers),
            rust_diagnostics: self.rust_diagnostics.saturating_sub(after.rust_diagnostics),
        }
    }

    fn add_assign(&mut self, other: Self) {
        self.errors = self.errors.saturating_add(other.errors);
        self.file_locations = self.file_locations.saturating_add(other.file_locations);
        self.diff_hunks = self.diff_hunks.saturating_add(other.diff_hunks);
        self.test_failures = self.test_failures.saturating_add(other.test_failures);
        self.exit_codes = self.exit_codes.saturating_add(other.exit_codes);
        self.stack_markers = self.stack_markers.saturating_add(other.stack_markers);
        self.rust_diagnostics = self.rust_diagnostics.saturating_add(other.rust_diagnostics);
    }

    fn subtract_assign(&mut self, other: Self) {
        self.errors = self.errors.saturating_sub(other.errors);
        self.file_locations = self.file_locations.saturating_sub(other.file_locations);
        self.diff_hunks = self.diff_hunks.saturating_sub(other.diff_hunks);
        self.test_failures = self.test_failures.saturating_sub(other.test_failures);
        self.exit_codes = self.exit_codes.saturating_sub(other.exit_codes);
        self.stack_markers = self.stack_markers.saturating_sub(other.stack_markers);
        self.rust_diagnostics = self.rust_diagnostics.saturating_sub(other.rust_diagnostics);
    }

    fn overlaps(self, other: Self) -> bool {
        self.errors > 0 && other.errors > 0
            || self.file_locations > 0 && other.file_locations > 0
            || self.diff_hunks > 0 && other.diff_hunks > 0
            || self.test_failures > 0 && other.test_failures > 0
            || self.exit_codes > 0 && other.exit_codes > 0
            || self.stack_markers > 0 && other.stack_markers > 0
            || self.rust_diagnostics > 0 && other.rust_diagnostics > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CriticalSignalSelfCheck {
    pub before: CriticalSignalCounts,
    pub after: CriticalSignalCounts,
    pub lost: CriticalSignalCounts,
    pub gained: CriticalSignalCounts,
}

impl CriticalSignalSelfCheck {
    pub fn passed(self) -> bool {
        self.lost.is_empty()
    }

    pub fn has_loss(self) -> bool {
        !self.passed()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CriticalSignalLineRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CriticalSignalLineRangeOptions {
    pub context_lines: usize,
    pub max_ranges: usize,
    pub max_range_lines: usize,
}

impl Default for CriticalSignalLineRangeOptions {
    fn default() -> Self {
        Self {
            context_lines: 1,
            max_ranges: 32,
            max_range_lines: 6,
        }
    }
}

pub fn count_critical_signals(input: &str) -> CriticalSignalCounts {
    let normalized = normalize_command_output(input);
    let mut counts = CriticalSignalCounts::default();

    for line in command_lines(&normalized) {
        counts.add_assign(critical_signal_counts_for_line(line));
    }

    counts
}

pub fn critical_signal_self_check(before: &str, after: &str) -> CriticalSignalSelfCheck {
    let before = count_critical_signals(before);
    let after = count_critical_signals(after);
    CriticalSignalSelfCheck {
        before,
        after,
        lost: before.saturating_loss(after),
        gained: after.saturating_loss(before),
    }
}

pub fn critical_signal_lost_line_ranges(before: &str, after: &str) -> Vec<CriticalSignalLineRange> {
    critical_signal_lost_line_ranges_with_options(
        before,
        after,
        CriticalSignalLineRangeOptions::default(),
    )
}

pub fn critical_signal_lost_line_ranges_with_options(
    before: &str,
    after: &str,
    options: CriticalSignalLineRangeOptions,
) -> Vec<CriticalSignalLineRange> {
    let check = critical_signal_self_check(before, after);
    if check.passed() || options.max_ranges == 0 {
        return Vec::new();
    }

    let before = normalize_command_output(before);
    let after = normalize_command_output(after);
    let before_lines = command_lines(&before);
    let mut after_available = critical_signal_line_multiset(&after);
    let mut remaining_loss = check.lost;
    let mut ranges = Vec::<CriticalSignalLineRange>::new();

    for (line_index, line) in before_lines.iter().enumerate() {
        if remaining_loss.is_empty() {
            break;
        }

        let counts = critical_signal_counts_for_line(line);
        if counts.is_empty() {
            continue;
        }

        let key = critical_signal_line_key(line);
        if let Some(available) = after_available.get_mut(&key)
            && *available > 0
        {
            *available -= 1;
            continue;
        }

        if !counts.overlaps(remaining_loss) {
            continue;
        }

        ranges.push(critical_signal_range_around_line(
            line_index,
            before_lines.len(),
            options.context_lines,
            options.max_range_lines,
        ));
        remaining_loss.subtract_assign(counts);
    }

    merge_critical_signal_ranges(ranges, options.max_ranges)
}

fn critical_signal_counts_for_line(line: &str) -> CriticalSignalCounts {
    let mut counts = CriticalSignalCounts::default();
    if is_error_signal_line(line) {
        counts.errors += 1;
    }
    counts.file_locations += count_file_location_signals(line);
    if is_diff_hunk_line(line) {
        counts.diff_hunks += 1;
    }
    if is_test_failure_signal_line(line) {
        counts.test_failures += 1;
    }
    if is_rust_exit_status_line(line) {
        counts.exit_codes += 1;
    }
    if is_stack_signal_line(line) {
        counts.stack_markers += 1;
    }
    if is_rust_diagnostic_signal_line(line) {
        counts.rust_diagnostics += 1;
    }
    counts
}

fn critical_signal_line_multiset(input: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for line in command_lines(input) {
        if critical_signal_counts_for_line(line).is_empty() {
            continue;
        }
        counts
            .entry(critical_signal_line_key(line))
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
    }
    counts
}

fn critical_signal_line_key(line: &str) -> String {
    line.trim().to_string()
}

fn critical_signal_range_around_line(
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

fn merge_critical_signal_ranges(
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

pub(crate) fn is_error_signal_line(line: &str) -> bool {
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

    contains_jsonish_error_key(trimmed)
        || lower.contains(" status=error")
        || lower.starts_with("status=error")
        || lower.contains(" level=error")
        || lower.starts_with("level=error")
}

fn contains_jsonish_error_key(line: &str) -> bool {
    line.contains("\"error\"")
        || line.contains("'error'")
        || line.contains("\\\"error\\\"")
        || line.contains("\"type\":\"error\"")
        || line.contains("\"type\": \"error\"")
        || line.contains("\\\"type\\\":\\\"error\\\"")
        || line.contains("\\\"type\\\": \\\"error\\\"")
}

pub(crate) fn count_file_location_signals(line: &str) -> usize {
    let token_locations = line
        .split_whitespace()
        .filter(|token| token_contains_file_location(token))
        .count();
    let python_location = usize::from(contains_python_file_location(line));
    let paren_location = usize::from(contains_paren_file_location(line));
    token_locations + python_location + paren_location
}

fn token_contains_file_location(token: &str) -> bool {
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

fn contains_python_file_location(line: &str) -> bool {
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

fn contains_paren_file_location(line: &str) -> bool {
    line.split_whitespace()
        .any(token_contains_paren_file_location)
}

fn token_contains_paren_file_location(token: &str) -> bool {
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

pub(crate) fn looks_like_location_path(path: &str) -> bool {
    let path = path.trim_matches(|ch: char| matches!(ch, '<' | '>' | '-' | ':' | ' '));
    if path.is_empty() {
        return false;
    }
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

pub(crate) fn is_diff_hunk_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("@@ ") && trimmed[3..].contains("@@")
}

pub(crate) fn is_test_failure_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    rust_failed_test_name(trimmed).is_some()
        || rust_failure_separator_name(trimmed).is_some()
        || generic_failed_test_name(trimmed).is_some()
        || is_rust_failure_summary_line(trimmed)
        || trimmed.starts_with("test result: FAILED")
        || trimmed.starts_with("failures:")
        || trimmed.contains(" ... FAILED")
}

pub(crate) fn is_stack_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    is_rust_backtrace_start(trimmed)
        || trimmed.starts_with("Traceback (most recent call last):")
        || trimmed.starts_with("Stack trace:")
        || trimmed.starts_with("stack trace:")
        || trimmed.starts_with("Backtrace:")
        || trimmed.starts_with("Caused by:")
}

pub(crate) fn is_rust_diagnostic_signal_line(line: &str) -> bool {
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
