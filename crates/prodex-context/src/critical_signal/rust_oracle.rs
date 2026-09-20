#[cfg(not(feature = "mojo"))]
mod implementation {
    use super::super::{
        CriticalSignalCounts, CriticalSignalLineRange, CriticalSignalLineRangeOptions,
    };
    use std::collections::BTreeMap;

    pub(super) fn count_critical_signals(input: &str) -> CriticalSignalCounts {
        normalized_lines(input).into_iter().fold(
            CriticalSignalCounts::default(),
            |mut total, line| {
                let current = counts_for_line(line);
                add_assign(&mut total, current);
                total
            },
        )
    }

    pub(super) fn signal_diff(
        before: CriticalSignalCounts,
        after: CriticalSignalCounts,
    ) -> (CriticalSignalCounts, CriticalSignalCounts) {
        (loss(before, after), loss(after, before))
    }

    pub(super) fn lost_line_ranges(
        before: &str,
        after: &str,
        mut remaining_loss: CriticalSignalCounts,
        options: CriticalSignalLineRangeOptions,
    ) -> Vec<CriticalSignalLineRange> {
        let before_lines = normalized_lines(before);
        let mut after_available = BTreeMap::<String, usize>::new();

        for line in normalized_lines(after) {
            let counts = counts_for_line(line);
            if counts.is_empty() {
                continue;
            }
            *after_available.entry(line.trim().to_string()).or_default() += 1;
        }

        let mut ranges = Vec::new();
        for (index, line) in before_lines.iter().enumerate() {
            if remaining_loss.is_empty() {
                break;
            }
            let counts = counts_for_line(line);
            if counts.is_empty() {
                continue;
            }

            let key = line.trim().to_string();
            if let Some(available) = after_available.get_mut(&key)
                && *available > 0
            {
                *available -= 1;
                continue;
            }
            if !overlaps(counts, remaining_loss) {
                continue;
            }

            ranges.push(range_around(
                index,
                before_lines.len(),
                options.context_lines,
                options.max_range_lines,
            ));
            subtract_assign(&mut remaining_loss, counts);
        }

        merge_ranges(ranges, options.max_ranges)
    }

    fn normalized_lines(input: &str) -> Vec<&str> {
        input
            .trim_end_matches(char::from(13))
            .trim_end_matches(char::from(10))
            .split(char::from(10))
            .map(|line| line.trim_end_matches(char::from(13)))
            .filter(|line| !(line.is_empty() && input.is_empty()))
            .collect()
    }

    fn counts_for_line(line: &str) -> CriticalSignalCounts {
        let trimmed = line.trim_start();
        let lower = trimmed.to_ascii_lowercase();
        CriticalSignalCounts {
            errors: usize::from(is_error_line(trimmed, &lower)),
            file_locations: count_file_locations(trimmed),
            diff_hunks: usize::from(trimmed.starts_with("@@ ") && trimmed[3..].contains("@@")),
            test_failures: usize::from(is_test_failure(trimmed, &lower)),
            exit_codes: usize::from(is_exit_code(trimmed, &lower)),
            stack_markers: usize::from(is_stack_marker(trimmed, &lower)),
            rust_diagnostics: usize::from(is_rust_diagnostic(trimmed, &lower)),
        }
    }

    fn add_assign(target: &mut CriticalSignalCounts, other: CriticalSignalCounts) {
        target.errors = target.errors.saturating_add(other.errors);
        target.file_locations = target.file_locations.saturating_add(other.file_locations);
        target.diff_hunks = target.diff_hunks.saturating_add(other.diff_hunks);
        target.test_failures = target.test_failures.saturating_add(other.test_failures);
        target.exit_codes = target.exit_codes.saturating_add(other.exit_codes);
        target.stack_markers = target.stack_markers.saturating_add(other.stack_markers);
        target.rust_diagnostics = target
            .rust_diagnostics
            .saturating_add(other.rust_diagnostics);
    }

    fn loss(before: CriticalSignalCounts, after: CriticalSignalCounts) -> CriticalSignalCounts {
        CriticalSignalCounts {
            errors: before.errors.saturating_sub(after.errors),
            file_locations: before.file_locations.saturating_sub(after.file_locations),
            diff_hunks: before.diff_hunks.saturating_sub(after.diff_hunks),
            test_failures: before.test_failures.saturating_sub(after.test_failures),
            exit_codes: before.exit_codes.saturating_sub(after.exit_codes),
            stack_markers: before.stack_markers.saturating_sub(after.stack_markers),
            rust_diagnostics: before
                .rust_diagnostics
                .saturating_sub(after.rust_diagnostics),
        }
    }

    fn subtract_assign(target: &mut CriticalSignalCounts, other: CriticalSignalCounts) {
        target.errors = target.errors.saturating_sub(other.errors);
        target.file_locations = target.file_locations.saturating_sub(other.file_locations);
        target.diff_hunks = target.diff_hunks.saturating_sub(other.diff_hunks);
        target.test_failures = target.test_failures.saturating_sub(other.test_failures);
        target.exit_codes = target.exit_codes.saturating_sub(other.exit_codes);
        target.stack_markers = target.stack_markers.saturating_sub(other.stack_markers);
        target.rust_diagnostics = target
            .rust_diagnostics
            .saturating_sub(other.rust_diagnostics);
    }

    fn overlaps(left: CriticalSignalCounts, right: CriticalSignalCounts) -> bool {
        left.errors > 0 && right.errors > 0
            || left.file_locations > 0 && right.file_locations > 0
            || left.diff_hunks > 0 && right.diff_hunks > 0
            || left.test_failures > 0 && right.test_failures > 0
            || left.exit_codes > 0 && right.exit_codes > 0
            || left.stack_markers > 0 && right.stack_markers > 0
            || left.rust_diagnostics > 0 && right.rust_diagnostics > 0
    }

    fn is_error_line(trimmed: &str, lower: &str) -> bool {
        lower.starts_with("error:")
            || lower.starts_with("error[")
            || lower.starts_with("fatal:")
            || lower.starts_with("panic:")
            || lower.starts_with("npm err!")
            || lower.starts_with("npm error")
            || lower.starts_with("pnpm error")
            || lower.starts_with("yarn error")
            || lower.starts_with("bun error")
            || lower.starts_with("failed ")
            || lower.starts_with("fail ")
            || trimmed.starts_with("E   ")
            || lower.starts_with("thread '") && lower.contains("' panicked at")
            || lower.contains(" status=error")
            || lower.starts_with("status=error")
            || lower.contains(" level=error")
            || lower.starts_with("level=error")
            || trimmed.contains(r#""error""#)
            || trimmed.contains(r#""type":"error""#)
            || trimmed.contains(r#""type": "error""#)
    }

    fn is_test_failure(trimmed: &str, lower: &str) -> bool {
        trimmed.contains(" ... FAILED")
            || lower.starts_with("test result: failed")
            || lower.starts_with("failures:")
            || lower.starts_with("failed tests:")
            || lower.starts_with("failed:")
    }

    fn is_exit_code(trimmed: &str, lower: &str) -> bool {
        lower.contains("exited with code ")
            || lower.contains("exit code ")
            || lower.starts_with("process exited ")
            || trimmed.starts_with("Process exited with code ")
    }

    fn is_stack_marker(trimmed: &str, lower: &str) -> bool {
        lower.starts_with("stack backtrace:")
            || lower.starts_with("stack trace:")
            || lower.starts_with("backtrace:")
            || lower.starts_with("traceback (most recent call last):")
            || lower.starts_with("caused by:")
            || trimmed.split_once(':').is_some_and(|(index, _)| {
                !index.is_empty() && index.trim().chars().all(|c| c.is_ascii_digit())
            })
    }

    fn is_rust_diagnostic(trimmed: &str, lower: &str) -> bool {
        lower.starts_with("error[e")
            || lower.starts_with("warning:")
            || lower.starts_with("warning[")
            || trimmed.starts_with("--> ")
            || trimmed.starts_with("::: ")
            || trimmed.starts_with("= note:")
            || trimmed.starts_with("= help:")
            || lower.starts_with("help:")
            || lower.starts_with("note:")
            || lower.contains("clippy::")
    }

    fn count_file_locations(line: &str) -> usize {
        line.split_whitespace()
            .filter(|token| token_contains_location(token))
            .count()
            + usize::from(python_location(line))
            + usize::from(paren_location(line))
    }

    fn token_contains_location(candidate: &str) -> bool {
        let candidate = candidate
            .trim_matches(|ch: char| {
                matches!(ch, '"' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}')
                    || ch == char::from(39)
            })
            .trim_end_matches([':', '.']);
        if candidate.contains("://") {
            return false;
        }

        let mut parts = candidate.rsplitn(3, ':');
        let tail = parts.next().unwrap_or_default();
        let middle = parts.next().unwrap_or_default();
        let path = parts.next().unwrap_or_default();
        if tail.is_empty() || !tail.chars().all(|ch| ch.is_ascii_digit()) {
            return false;
        }
        if !middle.is_empty() && middle.chars().all(|ch| ch.is_ascii_digit()) {
            return looks_like_path(path);
        }
        looks_like_path(middle)
    }

    fn python_location(line: &str) -> bool {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix(r#"File ""#) else {
            return false;
        };
        let Some((path, after)) = rest.split_once(char::from(34)) else {
            return false;
        };
        looks_like_path(path)
            && after.split_once(", line ").is_some_and(|(_, number)| {
                number.chars().next().is_some_and(|ch| ch.is_ascii_digit())
            })
    }

    fn paren_location(line: &str) -> bool {
        line.split_whitespace().any(|token| {
            let Some((path, rest)) = token.split_once('(') else {
                return false;
            };
            let Some((position, _)) = rest.split_once(')') else {
                return false;
            };
            let Some((line, column)) = position.split_once(',') else {
                return false;
            };
            looks_like_path(path)
                && !line.trim().is_empty()
                && line.trim().chars().all(|ch| ch.is_ascii_digit())
                && !column.trim().is_empty()
                && column.trim().chars().all(|ch| ch.is_ascii_digit())
        })
    }

    fn looks_like_path(path: &str) -> bool {
        let path = path.trim_matches(|ch: char| matches!(ch, '<' | '>' | '-' | ':' | ' '));
        if path.is_empty() {
            return false;
        }
        path.contains('/')
            || path.contains(char::from(92))
            || path.rsplit('/').next().is_some_and(|name| {
                name.rsplit_once('.').is_some_and(|(_, extension)| {
                    !extension.is_empty()
                        && extension.len() <= 12
                        && extension
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
                })
            })
    }

    fn range_around(
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

    fn merge_ranges(
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
                && range.start <= last.end.saturating_add(1)
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
}
#[cfg(not(feature = "mojo"))]
pub(super) fn count_critical_signals(input: &str) -> super::CriticalSignalCounts {
    implementation::count_critical_signals(input)
}

#[cfg(not(feature = "mojo"))]
pub(super) fn signal_diff(
    before: super::CriticalSignalCounts,
    after: super::CriticalSignalCounts,
) -> (super::CriticalSignalCounts, super::CriticalSignalCounts) {
    implementation::signal_diff(before, after)
}

#[cfg(not(feature = "mojo"))]
pub(super) fn lost_line_ranges(
    before: &str,
    after: &str,
    lost: super::CriticalSignalCounts,
    options: super::CriticalSignalLineRangeOptions,
) -> Vec<super::CriticalSignalLineRange> {
    implementation::lost_line_ranges(before, after, lost, options)
}
