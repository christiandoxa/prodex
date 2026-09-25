use serde::Serialize;

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
        critical_signal_available() && self.lost.is_empty()
    }

    pub fn has_loss(self) -> bool {
        !self.passed()
    }
}

/// Reports whether Mojo critical-signal operations are compiled into this crate.
///
/// Without the `mojo` feature, counts are empty, ranges are unavailable, and
/// self-checks fail closed.
pub const fn critical_signal_available() -> bool {
    cfg!(feature = "mojo")
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
            .expect("Mojo context analysis returned invalid structured output");
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
        let _ = input;
        CriticalSignalCounts::default()
    }
}

pub fn critical_signal_self_check(before: &str, after: &str) -> CriticalSignalSelfCheck {
    let before = count_critical_signals(before);
    let after = count_critical_signals(after);
    #[cfg(feature = "mojo")]
    let (lost, gained) = prodex_mojo_core::context::signal_diff(&before.values(), &after.values())
        .expect("Mojo critical-signal diff returned invalid output");
    #[cfg(not(feature = "mojo"))]
    let (lost, gained) = (
        CriticalSignalCounts::default(),
        CriticalSignalCounts::default(),
    );
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
            critical_signal_rows(before, after).expect("critical-signal rows fit the Mojo ABI");
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
        .collect()
    }
    #[cfg(not(feature = "mojo"))]
    {
        let _ = (before, after, options);
        Vec::new()
    }
}

#[cfg(feature = "mojo")]
fn critical_signal_rows(
    before: &str,
    after: &str,
) -> Result<(Vec<i64>, Vec<i64>, usize), prodex_mojo_core::MojoError> {
    let before = prodex_mojo_core::context::normalize_command_output(before)?;
    let after = prodex_mojo_core::context::normalize_command_output(after)?;
    let before_lines = lines(&before);
    let after_lines = lines(&after);
    let before_text = before_lines
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>();
    let after_text = after_lines
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>();
    let before_counts = prodex_mojo_core::rich::signal_counts_batch(&before_text)?;
    let after_counts = prodex_mojo_core::rich::signal_counts_batch(&after_text)?;
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

#[cfg(feature = "mojo")]
fn lines(input: &str) -> Vec<&str> {
    input
        .trim_end_matches('\n')
        .split('\n')
        .filter(|line| !(line.is_empty() && input.is_empty()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "mojo")]
    #[test]
    fn mojo_counts_and_diff_match_expected_fixture() {
        let before = concat!(
            "error[E0308]: mismatch\n",
            "noise\n",
            "src/lib.rs:12:5\n",
            "noise\n",
            "@@ -1,2 +1,3 @@\n",
            "noise\n",
            "test parses ... FAILED\n",
            "noise\n",
            "exit code 1\n",
            "noise\n",
            "stack backtrace:\n",
            "noise\n",
            "warning: unused variable\n",
        );
        let after = "error[E0308]: mismatch\nnoise\n";
        let expected_before = CriticalSignalCounts {
            errors: 1,
            file_locations: 1,
            diff_hunks: 1,
            test_failures: 1,
            exit_codes: 1,
            stack_markers: 1,
            rust_diagnostics: 2,
        };
        let expected_after = CriticalSignalCounts {
            errors: 1,
            rust_diagnostics: 1,
            ..CriticalSignalCounts::default()
        };
        let expected = CriticalSignalSelfCheck {
            before: expected_before,
            after: expected_after,
            lost: CriticalSignalCounts {
                file_locations: 1,
                diff_hunks: 1,
                test_failures: 1,
                exit_codes: 1,
                stack_markers: 1,
                rust_diagnostics: 1,
                ..CriticalSignalCounts::default()
            },
            gained: CriticalSignalCounts::default(),
        };

        assert!(critical_signal_available());
        assert_eq!(count_critical_signals(before), expected_before);
        assert_eq!(count_critical_signals(after), expected_after);
        let check = critical_signal_self_check(before, after);
        assert_eq!(check, expected);
        assert!(check.has_loss());
    }

    #[cfg(feature = "mojo")]
    #[test]
    fn mojo_lost_ranges_match_duplicate_line_fixture() {
        let before = "head\nerror: duplicate\na\nb\nc\nerror: duplicate\ntail\n";
        let after = "error: duplicate\n";
        let ranges = critical_signal_lost_line_ranges_with_options(
            before,
            after,
            CriticalSignalLineRangeOptions {
                context_lines: 1,
                max_ranges: 32,
                max_range_lines: 6,
            },
        );
        assert_eq!(ranges, [CriticalSignalLineRange { start: 5, end: 7 }]);
    }

    #[cfg(feature = "mojo")]
    #[test]
    fn unchanged_text_passes() {
        let text = "error: stable\nsrc/lib.rs:2:1\n";
        assert!(critical_signal_self_check(text, text).passed());
    }

    #[cfg(not(feature = "mojo"))]
    #[test]
    fn critical_signal_capability_is_unavailable_without_mojo() {
        assert!(!critical_signal_available());
        assert!(count_critical_signals("error: dropped").is_empty());
        let check = critical_signal_self_check("error: dropped", "");
        assert!(!check.passed());
        assert!(check.has_loss());
        assert!(critical_signal_lost_line_ranges("error: dropped", "").is_empty());
    }
}
