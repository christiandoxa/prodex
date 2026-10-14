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
    fn prodex_terminal_token_chunk_plan_v1(
        abi: i64,
        widths_address: u64,
        scalar_count: i64,
        maximum_cells: u64,
        boundaries_address: u64,
        boundaries_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_terminal_take_display_cells_v1(
        abi: i64,
        widths_address: u64,
        scalar_count: i64,
        maximum_cells: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_terminal_fit_cell_plan_v1(
        abi: i64,
        content_cells: u64,
        available_cells: u64,
        result_address: u64,
        result_capacity: i64,
    ) -> i64;
    fn prodex_terminal_wrap_piece_action_v1(
        abi: i64,
        current_empty: i64,
        current_cells: u64,
        piece_cells: u64,
        total_cells: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_terminal_pad_cell_spaces_v1(
        abi: i64,
        visible_cells: u64,
        total_cells: u64,
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

/// The host materializes strings only; the width and truncation decisions
/// come from a compiled Mojo ABI, without a duplicated Rust fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitCellPlan {
    Empty,
    Keep,
    Dots(usize),
    Truncate(usize),
}

pub fn fit_cell_plan(
    content_cells: usize,
    available_cells: usize,
) -> Result<FitCellPlan, MojoError> {
    let mut result = [u64::MAX; 2];
    check_status(unsafe {
        prodex_terminal_fit_cell_plan_v1(
            ABI_VERSION,
            from_usize(content_cells)?,
            from_usize(available_cells)?,
            result.as_mut_ptr() as usize as u64,
            2,
        )
    })?;
    match result {
        [0, 0] => Ok(FitCellPlan::Empty),
        [1, 0] => Ok(FitCellPlan::Keep),
        [2, n] if n > 0 && n <= 3 => Ok(FitCellPlan::Dots(
            usize::try_from(n).map_err(|_| MojoError::InvalidOutput)?,
        )),
        [3, n] if n > 0 => Ok(FitCellPlan::Truncate(
            usize::try_from(n).map_err(|_| MojoError::InvalidOutput)?,
        )),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Mojo returns scalar indexes at which each visible Unicode token ends.
/// The caller measures Unicode cell widths, never decides chunk boundaries.
pub fn token_chunk_boundaries(
    scalar_cells: &[u64],
    max_cells: usize,
) -> Result<Vec<usize>, MojoError> {
    let capacity = scalar_cells
        .len()
        .checked_add(1)
        .ok_or(MojoError::InvalidInput)?;
    let count = i64::try_from(scalar_cells.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut endings = vec![u64::MAX; capacity];
    let mut written = -1i64;
    check_status(unsafe {
        prodex_terminal_token_chunk_plan_v1(
            ABI_VERSION,
            scalar_cells.as_ptr() as usize as u64,
            count,
            from_usize(max_cells)?,
            endings.as_mut_ptr() as usize as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written == 0 || written > capacity {
        return Err(MojoError::InvalidOutput);
    }
    let endings = endings[..written]
        .iter()
        .map(|end| usize::try_from(*end).map_err(|_| MojoError::InvalidOutput))
        .collect::<Result<Vec<_>, _>>()?;
    if scalar_cells.is_empty() || max_cells == 0 {
        return if endings == [0] {
            Ok(endings)
        } else {
            Err(MojoError::InvalidOutput)
        };
    }
    if endings.last() != Some(&scalar_cells.len())
        || endings
            .iter()
            .any(|end| *end == 0 || *end > scalar_cells.len())
        || endings.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(endings)
}

/// Plan the number of Unicode scalars fitting in an already measured cell
/// budget. Zero-width grapheme marks follow the original scalar semantics.
pub fn display_prefix_scalars(scalar_cells: &[u64], max_cells: usize) -> Result<usize, MojoError> {
    let mut prefix = u64::MAX;
    check_status(unsafe {
        prodex_terminal_take_display_cells_v1(
            ABI_VERSION,
            scalar_cells.as_ptr() as usize as u64,
            i64::try_from(scalar_cells.len()).map_err(|_| MojoError::InvalidInput)?,
            from_usize(max_cells)?,
            (&mut prefix as *mut u64) as usize as u64,
        )
    })?;
    let prefix = usize::try_from(prefix).map_err(|_| MojoError::InvalidOutput)?;
    if prefix > scalar_cells.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(prefix)
}

/// Mojo-authoritative word wrapping action; Rust only materializes strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapPieceAction {
    Start,
    Join,
    Flush,
}

pub fn wrap_piece_action(
    current_empty: bool,
    current_cells: usize,
    piece_cells: usize,
    total_cells: usize,
) -> Result<WrapPieceAction, MojoError> {
    let mut action = u64::MAX;
    check_status(unsafe {
        prodex_terminal_wrap_piece_action_v1(
            ABI_VERSION,
            i64::from(current_empty),
            from_usize(current_cells)?,
            from_usize(piece_cells)?,
            from_usize(total_cells)?,
            (&mut action as *mut u64) as usize as u64,
        )
    })?;
    match action {
        0 => Ok(WrapPieceAction::Start),
        1 => Ok(WrapPieceAction::Join),
        2 => Ok(WrapPieceAction::Flush),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Mojo-authoritative count of spaces for the host terminal cell.
pub fn pad_cell_spaces(visible_cells: usize, total_cells: usize) -> Result<usize, MojoError> {
    let mut spaces = u64::MAX;
    check_status(unsafe {
        prodex_terminal_pad_cell_spaces_v1(
            ABI_VERSION,
            from_usize(visible_cells)?,
            from_usize(total_cells)?,
            (&mut spaces as *mut u64) as usize as u64,
        )
    })?;
    let spaces = usize::try_from(spaces).map_err(|_| MojoError::InvalidOutput)?;
    if spaces > total_cells {
        return Err(MojoError::InvalidOutput);
    }
    Ok(spaces)
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
    fn token_chunk_boundaries_preserve_oversized_and_zero_cell_segments() {
        for (cells, maximum, expected) in [
            (vec![], 2, vec![0]),
            (vec![1, 1], 0, vec![0]),
            (vec![1, 1, 1, 1], 2, vec![2, 4]),
            (vec![2, 1], 3, vec![2]),
            (vec![2, 1, 0, 2], 1, vec![1, 2, 4]),
            (vec![0, 0, 1], 1, vec![3]),
            (vec![0, 0], 1, vec![2]),
            (vec![0, 0, 2, 1], 1, vec![3, 4]),
            (vec![2], 1, vec![1]),
            (vec![1, 0, 0, 1], 2, vec![4]),
        ] {
            assert_eq!(
                token_chunk_boundaries(&cells, maximum).unwrap(),
                expected,
                "Mojo token chunking for widths {cells:?} / {maximum}",
            );
        }
    }

    #[test]
    fn terminal_prefix_width_preserves_zero_width_unicode_scalars() {
        for (widths, available, expected) in [
            (vec![], 0, 0),
            (vec![0, 0, 2, 1], 0, 2),
            (vec![0, 0, 2, 1], 1, 2),
            (vec![0, 0, 2, 1], 2, 3),
            (vec![0, 0, 2, 1], 3, 4),
            (vec![2, 2, 2], 4, 2),
            (vec![1, 1, 1], usize::MAX, 3),
        ] {
            assert_eq!(
                display_prefix_scalars(&widths, available).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn terminal_fit_plan_retains_content_and_ellipsis_modes() {
        for (content, available, expected) in [
            (3, 0, FitCellPlan::Empty),
            (0, 1, FitCellPlan::Keep),
            (7, 7, FitCellPlan::Keep),
            (7, 8, FitCellPlan::Keep),
            (7, 3, FitCellPlan::Dots(3)),
            (7, 1, FitCellPlan::Dots(1)),
            (7, 4, FitCellPlan::Truncate(1)),
            (7, 6, FitCellPlan::Truncate(3)),
        ] {
            assert_eq!(fit_cell_plan(content, available).unwrap(), expected);
        }
    }

    #[test]
    fn strict_chunk_and_truncate_abi_reject_invalid_input_and_capacity() {
        let widths = [1u64, 3];
        let mut boundaries = [u64::MAX; 3];
        let mut written = -1i64;
        let input = widths.as_ptr() as usize as u64;
        let output = boundaries.as_mut_ptr() as usize as u64;
        let count_ptr = (&mut written as *mut i64) as usize as u64;
        assert_eq!(
            unsafe { prodex_terminal_token_chunk_plan_v1(4, input, 2, 1, output, 3, count_ptr) },
            4,
        );
        assert_eq!(
            unsafe { prodex_terminal_token_chunk_plan_v1(1, input, 2, 1, output, 2, count_ptr) },
            1,
        );
        assert_eq!(
            unsafe { prodex_terminal_token_chunk_plan_v1(1, 0, 2, 1, output, 3, count_ptr) },
            1,
        );
        assert_eq!(
            unsafe { prodex_terminal_token_chunk_plan_v1(1, input, 2, 1, output, 3, count_ptr) },
            1,
            "reject cell widths outside Unicode's domain",
        );
        assert_eq!(
            unsafe { prodex_terminal_take_display_cells_v1(1, input, 2, 2, 0) },
            1,
        );
        assert_eq!(
            unsafe { prodex_terminal_fit_cell_plan_v1(1, 4, 5, output, 1) },
            1,
        );
        assert_eq!(written, -1, "invalid ABI must not publish a chunk count");
    }

    #[test]
    fn mojo_wrap_piece_action_keeps_exact_boundary_and_zero_width_semantics() {
        for (empty, current, piece, total, expected) in [
            (true, 0, 2, 3, WrapPieceAction::Start),
            (true, 100, 100, 1, WrapPieceAction::Start),
            (false, 4, 2, 7, WrapPieceAction::Join),
            (false, 4, 3, 7, WrapPieceAction::Flush),
            (false, 0, 0, 1, WrapPieceAction::Join),
            (false, 0, 2, 1, WrapPieceAction::Flush),
            (false, 1, 1, 1, WrapPieceAction::Flush),
            (false, usize::MAX, 2, usize::MAX, WrapPieceAction::Flush),
        ] {
            assert_eq!(
                wrap_piece_action(empty, current, piece, total).unwrap(),
                expected,
                "Mojo wrap policy for empty={empty} current={current} piece={piece} total={total}",
            );
        }
    }

    #[test]
    fn mojo_padding_saturates_remaining_terminal_cells() {
        for (visible, total, expected) in [
            (0, 0, 0),
            (0, 10, 10),
            (10, 0, 0),
            (10, 10, 0),
            (10, 15, 5),
            (usize::MAX, 0, 0),
            (0, usize::MAX, usize::MAX),
        ] {
            assert_eq!(pad_cell_spaces(visible, total).unwrap(), expected);
        }
    }

    #[test]
    fn wrap_and_pad_abi_reject_null_output_and_invalid_selector() {
        let mut output = u64::MAX;
        let pointer = (&mut output as *mut u64) as usize as u64;
        assert_eq!(
            unsafe { prodex_terminal_wrap_piece_action_v1(99, 1, 0, 0, 4, pointer) },
            4,
        );
        assert_eq!(
            unsafe { prodex_terminal_wrap_piece_action_v1(1, 2, 0, 0, 4, pointer) },
            1,
        );
        assert_eq!(
            unsafe { prodex_terminal_wrap_piece_action_v1(1, 1, 0, 0, 4, 0) },
            1,
        );
        assert_eq!(
            unsafe { prodex_terminal_pad_cell_spaces_v1(1, 0, 10, 0) },
            1,
        );
        assert_eq!(output, u64::MAX);
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
