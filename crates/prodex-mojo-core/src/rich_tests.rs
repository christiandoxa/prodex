use super::*;
#[test]
fn rich_domain_self_test_covers_structured_results() {
    assert!(rich_self_test());
}

#[test]
fn rich_abi_rejects_null_views_and_reports_utf8_offsets() {
    ensure_rich_abi().expect("rich ABI layout should match");
    let mut result = RichContextResult::default();
    let mut records = [RichContextRecord::default()];
    let mut slots = [-1_i64];
    let invalid = [0xff_u8];
    let status = unsafe {
        prodex_mojo_rich_context_analyze_v2(
            RICH_ABI_VERSION,
            RichStringView {
                ptr: invalid.as_ptr(),
                len: invalid.len(),
            },
            mojo_pointer_address(records.as_mut_ptr()),
            records.len() as i64,
            0,
            0,
            mojo_pointer_address(slots.as_mut_ptr()),
            slots.len() as i64,
            mojo_mut_pointer_address(&mut result),
        )
    };
    assert_eq!(status, RICH_STATUS_UTF8);
    assert_eq!(result.issue_kind, 6);
    assert_eq!(result.issue_offset, 0);

    let status = unsafe {
        prodex_mojo_rich_context_analyze_v2(
            RICH_ABI_VERSION,
            RichStringView::default(),
            0,
            0,
            0,
            0,
            mojo_pointer_address(slots.as_mut_ptr()),
            slots.len() as i64,
            0,
        )
    };
    assert_eq!(status, RICH_STATUS_INVALID);
}

#[test]
fn rich_abi_malformed_utf8_and_capacity_are_bounded() {
    ensure_rich_abi().expect("rich ABI layout should match");
    let malformed = [
        vec![0xff],
        vec![0xc0, 0x80],
        vec![0xe0, 0x80, 0x80],
        vec![0xed, 0xa0, 0x80],
        vec![0xf0, 0x80, 0x80, 0x80],
        vec![0xf4, 0x90, 0x80, 0x80],
        vec![0xf0, 0x90, 0x80],
    ];
    for case in 0..20_000 {
        let bytes = &malformed[case % malformed.len()];
        let mut result = RichContextResult::default();
        let mut records = [RichContextRecord::default()];
        let mut output = [0_u8; 8];
        let mut slots = [-1_i64; 2];
        let status = unsafe {
            prodex_mojo_rich_context_analyze_v2(
                RICH_ABI_VERSION,
                RichStringView {
                    ptr: bytes.as_ptr(),
                    len: bytes.len(),
                },
                mojo_pointer_address(records.as_mut_ptr()),
                records.len() as i64,
                mojo_pointer_address(output.as_mut_ptr()),
                output.len() as i64,
                mojo_pointer_address(slots.as_mut_ptr()),
                slots.len() as i64,
                mojo_mut_pointer_address(&mut result),
            )
        };
        assert_eq!(status, RICH_STATUS_UTF8, "malformed case {case}");
        assert_eq!(result.issue_kind, 6, "malformed case {case}");
        assert!(result.issue_offset >= 0, "malformed case {case}");
    }

    let input = b"error: bounded";
    let mut result = RichContextResult::default();
    let status = unsafe {
        prodex_mojo_rich_context_analyze_v2(
            RICH_ABI_VERSION,
            RichStringView {
                ptr: input.as_ptr(),
                len: input.len(),
            },
            0,
            0,
            0,
            0,
            0,
            0,
            mojo_mut_pointer_address(&mut result),
        )
    };
    assert_eq!(status, RICH_STATUS_CAPACITY);
    assert_eq!(result.required_records, 1);
    assert!(result.required_output >= input.len() as i64);
    assert!(result.required_scratch >= 1);
}
