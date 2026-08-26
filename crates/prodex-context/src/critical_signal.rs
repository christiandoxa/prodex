use serde::Serialize;
#[cfg(any(not(feature = "mojo"), test))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(feature = "mojo")]
    fn values(self) -> [usize; 7] {
        [
            self.errors,
            self.file_locations,
            self.diff_hunks,
            self.test_failures,
            self.exit_codes,
            self.stack_markers,
            self.rust_diagnostics,
        ]
    }

    #[cfg(feature = "mojo")]
    fn from_values(values: [usize; 7]) -> Self {
        Self {
            errors: values[0],
            file_locations: values[1],
            diff_hunks: values[2],
            test_failures: values[3],
            exit_codes: values[4],
            stack_markers: values[5],
            rust_diagnostics: values[6],
        }
    }

    #[cfg(any(not(feature = "mojo"), test))]
    pub(crate) fn add_assign(&mut self, other: Self) {
        self.errors = self.errors.saturating_add(other.errors);
        self.file_locations = self.file_locations.saturating_add(other.file_locations);
        self.diff_hunks = self.diff_hunks.saturating_add(other.diff_hunks);
        self.test_failures = self.test_failures.saturating_add(other.test_failures);
        self.exit_codes = self.exit_codes.saturating_add(other.exit_codes);
        self.stack_markers = self.stack_markers.saturating_add(other.stack_markers);
        self.rust_diagnostics = self.rust_diagnostics.saturating_add(other.rust_diagnostics);
    }

    #[cfg(not(feature = "mojo"))]
    fn subtract_assign(&mut self, other: Self) {
        self.errors = self.errors.saturating_sub(other.errors);
        self.file_locations = self.file_locations.saturating_sub(other.file_locations);
        self.diff_hunks = self.diff_hunks.saturating_sub(other.diff_hunks);
        self.test_failures = self.test_failures.saturating_sub(other.test_failures);
        self.exit_codes = self.exit_codes.saturating_sub(other.exit_codes);
        self.stack_markers = self.stack_markers.saturating_sub(other.stack_markers);
        self.rust_diagnostics = self.rust_diagnostics.saturating_sub(other.rust_diagnostics);
    }

    #[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    {
        let analysis = prodex_mojo_core::rich::analyze_context(input)
            .expect("Mojo context analysis returned an invalid structured result");
        CriticalSignalCounts {
            errors: analysis.counts[0],
            file_locations: analysis.counts[1],
            diff_hunks: analysis.counts[2],
            test_failures: analysis.counts[3],
            exit_codes: analysis.counts[4],
            stack_markers: analysis.counts[5],
            rust_diagnostics: analysis.counts[6],
        }
    }
    #[cfg(not(feature = "mojo"))]
    {
        let normalized = normalize_command_output(input);
        let mut counts = CriticalSignalCounts::default();

        for line in command_lines(&normalized) {
            counts.add_assign(critical_signal_counts_for_line(line));
        }

        counts
    }
}

pub fn critical_signal_self_check(before: &str, after: &str) -> CriticalSignalSelfCheck {
    let before = count_critical_signals(before);
    let after = count_critical_signals(after);
    #[cfg(feature = "mojo")]
    let (lost, gained) = prodex_mojo_core::context::signal_diff(&before.values(), &after.values())
        .expect("Mojo critical-signal diff returned invalid output");
    #[cfg(not(feature = "mojo"))]
    let (lost, gained) = (before.saturating_loss(after), after.saturating_loss(before));
    CriticalSignalSelfCheck {
        before,
        after,
        #[cfg(feature = "mojo")]
        lost: CriticalSignalCounts::from_values(lost),
        #[cfg(feature = "mojo")]
        gained: CriticalSignalCounts::from_values(gained),
        #[cfg(not(feature = "mojo"))]
        lost,
        #[cfg(not(feature = "mojo"))]
        gained,
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

    #[cfg(feature = "mojo")]
    {
        let (before_rows, mut after_available, line_count) =
            critical_signal_normalized_rows(before, after)
                .expect("critical-signal rows fit the Mojo ABI");
        prodex_mojo_core::context::lost_line_ranges_batch(
            &before_rows,
            &mut after_available,
            &check.lost.values(),
            line_count,
            options.context_lines,
            options.max_ranges,
            options.max_range_lines,
        )
        .expect("Mojo critical-signal range selection returned invalid output")
        .into_iter()
        .map(|(start, end)| CriticalSignalLineRange { start, end })
        .collect()
    }

    #[cfg(not(feature = "mojo"))]
    {
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
}

#[cfg(feature = "mojo")]
fn critical_signal_normalized_rows(
    before: &str,
    after: &str,
) -> Result<(Vec<i64>, Vec<i64>, usize), prodex_mojo_core::MojoError> {
    let before = normalize_command_output(before);
    let after = normalize_command_output(after);
    let before_lines = command_lines(&before);
    let after_lines = command_lines(&after);
    let before_input = before_lines
        .iter()
        .map(|line| prodex_mojo_core::context::ContextSignalLine {
            text: line.trim(),
            counts: critical_signal_counts_for_line(line).values(),
        })
        .collect::<Vec<_>>();
    let after_input = after_lines
        .iter()
        .map(|line| prodex_mojo_core::context::ContextSignalLine {
            text: line.trim(),
            counts: critical_signal_counts_for_line(line).values(),
        })
        .collect::<Vec<_>>();
    let rows = prodex_mojo_core::context::prepare_signal_rows(&before_input, &after_input)?;
    Ok((rows.before_rows, rows.after_available, before_lines.len()))
}

#[cfg(all(test, feature = "mojo"))]
fn critical_signal_normalized_rows_rust(
    before: &str,
    after: &str,
) -> Result<(Vec<i64>, Vec<i64>, usize), prodex_mojo_core::MojoError> {
    let before = normalize_command_output(before);
    let after = normalize_command_output(after);
    let before_lines = command_lines(&before);
    let mut key_ids = BTreeMap::<String, usize>::new();
    let mut after_available = Vec::<i64>::new();

    for line in command_lines(&after) {
        let counts = critical_signal_counts_for_line(line);
        if counts.is_empty() {
            continue;
        }
        let next_id = key_ids.len();
        let key_id = *key_ids
            .entry(critical_signal_line_key(line))
            .or_insert(next_id);
        if key_id == after_available.len() {
            after_available.push(0);
        }
        after_available[key_id] = after_available[key_id]
            .checked_add(1_i64)
            .ok_or(prodex_mojo_core::MojoError::InvalidInput)?;
    }

    let mut before_rows = Vec::with_capacity(
        before_lines
            .len()
            .checked_mul(8)
            .ok_or(prodex_mojo_core::MojoError::InvalidInput)?,
    );
    for line in &before_lines {
        let counts = critical_signal_counts_for_line(line);
        let key_id = if counts.is_empty() {
            -1
        } else {
            let next_id = key_ids.len();
            let key_id = *key_ids
                .entry(critical_signal_line_key(line))
                .or_insert(next_id);
            if key_id == after_available.len() {
                after_available.push(0);
            }
            i64::try_from(key_id).map_err(|_| prodex_mojo_core::MojoError::InvalidInput)?
        };
        before_rows.push(key_id);
        for value in counts.values() {
            before_rows
                .push(i64::try_from(value).map_err(|_| prodex_mojo_core::MojoError::InvalidInput)?);
        }
    }

    Ok((before_rows, after_available, before_lines.len()))
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_text_rows_tests {
    use super::{critical_signal_normalized_rows, critical_signal_normalized_rows_rust};

    #[test]
    fn mojo_text_rows_match_rust_oracle_for_utf8_duplicates_and_generated_inputs() {
        let long = format!("error: {}🔥", "x".repeat(64 * 1024));
        let fixed = [
            (String::new(), String::new()),
            (
                "error: duplicate\nnoise\nerror: duplicate\n".into(),
                "error: duplicate\n".into(),
            ),
            (
                "  error: 账户🙂e\u{301}\0  \n警告: 東京\n".into(),
                "error: 账户🙂e\u{301}\0\n".into(),
            ),
            (format!("{long}\n"), String::new()),
        ];
        for (case, (before, after)) in fixed.into_iter().enumerate() {
            assert_eq!(
                critical_signal_normalized_rows(&before, &after),
                critical_signal_normalized_rows_rust(&before, &after),
                "fixed case {case}"
            );
        }

        let mut state = 0x7465_7874_5f72_6f77_u64;
        for case in 0..512 {
            let before_count = (next_random(&mut state) % 33) as usize;
            let before = generated_output(&mut state, before_count);
            let after_count = (next_random(&mut state) % 33) as usize;
            let after = generated_output(&mut state, after_count);
            assert_eq!(
                critical_signal_normalized_rows(&before, &after),
                critical_signal_normalized_rows_rust(&before, &after),
                "generated case {case}: before={before:?} after={after:?}"
            );
        }
    }

    fn generated_output(state: &mut u64, count: usize) -> String {
        let mut output = String::new();
        for index in 0..count {
            let bucket = next_random(state) % 8;
            let line = match next_random(state) % 6 {
                0 => format!("error: duplicate-{bucket} 账户🙂"),
                1 => format!("fatal: e\u{301}-{bucket}"),
                2 => format!("noise-\0-{bucket}"),
                3 => format!("   warning: 東京-{bucket}   "),
                4 => format!("src/火.rs:{}:2", bucket + 1),
                _ => format!("@@ -{index},1 +{bucket},1 @@"),
            };
            output.push_str(&line);
            output.push(if next_random(state).is_multiple_of(5) {
                '\r'
            } else {
                '\n'
            });
            if output.ends_with('\r') {
                output.push('\n');
            }
        }
        output
    }

    fn next_random(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state
    }
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

#[cfg(all(test, feature = "mojo"))]
pub(crate) fn critical_signal_counts_for_line_for_test(line: &str) -> CriticalSignalCounts {
    critical_signal_counts_for_line(line)
}

#[cfg(not(feature = "mojo"))]
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

#[cfg(any(not(feature = "mojo"), test))]
fn critical_signal_line_key(line: &str) -> String {
    line.trim().to_string()
}

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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
