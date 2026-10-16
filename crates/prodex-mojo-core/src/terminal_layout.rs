//! Typed, fail-closed adapters for Mojo's authoritative terminal layout.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_terminal_positive_size_override_v1(
        abi: i64,
        text_address: u64,
        text_length: i64,
        maximum: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_terminal_tty_dimensions_v1(
        abi: i64,
        text_address: u64,
        text_length: i64,
        maximum: u64,
        result_address: u64,
        result_capacity: i64,
    ) -> i64;
    fn prodex_terminal_panel_label_width_v1(
        abi: i64,
        longest: u64,
        total_width: u64,
        minimum: u64,
        maximum: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_terminal_field_row_layout_v1(
        abi: i64,
        total_width: u64,
        label_width: u64,
        label_cells: u64,
        result_address: u64,
        result_capacity: i64,
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
    fn prodex_terminal_session_report_viewport_v1(
        abi: i64,
        report_count: u64,
        terminal_height: u64,
        result_address: u64,
        result_capacity: i64,
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
    fn prodex_terminal_middle_ellipsis_plan_v1(
        abi: i64,
        widths_address: u64,
        scalar_count: i64,
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

/// Parse terminal override text using Mojo Unicode trimming, checked decimal
/// grammar and the host usize limit. None means missing/malformed/zero input.
pub fn terminal_positive_size_override(value: &str) -> Result<Option<usize>, MojoError> {
    let mut result = u64::MAX;
    check_status(unsafe {
        prodex_terminal_positive_size_override_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            usize::MAX as u64,
            (&mut result as *mut u64) as usize as u64,
        )
    })?;
    if result == 0 {
        Ok(None)
    } else {
        usize::try_from(result)
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput)
    }
}

/// Decode the first two stty dimensions in Mojo. Rust only executes stty and
/// converts UTF-8; extra trailing tokens retain the existing ignored behavior.
pub fn terminal_tty_dimensions(value: &str) -> Result<Option<(usize, usize)>, MojoError> {
    let mut result = [u64::MAX; 3];
    check_status(unsafe {
        prodex_terminal_tty_dimensions_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            usize::MAX as u64,
            result.as_mut_ptr() as usize as u64,
            3,
        )
    })?;
    match result {
        [0, 0, 0] => Ok(None),
        [1, rows, columns] => Ok(Some((
            usize::try_from(rows).map_err(|_| MojoError::InvalidOutput)?,
            usize::try_from(columns).map_err(|_| MojoError::InvalidOutput)?,
        ))),
        _ => Err(MojoError::InvalidOutput),
    }
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

/// The host slices UTF-8; Mojo chooses the middle-ellipsis presentation mode
/// and the scalar counts for its retained prefix and suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiddleEllipsisPlan {
    Empty,
    Keep,
    Dots(usize),
    Split {
        left_scalars: usize,
        right_scalars: usize,
    },
}

pub fn middle_ellipsis_plan(
    scalar_cells: &[u64],
    content_cells: usize,
    available_cells: usize,
) -> Result<MiddleEllipsisPlan, MojoError> {
    let scalar_count = u64::try_from(scalar_cells.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut result = [u64::MAX; 3];
    check_status(unsafe {
        prodex_terminal_middle_ellipsis_plan_v1(
            ABI_VERSION,
            scalar_cells.as_ptr() as usize as u64,
            i64::try_from(scalar_count).map_err(|_| MojoError::InvalidInput)?,
            from_usize(content_cells)?,
            from_usize(available_cells)?,
            result.as_mut_ptr() as usize as u64,
            3,
        )
    })?;
    match result {
        [0, 0, 0] => Ok(MiddleEllipsisPlan::Empty),
        [1, 0, 0] => Ok(MiddleEllipsisPlan::Keep),
        [2, count, 0] if count > 0 && count <= 3 => Ok(MiddleEllipsisPlan::Dots(
            usize::try_from(count).map_err(|_| MojoError::InvalidOutput)?,
        )),
        [3, left, right] if left <= scalar_count && right <= scalar_count => {
            Ok(MiddleEllipsisPlan::Split {
                left_scalars: usize::try_from(left).map_err(|_| MojoError::InvalidOutput)?,
                right_scalars: usize::try_from(right).map_err(|_| MojoError::InvalidOutput)?,
            })
        }
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

/// Plan the label padding and value width for one or more wrapped field rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldRowLayoutPlan {
    pub value_width: usize,
    pub first_padding: usize,
    pub continuation_padding: usize,
}

pub fn field_row_layout_plan(
    total_width: usize,
    label_width: usize,
    label_cells: usize,
) -> Result<FieldRowLayoutPlan, MojoError> {
    let mut result = [u64::MAX; 3];
    check_status(unsafe {
        prodex_terminal_field_row_layout_v1(
            ABI_VERSION,
            from_usize(total_width)?,
            from_usize(label_width)?,
            from_usize(label_cells)?,
            result.as_mut_ptr() as usize as u64,
            3,
        )
    })?;
    let value_width = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
    let first_padding = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
    let continuation_padding = usize::try_from(result[2]).map_err(|_| MojoError::InvalidOutput)?;
    if value_width == 0 || first_padding > label_width || continuation_padding != label_width {
        return Err(MojoError::InvalidOutput);
    }
    Ok(FieldRowLayoutPlan {
        value_width,
        first_padding,
        continuation_padding,
    })
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

/// Mojo plans the session-list scroll budget and bounded inline viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionReportViewportPlan {
    pub total_lines: usize,
    pub inline_height: u16,
}

pub fn session_report_viewport_plan(
    report_count: usize,
    terminal_height: u16,
) -> Result<SessionReportViewportPlan, MojoError> {
    let mut result = [u64::MAX; 2];
    check_status(unsafe {
        prodex_terminal_session_report_viewport_v1(
            ABI_VERSION,
            from_usize(report_count)?,
            u64::from(terminal_height),
            result.as_mut_ptr() as usize as u64,
            2,
        )
    })?;
    let total_lines = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
    let inline_height = u16::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
    if inline_height == 0 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(SessionReportViewportPlan {
        total_lines,
        inline_height,
    })
}

#[cfg(test)]
#[path = "terminal_layout/tests.rs"]
mod tests;
