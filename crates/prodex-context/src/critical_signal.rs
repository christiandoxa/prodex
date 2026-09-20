#[cfg(not(feature = "mojo"))]
#[path = "critical_signal/rust_oracle.rs"]
mod rust_oracle;

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
            .expect("Mojo context analysis returned invalid structured output");
        return CriticalSignalCounts {
            errors: analysis.counts[0],
            file_locations: analysis.counts[1],
            diff_hunks: analysis.counts[2],
            test_failures: analysis.counts[3],
            exit_codes: analysis.counts[4],
            stack_markers: analysis.counts[5],
            rust_diagnostics: analysis.counts[6],
        };
    }
    #[cfg(not(feature = "mojo"))]
    rust_oracle::count_critical_signals(input)
}

pub fn critical_signal_self_check(before: &str, after: &str) -> CriticalSignalSelfCheck {
    let before = count_critical_signals(before);
    let after = count_critical_signals(after);
    #[cfg(feature = "mojo")]
    let (lost, gained) = prodex_mojo_core::context::signal_diff(&before.values(), &after.values())
        .expect("Mojo critical-signal diff returned invalid output");
    #[cfg(not(feature = "mojo"))]
    let (lost, gained) = rust_oracle::signal_diff(before, after);
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
        return prodex_mojo_core::context::lost_line_ranges_batch(
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
        .collect();
    }
    #[cfg(not(feature = "mojo"))]
    rust_oracle::lost_line_ranges(before, after, check.lost, options)
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

    #[test]
    fn mojo_counts_and_diff_preserve_critical_signal_contract() {
        let before = "error[E0308]: mismatch\nsrc/lib.rs:12:5\nprocess exited with code 1\n";
        let after = "error[E0308]: mismatch\n";
        let check = critical_signal_self_check(before, after);
        assert!(check.has_loss());
        assert!(check.before.total() > check.after.total());
        assert!(check.lost.file_locations > 0 || check.lost.exit_codes > 0);
    }

    #[test]
    fn mojo_lost_ranges_are_one_based_and_bounded() {
        let before = "head\nerror: failed\nsrc/lib.rs:2:1\ntail\n";
        let ranges = critical_signal_lost_line_ranges(before, "");
        assert!(!ranges.is_empty());
        assert!(
            ranges
                .iter()
                .all(|range| range.start >= 1 && range.end >= range.start)
        );
    }

    #[test]
    fn unchanged_text_passes() {
        let text = "error: stable\nsrc/lib.rs:2:1\n";
        assert!(critical_signal_self_check(text, text).passed());
    }
}
