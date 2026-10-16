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
fn field_row_layout_preserves_tiny_width_and_unicode_padding() {
    for (total, label, cells, expected) in [
        (
            24,
            10,
            5,
            FieldRowLayoutPlan {
                value_width: 13,
                first_padding: 5,
                continuation_padding: 10,
            },
        ),
        (
            10,
            10,
            11,
            FieldRowLayoutPlan {
                value_width: 1,
                first_padding: 0,
                continuation_padding: 10,
            },
        ),
        (
            0,
            0,
            0,
            FieldRowLayoutPlan {
                value_width: 1,
                first_padding: 0,
                continuation_padding: 0,
            },
        ),
    ] {
        assert_eq!(
            field_row_layout_plan(total, label, cells).unwrap(),
            expected
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
fn session_report_viewport_plan_preserves_scroll_and_height_bounds() {
    for (reports, terminal_height, expected_lines, expected_height) in [
        (0, 0, 0, 1),
        (0, 24, 0, 8),
        (1, 24, 4, 9),
        (2, 24, 8, 13),
        (100, 10, 400, 10),
    ] {
        let plan = session_report_viewport_plan(reports, terminal_height).unwrap();
        assert_eq!(plan.total_lines, expected_lines);
        assert_eq!(plan.inline_height, expected_height);
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn session_report_viewport_plan_saturates_at_host_maximum() {
    let plan = session_report_viewport_plan(usize::MAX, u16::MAX).unwrap();
    assert_eq!(plan.total_lines, usize::MAX);
    assert_eq!(plan.inline_height, u16::MAX);
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
fn middle_ellipsis_plan_preserves_unicode_cell_budget_modes() {
    for (cells, content, available, expected) in [
        (vec![], 0, 0, MiddleEllipsisPlan::Empty),
        (vec![1, 1, 1], 3, 8, MiddleEllipsisPlan::Keep),
        (vec![1, 1, 1, 1], 4, 1, MiddleEllipsisPlan::Dots(1)),
        (
            vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            12,
            8,
            MiddleEllipsisPlan::Split {
                left_scalars: 2,
                right_scalars: 3,
            },
        ),
        (
            vec![0, 0, 2, 2, 1],
            6,
            4,
            MiddleEllipsisPlan::Split {
                left_scalars: 2,
                right_scalars: 1,
            },
        ),
        (
            vec![2, 2, 2, 2],
            8,
            6,
            MiddleEllipsisPlan::Split {
                left_scalars: 0,
                right_scalars: 1,
            },
        ),
        (vec![1, 1, 1], 3, usize::MAX, MiddleEllipsisPlan::Keep),
    ] {
        assert_eq!(
            middle_ellipsis_plan(&cells, content, available).unwrap(),
            expected,
            "Mojo middle ellipsis for widths {cells:?} / {content} / {available}",
        );
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
        unsafe { prodex_terminal_field_row_layout_v1(1, 8, 4, 3, output, 2) },
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
    assert_eq!(
        unsafe { prodex_terminal_middle_ellipsis_plan_v1(1, input, 2, 4, 8, output, 2) },
        1,
    );
    assert_eq!(
        unsafe { prodex_terminal_middle_ellipsis_plan_v1(1, input, 2, 4, 8, output, 3) },
        1,
        "reject cell widths outside Unicode's domain",
    );
    assert_eq!(
        unsafe { prodex_terminal_middle_ellipsis_plan_v1(1, input, 2, 4, 8, 0, 3) },
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

#[test]
fn session_report_viewport_abi_rejects_version_pointer_and_capacity() {
    let mut result = [9876u64; 2];
    let address = result.as_mut_ptr() as usize as u64;
    assert_eq!(
        unsafe { prodex_terminal_session_report_viewport_v1(99, 2, 24, address, 2) },
        4
    );
    assert_eq!(
        unsafe { prodex_terminal_session_report_viewport_v1(1, 2, 24, 0, 2) },
        1
    );
    assert_eq!(
        unsafe { prodex_terminal_session_report_viewport_v1(1, 2, 24, address, 1) },
        1
    );
    assert_eq!(result, [9876, 9876]);

    assert_eq!(
        unsafe {
            prodex_terminal_session_report_viewport_v1(1, u64::MAX, u64::from(u16::MAX), address, 2)
        },
        0
    );
    assert_eq!(result, [u64::MAX, u64::from(u16::MAX)]);
}
