//! Mojo-owned deterministic profile-screen geometry and value-color policy.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const OP_NORMALIZE_HEIGHT: i64 = 0;
const OP_SCROLL_BODY_HEIGHT: i64 = 1;
const OP_SCROLL_MAX_OFFSET: i64 = 2;
const OP_VALUE_COLOR: i64 = 3;
const OP_TUI_HEIGHT: i64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ProfileValueColor {
    Reset = 0,
    Red = 1,
    Green = 2,
    Cyan = 3,
}

unsafe extern "C" {
    fn prodex_profile_ui_numeric_v1(
        abi_version: i64,
        operation: i64,
        input0: i64,
        input1: i64,
        label_address: u64,
        label_length: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
}

fn call(
    operation: i64,
    input0: usize,
    input1: usize,
    label: &str,
    value: &str,
) -> Result<i64, MojoError> {
    let mut output = [0_i64; 1];
    let status = unsafe {
        prodex_profile_ui_numeric_v1(
            ABI_VERSION,
            operation,
            i64::try_from(input0).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(input1).map_err(|_| MojoError::InvalidInput)?,
            label.as_ptr() as usize as u64,
            i64::try_from(label.len()).map_err(|_| MojoError::InvalidInput)?,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(output[0]),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn nonnegative(value: i64) -> Result<usize, MojoError> {
    usize::try_from(value).map_err(|_| MojoError::InvalidOutput)
}

/// Normalize the terminal height used by profile views.
pub fn normalized_terminal_height(height: u16) -> Result<usize, MojoError> {
    nonnegative(call(OP_NORMALIZE_HEIGHT, usize::from(height), 0, "", "")?)
}

/// Return the scrollable body height after the profile header and footer.
pub fn scroll_body_height(terminal_height: u16) -> Result<usize, MojoError> {
    nonnegative(call(
        OP_SCROLL_BODY_HEIGHT,
        usize::from(terminal_height),
        0,
        "",
        "",
    )?)
}

/// Return the largest valid scroll offset for a body.
pub fn scroll_max_offset(total_lines: usize, body_height: usize) -> Result<usize, MojoError> {
    nonnegative(call(
        OP_SCROLL_MAX_OFFSET,
        total_lines,
        body_height,
        "",
        "",
    )?)
}

/// Select the stable semantic color class for a profile value.
pub fn value_color(label: &str, value: &str) -> Result<ProfileValueColor, MojoError> {
    match call(OP_VALUE_COLOR, 0, 0, label, value)? {
        0 => Ok(ProfileValueColor::Reset),
        1 => Ok(ProfileValueColor::Red),
        2 => Ok(ProfileValueColor::Green),
        3 => Ok(ProfileValueColor::Cyan),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Bound the inline profile TUI to the available terminal height.
pub fn tui_height(rows: usize, terminal_height: u16) -> Result<u16, MojoError> {
    let value = nonnegative(call(
        OP_TUI_HEIGHT,
        rows.min(i64::MAX as usize), // transport-bound only; Mojo owns height policy
        usize::from(terminal_height),
        "",
        "",
    )?)?;
    u16::try_from(value).map_err(|_| MojoError::InvalidOutput)
}
