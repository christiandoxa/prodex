//! Typed, fail-closed adapters for Mojo's authoritative terminal layout.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_terminal_panel_label_width_v1(
        abi: i64,
        longest: u64,
        total_width: u64,
        minimum: u64,
        maximum: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_terminal_session_columns_v1(
        abi: i64,
        total_width: u64,
        gap_width: u64,
        result_address: u64,
        result_capacity: i64,
    ) -> i64;
    fn prodex_terminal_panel_inline_height_v1(
        abi: i64,
        line_count: u64,
        result_address: u64,
    ) -> i64;
}

fn check_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn from_usize(value: usize) -> Result<u64, MojoError> {
    u64::try_from(value).map_err(|_| MojoError::InvalidInput)
}

/// Plan the label column using the already measured Unicode cell width.
pub fn panel_label_width(
    longest: usize,
    total_width: usize,
    min_label_width: usize,
    max_label_width: usize,
) -> Result<usize, MojoError> {
    let mut result = u64::MAX;
    check_status(unsafe {
        prodex_terminal_panel_label_width_v1(
            ABI_VERSION,
            from_usize(longest)?,
            from_usize(total_width)?,
            from_usize(min_label_width)?,
            from_usize(max_label_width)?,
            (&mut result as *mut u64) as usize as u64,
        )
    })?;
    let result = usize::try_from(result).map_err(|_| MojoError::InvalidOutput)?;
    if result == 0 || result > max_label_width {
        return Err(MojoError::InvalidOutput);
    }
    Ok(result)
}

/// Return [id, updated, thread, cwd, path] widths. No Rust layout fallback.
pub fn session_columns(total_width: usize, gap_width: usize) -> Result<[usize; 5], MojoError> {
    let mut result = [u64::MAX; 5];
    check_status(unsafe {
        prodex_terminal_session_columns_v1(
            ABI_VERSION,
            from_usize(total_width)?,
            from_usize(gap_width)?,
            result.as_mut_ptr() as usize as u64,
            5,
        )
    })?;
    let widths = result.map(|value| usize::try_from(value).map_err(|_| MojoError::InvalidOutput));
    let [id, updated, thread, cwd, path] = widths;
    let widths = [id?, updated?, thread?, cwd?, path?];
    if widths[..3].contains(&0) || widths[0] > 26 || widths[1] > 22 || widths[2] > 24 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(widths)
}

/// Bound a panel's inline terminal viewport to u16 after Mojo planning.
pub fn panel_inline_height(lines: usize) -> Result<u16, MojoError> {
    let mut result = u64::MAX;
    check_status(unsafe {
        prodex_terminal_panel_inline_height_v1(
            ABI_VERSION,
            from_usize(lines)?,
            (&mut result as *mut u64) as usize as u64,
        )
    })?;
    u16::try_from(result).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_policy_preserves_tiny_wide_and_unicode_precomputed_widths() {
        for (longest, total, minimum, maximum, expected) in [
            (16, 8, 10, 24, 1),
            (16, 24, 10, 24, 4),
            (20, 80, 10, 24, 20),
            (120, 80, 10, 24, 20),
            (16, 110, 10, 24, 16),
            (31, 110, 10, 24, 24),
            (3, 110, 10, 24, 10),
            (0, 0, 10, 24, 1),
        ] {
            assert_eq!(
                panel_label_width(longest, total, minimum, maximum).unwrap(),
                expected,
                "label width for {longest}/{total}"
            );
        }
    }

    #[test]
    fn session_columns_preserve_uneven_remainders_and_saturation() {
        for (total, gap, expected) in [
            (80, 8, [14, 14, 14, 15, 15]),
            (40, 8, [6, 6, 6, 7, 7]),
            (0, 8, [1, 1, 1, 1, 1]),
            (8, 8, [1, 1, 1, 1, 1]),
            (110, 8, [20, 20, 20, 21, 21]),
            (999, 8, [26, 22, 24, 459, 460]),
        ] {
            assert_eq!(session_columns(total, gap).unwrap(), expected);
        }
    }

    #[test]
    fn inline_height_is_bounded_by_mojo_even_at_host_max() {
        for (lines, expected) in [
            (0, 4),
            (1, 4),
            (4, 4),
            (7, 7),
            (65_535, 65_535),
            (65_536, 65_535),
            (usize::MAX, 65_535),
        ] {
            assert_eq!(panel_inline_height(lines).unwrap(), expected);
        }
    }

    #[test]
    fn abi_rejects_invalid_version_pointer_and_capacity() {
        let mut result = 9876u64;
        let address = (&mut result as *mut u64) as usize as u64;
        assert_eq!(
            unsafe { prodex_terminal_panel_label_width_v1(99, 4, 80, 10, 24, address) },
            4
        );
        assert_eq!(
            unsafe { prodex_terminal_panel_label_width_v1(1, 4, 80, 10, 24, 0) },
            1
        );
        assert_eq!(
            unsafe { prodex_terminal_session_columns_v1(1, 80, 8, address, 4) },
            1
        );
        assert_eq!(
            unsafe { prodex_terminal_panel_inline_height_v1(1, 40, 0) },
            1
        );
        assert_eq!(result, 9876);
    }
}
