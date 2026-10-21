//! Checked compiled decoded JSONL session framing and compression suffix policy.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_NAME_BYTES: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_session_file_line_frame_v1(
        abi_version: i64,
        decoded_before: u64,
        scan_start: u64,
        maximum_scan_bytes: u64,
        line_address: u64,
        line_length: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_file_compressed_suffix_v1(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
        output_address: u64,
    ) -> i64;
}

fn status(code: i64) -> Result<(), MojoError> {
    match code {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLineFrameAction {
    Eof,
    TooLarge,
    Incomplete,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionLineFramePlan {
    pub action: SessionLineFrameAction,
    pub complete_offset: u64,
}

/// Decide decoded scan progress from the reader's original UTF-8 line bytes.
/// No host fallback repeats Mojo's bounds/line-termination decisions.
pub fn session_line_frame_plan(
    decoded_before: u64,
    scan_start: u64,
    maximum_scan_bytes: u64,
    line: &str,
) -> Result<SessionLineFramePlan, MojoError> {
    let mut output = [u64::MAX; 2];
    status(unsafe {
        prodex_session_file_line_frame_v1(
            ABI_VERSION,
            decoded_before,
            scan_start,
            maximum_scan_bytes,
            line.as_ptr() as usize as u64,
            u64::try_from(line.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let action = match output[0] {
        0 => SessionLineFrameAction::Eof,
        1 => SessionLineFrameAction::TooLarge,
        2 => SessionLineFrameAction::Incomplete,
        3 => SessionLineFrameAction::Complete,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(SessionLineFramePlan {
        action,
        complete_offset: output[1],
    })
}

/// Exact compressed JSONL suffix selection in compiled Mojo.
pub fn session_file_is_compressed_name(name: &str) -> Result<bool, MojoError> {
    if name.len() > MAX_NAME_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut selected = -1_i64;
    status(unsafe {
        prodex_session_file_compressed_suffix_v1(
            ABI_VERSION,
            name.as_ptr() as usize as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut selected as *mut i64) as usize as u64,
        )
    })?;
    match selected {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_scan_frame_preserves_eof_incomplete_budget_and_saturation() {
        use SessionLineFrameAction as A;
        let plan = |before, start, maximum, line| {
            session_line_frame_plan(before, start, maximum, line).unwrap()
        };
        assert_eq!(
            plan(5, 5, 2, ""),
            SessionLineFramePlan {
                action: A::Eof,
                complete_offset: 5
            }
        );
        assert_eq!(
            plan(5, 5, 2, "a"),
            SessionLineFramePlan {
                action: A::Incomplete,
                complete_offset: 5
            }
        );
        assert_eq!(
            plan(5, 5, 2, "a\n"),
            SessionLineFramePlan {
                action: A::Complete,
                complete_offset: 7
            }
        );
        assert_eq!(
            plan(5, 5, 1, "a\n"),
            SessionLineFramePlan {
                action: A::TooLarge,
                complete_offset: 7
            }
        );
        assert_eq!(
            plan(5, 5, 2, "a\nb"),
            SessionLineFramePlan {
                action: A::TooLarge,
                complete_offset: 8
            }
        );
        assert_eq!(plan(u64::MAX, 0, 0, "a\n").action, A::TooLarge);
        assert_eq!(
            plan(u64::MAX, u64::MAX, 5, "x"),
            SessionLineFramePlan {
                action: A::Incomplete,
                complete_offset: u64::MAX - 1,
            }
        );
        assert_eq!(
            plan(u64::MAX - 2, u64::MAX - 2, 10, "abcd\n"),
            SessionLineFramePlan {
                action: A::Complete,
                complete_offset: u64::MAX
            }
        );
    }

    #[test]
    fn compiled_compressed_suffix_is_exact_and_case_sensitive() {
        for (name, expected) in [
            ("rollout.jsonl.zst", true),
            (".jsonl.zst", true),
            ("ROLLout.jsonl.zst", true),
            ("rollout.JSONL.ZST", false),
            ("rollout.jsonl.zst.backup", false),
            ("rollout.jsonl", false),
            ("", false),
        ] {
            assert_eq!(session_file_is_compressed_name(name), Ok(expected));
        }
    }
}
