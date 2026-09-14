use serde::Serialize;
#[cfg(any(not(feature = "mojo"), test))]
use std::collections::BTreeMap;

#[cfg(any(not(feature = "mojo"), test))]
#[path = "critical_signal/oracle.rs"]
pub(crate) mod oracle;
#[cfg(any(not(feature = "mojo"), test))]
use oracle::*;

#[cfg(any(not(feature = "mojo"), test))]
use crate::command_output::is_rust_exit_status_line;
use crate::{command_lines, normalize_command_output};
#[cfg(any(not(feature = "mojo"), test))]
use crate::{
    generic_failed_test_name, has_zero_only_summary_count, is_eslint_diagnostic_line,
    is_exception_signal_line, is_junit_xml_failure_line, is_log_level_signal_line,
    is_rust_backtrace_start, is_rust_failure_summary_line, is_rust_panic_line,
    is_typescript_diagnostic_line, rust_diagnostic_severity, rust_failed_test_name,
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
            counts.add_assign(rust_critical_signal_counts_for_line(line));
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
        .unwrap_or_else(|error| {
            panic!(
                "Mojo critical-signal range selection returned invalid output: {error:?}; before_rows={} after_available={} line_count={} lost={:?} options={:?}",
                before_rows.len(),
                after_available.len(),
                line_count,
                check.lost.values(),
                options,
            )
        })
        .into_iter()
        .map(|(start, end)| CriticalSignalLineRange { start, end })
        .collect::<Vec<_>>()
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

            let counts = rust_critical_signal_counts_for_line(line);
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
    let before_text = before_lines
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>();
    let after_text = after_lines
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>();
    let before_counts = prodex_mojo_core::rich::signal_counts_batch(&before_text)
        .expect("Mojo critical-signal batch classification returned invalid output");
    let after_counts = prodex_mojo_core::rich::signal_counts_batch(&after_text)
        .expect("Mojo critical-signal batch classification returned invalid output");
    let before_input = before_lines
        .iter()
        .zip(before_counts)
        .map(
            |(line, counts)| prodex_mojo_core::context::ContextSignalLine {
                text: line.trim(),
                counts,
            },
        )
        .collect::<Vec<_>>();
    let after_input = after_lines
        .iter()
        .zip(after_counts)
        .map(
            |(line, counts)| prodex_mojo_core::context::ContextSignalLine {
                text: line.trim(),
                counts,
            },
        )
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
        let counts = rust_critical_signal_counts_for_line(line);
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
        let counts = rust_critical_signal_counts_for_line(line);
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
#[path = "critical_signal_tests.rs"]
mod mojo_text_rows_tests;
#[cfg(feature = "mojo")]
fn mojo_critical_signal_counts_for_line(line: &str) -> [usize; 7] {
    prodex_mojo_core::rich::signal_counts_batch(&[line])
        .expect("Mojo critical-signal line classification returned an invalid result")
        .into_iter()
        .next()
        .expect("Mojo critical-signal line classification returned no result")
}

pub(crate) fn is_error_signal_line(line: &str) -> bool {
    #[cfg(feature = "mojo")]
    {
        mojo_critical_signal_counts_for_line(line)[0] > 0
    }
    #[cfg(not(feature = "mojo"))]
    {
        rust_is_error_signal_line(line)
    }
}

pub(crate) fn count_file_location_signals(line: &str) -> usize {
    #[cfg(feature = "mojo")]
    {
        mojo_critical_signal_counts_for_line(line)[1]
    }
    #[cfg(not(feature = "mojo"))]
    {
        rust_count_file_location_signals(line)
    }
}

pub(crate) fn is_diff_hunk_line(line: &str) -> bool {
    #[cfg(feature = "mojo")]
    {
        mojo_critical_signal_counts_for_line(line)[2] > 0
    }
    #[cfg(not(feature = "mojo"))]
    {
        rust_is_diff_hunk_line(line)
    }
}

pub(crate) fn is_test_failure_signal_line(line: &str) -> bool {
    #[cfg(feature = "mojo")]
    {
        mojo_critical_signal_counts_for_line(line)[3] > 0
    }
    #[cfg(not(feature = "mojo"))]
    {
        rust_is_test_failure_signal_line(line)
    }
}

pub(crate) fn is_stack_signal_line(line: &str) -> bool {
    #[cfg(feature = "mojo")]
    {
        mojo_critical_signal_counts_for_line(line)[5] > 0
    }
    #[cfg(not(feature = "mojo"))]
    {
        rust_is_stack_signal_line(line)
    }
}

pub(crate) fn is_rust_diagnostic_signal_line(line: &str) -> bool {
    #[cfg(feature = "mojo")]
    {
        mojo_critical_signal_counts_for_line(line)[6] > 0
    }
    #[cfg(not(feature = "mojo"))]
    {
        rust_is_diagnostic_signal_line(line)
    }
}

pub(crate) fn looks_like_location_path(path: &str) -> bool {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::context::looks_like_location_path(path)
            .expect("Mojo location-path classification returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        rust_looks_like_location_path(path)
    }
}
