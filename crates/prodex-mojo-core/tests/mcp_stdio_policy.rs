#![cfg(all(feature = "mojo-runtime", prodex_mojo_required))]
#![allow(unsafe_code)]

use prodex_mojo_core::mcp_stdio_policy::first_line_is_header;

unsafe extern "C" {
    fn prodex_mcp_first_line_is_header_v1(
        abi_version: i64,
        first_byte: i64,
        output_address: u64,
    ) -> i64;
}

#[test]
fn compiled_mojo_first_line_classifier_matches_all_byte_inputs() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };

    for value in 0_u16..=u16::from(u8::MAX) {
        let byte = value as u8;
        assert_eq!(
            first_line_is_header(byte).unwrap(),
            !matches!(byte, b'{' | b'['),
            "first byte {byte}"
        );
    }
}

#[test]
fn compiled_mojo_first_line_abi_rejects_invalid_version_input_and_output() {
    let mut output = -7_i64;
    let output_address = (&mut output as *mut i64) as usize as u64;

    assert_eq!(
        unsafe { prodex_mcp_first_line_is_header_v1(2, b'{'.into(), output_address) },
        4
    );
    assert_eq!(output, -7);
    assert_eq!(
        unsafe { prodex_mcp_first_line_is_header_v1(1, -1, output_address) },
        1
    );
    assert_eq!(output, -7);
    assert_eq!(
        unsafe { prodex_mcp_first_line_is_header_v1(1, 256, output_address) },
        1
    );
    assert_eq!(output, -7);
    assert_eq!(
        unsafe { prodex_mcp_first_line_is_header_v1(1, b'{'.into(), 0) },
        1
    );
    assert_eq!(output, -7);
}
