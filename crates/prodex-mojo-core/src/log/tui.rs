//! Typed Mojo ABI for log TUI snapshot, viewport, and event-source color policy.
//! All deterministic decisions are compiled in log_semantics.mojo.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_mojo_log_tui_window_v1(
        version: i64,
        line_count: u64,
        max_lines: u64,
        scroll_from_bottom: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_log_tui_snapshot_height_v1(
        version: i64,
        transcript_present: i64,
        transcript_rows: u64,
        upstream_present: i64,
        upstream_rows: u64,
        token_usage_present: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_log_tui_source_tone_v1(
        version: i64,
        source_address: u64,
        source_length: i64,
        output_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Inclusive start and exclusive end of the visible log lines.
pub fn log_tui_window(
    line_count: usize,
    max_lines: usize,
    scroll_from_bottom: usize,
) -> Result<std::ops::Range<usize>, MojoError> {
    let mut output = [u64::MAX; 2];
    status(unsafe {
        prodex_mojo_log_tui_window_v1(
            ABI_VERSION,
            u64::try_from(line_count).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(max_lines).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(scroll_from_bottom).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let start = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    if start > end || end > line_count || end - start > max_lines {
        return Err(MojoError::InvalidOutput);
    }
    Ok(start..end)
}

/// Computes a clamped snapshot height using host-measured rendered line counts.
pub fn log_tui_snapshot_height(
    transcript_rows: Option<usize>,
    upstream_rows: Option<usize>,
    token_usage_present: bool,
) -> Result<u16, MojoError> {
    let mut output = u64::MAX;
    status(unsafe {
        prodex_mojo_log_tui_snapshot_height_v1(
            ABI_VERSION,
            i64::from(transcript_rows.is_some()),
            u64::try_from(transcript_rows.unwrap_or(0)).map_err(|_| MojoError::InvalidInput)?,
            i64::from(upstream_rows.is_some()),
            u64::try_from(upstream_rows.unwrap_or(0)).map_err(|_| MojoError::InvalidInput)?,
            i64::from(token_usage_present),
            (&mut output as *mut u64) as usize as u64,
        )
    })?;
    let height = u16::try_from(output).map_err(|_| MojoError::InvalidOutput)?;
    if !(8..=28).contains(&height) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(height)
}

/// Terminal palette meaning for compiled Mojo's source classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTuiSourceTone {
    Reset,
    Green,
    Cyan,
    Yellow,
    Blue,
    LightBlue,
    LightYellow,
    LightMagenta,
    Magenta,
    Red,
}

pub fn log_tui_source_tone(source: &str) -> Result<LogTuiSourceTone, MojoError> {
    let mut output = -1_i64;
    status(unsafe {
        prodex_mojo_log_tui_source_tone_v1(
            ABI_VERSION,
            source.as_ptr() as usize as u64,
            i64::try_from(source.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut output as *mut i64) as usize as u64,
        )
    })?;
    match output {
        0 => Ok(LogTuiSourceTone::Reset),
        1 => Ok(LogTuiSourceTone::Green),
        2 => Ok(LogTuiSourceTone::Cyan),
        3 => Ok(LogTuiSourceTone::Yellow),
        4 => Ok(LogTuiSourceTone::Blue),
        5 => Ok(LogTuiSourceTone::LightBlue),
        6 => Ok(LogTuiSourceTone::LightYellow),
        7 => Ok(LogTuiSourceTone::LightMagenta),
        8 => Ok(LogTuiSourceTone::Magenta),
        9 => Ok(LogTuiSourceTone::Red),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_log_tui_window_handles_edges_and_maximum_scroll() {
        for (count, max, scroll, expected) in [
            (0, 10, 0, 0..0),
            (3, 10, 99, 0..3),
            (10, 4, 0, 6..10),
            (10, 4, 2, 4..8),
            (10, 4, usize::MAX, 0..4),
            (10, 0, 9, 0..0),
            (usize::MAX, 2, 0, (usize::MAX - 2)..usize::MAX),
        ] {
            assert_eq!(log_tui_window(count, max, scroll), Ok(expected));
        }
    }

    #[test]
    fn compiled_log_tui_snapshot_height_clamps_measured_content() {
        for (transcript, upstream, usage, expected) in [
            (None, None, false, 8),
            (None, None, true, 8),
            (Some(0), None, false, 8),
            (Some(1), None, false, 8),
            (Some(4), Some(2), false, 16),
            (Some(24), None, false, 28),
            (None, Some(usize::MAX), true, 28),
        ] {
            assert_eq!(
                log_tui_snapshot_height(transcript, upstream, usage),
                Ok(expected)
            );
        }
    }

    #[test]
    fn compiled_source_tone_preserves_exact_case_and_tool_prefix() {
        for (source, tone) in [
            ("user", LogTuiSourceTone::Green),
            ("assistant", LogTuiSourceTone::Cyan),
            ("reasoning", LogTuiSourceTone::Yellow),
            ("turn-context", LogTuiSourceTone::Blue),
            ("smart", LogTuiSourceTone::LightBlue),
            ("quota", LogTuiSourceTone::LightYellow),
            ("health", LogTuiSourceTone::LightMagenta),
            ("tool-output", LogTuiSourceTone::Magenta),
            ("tool-call:🧭", LogTuiSourceTone::Magenta),
            ("error", LogTuiSourceTone::Red),
            ("TOOL", LogTuiSourceTone::Reset),
            ("tool-output-extra", LogTuiSourceTone::Reset),
            ("🧭", LogTuiSourceTone::Reset),
            ("", LogTuiSourceTone::Reset),
        ] {
            assert_eq!(log_tui_source_tone(source), Ok(tone), "{source:?}");
        }
    }
}
