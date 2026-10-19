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
    fn prodex_mojo_log_tui_key_plan_v1(
        version: i64,
        key_kind: i64,
        scalar: i64,
        control: i64,
        editing_search: i64,
        scroll_from_bottom: u64,
        max_scroll: u64,
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

/// Terminal-decoded key shape, without crossterm or input-policy semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTuiKey {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Other,
}

/// The only mutations that a compiled input plan may request from its Rust host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTuiSearchAction {
    Unchanged,
    Clear,
    Pop,
    Push,
}

/// Mojo-owned state transition; Rust only applies this validated plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogTuiKeyPlan {
    pub quit: bool,
    pub editing_search: bool,
    pub search_action: LogTuiSearchAction,
    pub scroll_from_bottom: usize,
}

pub fn log_tui_key_plan(
    key: LogTuiKey,
    control: bool,
    editing_search: bool,
    scroll_from_bottom: usize,
) -> Result<LogTuiKeyPlan, MojoError> {
    let (kind, scalar) = match key {
        LogTuiKey::Char(ch) => (0, i64::from(u32::from(ch))),
        LogTuiKey::Enter => (1, 0),
        LogTuiKey::Esc => (2, 0),
        LogTuiKey::Backspace => (3, 0),
        LogTuiKey::Up => (4, 0),
        LogTuiKey::Down => (5, 0),
        LogTuiKey::PageUp => (6, 0),
        LogTuiKey::PageDown => (7, 0),
        LogTuiKey::Home => (8, 0),
        LogTuiKey::End => (9, 0),
        LogTuiKey::Other => (10, 0),
    };
    let mut output = [u64::MAX; 4];
    status(unsafe {
        prodex_mojo_log_tui_key_plan_v1(
            ABI_VERSION,
            kind,
            scalar,
            i64::from(control),
            i64::from(editing_search),
            u64::try_from(scroll_from_bottom).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(usize::MAX).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let boolean = |value| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let search_action = match output[2] {
        0 => LogTuiSearchAction::Unchanged,
        1 => LogTuiSearchAction::Clear,
        2 => LogTuiSearchAction::Pop,
        3 if matches!(key, LogTuiKey::Char(_)) => LogTuiSearchAction::Push,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(LogTuiKeyPlan {
        quit: boolean(output[0])?,
        editing_search: boolean(output[1])?,
        search_action,
        scroll_from_bottom: usize::try_from(output[3]).map_err(|_| MojoError::InvalidOutput)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_log_tui_key_state_machine_preserves_control_and_unicode() {
        use LogTuiKey::*;
        use LogTuiSearchAction::*;
        for (key, control, editing, scroll, quit, next_editing, action, next_scroll) in [
            (Up, false, false, 0, false, false, Unchanged, 1),
            (Char('k'), true, false, 2, false, false, Unchanged, 3),
            (Down, false, false, 0, false, false, Unchanged, 0),
            (
                PageUp,
                false,
                false,
                usize::MAX - 5,
                false,
                false,
                Unchanged,
                usize::MAX,
            ),
            (PageDown, false, false, 8, false, false, Unchanged, 0),
            (Home, false, false, 9, false, false, Unchanged, usize::MAX),
            (End, false, false, 9, false, false, Unchanged, 0),
            (Char('/'), false, false, 8, false, true, Clear, 0),
            (Char('c'), false, false, 8, false, false, Clear, 0),
            (Char('c'), true, false, 8, true, false, Unchanged, 8),
            (Char('z'), true, true, 8, true, true, Unchanged, 8),
            (Char('q'), false, false, 8, true, false, Unchanged, 8),
            (Char('q'), true, true, 8, false, true, Unchanged, 8),
            (Char('🚀'), false, true, 8, false, true, Push, 0),
            (Char('界'), false, true, 8, false, true, Push, 0),
            (Backspace, false, true, 8, false, true, Pop, 0),
            (Char('u'), true, true, 8, false, true, Clear, 0),
            (Enter, false, true, 8, false, false, Unchanged, 8),
            (Esc, false, true, 8, false, false, Unchanged, 8),
            (Esc, false, false, 8, true, false, Unchanged, 8),
            (Other, false, false, 8, false, false, Unchanged, 8),
            (Char('x'), true, true, 8, false, true, Unchanged, 8),
        ] {
            assert_eq!(
                log_tui_key_plan(key, control, editing, scroll),
                Ok(LogTuiKeyPlan {
                    quit,
                    editing_search: next_editing,
                    search_action: action,
                    scroll_from_bottom: next_scroll,
                }),
                "{key:?}, control={control}, editing={editing}, scroll={scroll}",
            );
        }
    }

    #[test]
    fn compiled_log_tui_key_abi_rejects_invalid_inputs_before_writing() {
        let mut output = [91_u64; 4];
        for (version, kind, scalar, control, editing, scroll, max, expected) in [
            (99, 0, 97, 0, 0, 0, u64::MAX, 4),
            (1, -1, 0, 0, 0, 0, u64::MAX, 1),
            (1, 11, 0, 0, 0, 0, u64::MAX, 1),
            (1, 0, 0x11_0000, 0, 0, 0, u64::MAX, 1),
            (1, 0, 0xD800, 0, 0, 0, u64::MAX, 1),
            (1, 3, 1, 0, 0, 0, u64::MAX, 1),
            (1, 0, 97, 2, 0, 0, u64::MAX, 1),
            (1, 0, 97, 0, -1, 0, u64::MAX, 1),
            (1, 0, 97, 0, 0, 5, 4, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_mojo_log_tui_key_plan_v1(
                        version,
                        kind,
                        scalar,
                        control,
                        editing,
                        scroll,
                        max,
                        output.as_mut_ptr() as usize as u64,
                    )
                },
                expected
            );
            assert_eq!(output, [91_u64; 4]);
        }
        assert_eq!(
            unsafe { prodex_mojo_log_tui_key_plan_v1(1, 0, 97, 0, 0, 0, u64::MAX, 0) },
            1
        );
    }

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
