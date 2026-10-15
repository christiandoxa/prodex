#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::smart_context_model_registry::{
    extract_model_name_from_json_prefix, normalize_model_name,
};

unsafe extern "C" {
    fn prodex_smart_context_model_name_normalize_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_smart_context_model_prefix_scan_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

#[test]
fn model_prefix_scan_preserves_bounded_prefix_compatibility() {
    let cases = [
        ("{\"model\" :　\"gpt-5.5-mini\"}", Some("gpt-5.5-mini")),
        (r#"{"model":"gpt\"5"}"#, Some("gpt\"5")),
        (r#"{"message":"model","model":"ignored"}"#, None),
        ("{\"model\":\"bad\u{0007}model\"}", None),
        (r#"{"model":"unterminated}"#, None),
    ];

    for (input, expected) in cases {
        assert_eq!(
            extract_model_name_from_json_prefix(input)
                .unwrap()
                .as_deref(),
            expected,
            "{input:?}"
        );
    }
}

#[test]
fn model_prefix_scan_abi_rejects_invalid_inputs_and_capacity() {
    let mut output = [0xA5_u8; 8];
    let mut written = 99_i64;
    let call = |version, input: &[u8], capacity, output: &mut [u8], written: &mut i64| {
        // SAFETY: every pointer and length refers to a live test buffer for this call.
        unsafe {
            prodex_smart_context_model_prefix_scan_v1(
                version,
                input.as_ptr() as usize as u64,
                input.len() as i64,
                output.as_mut_ptr() as usize as u64,
                capacity,
                written as *mut i64 as usize as u64,
            )
        }
    };

    assert_eq!(
        call(2, br#"{"model":"gpt-5"}"#, 8, &mut output, &mut written),
        -4
    );
    assert_eq!(call(1, &[0xff], 8, &mut output, &mut written), -2);
    assert_eq!(
        call(1, br#"{"model":"gpt-5.5"}"#, 1, &mut output, &mut written),
        3
    );
    assert_eq!(written, 99);
}

#[test]
fn model_name_normalization_matches_golden_policy_cases() {
    let cases = [
        ("\u{2003}gpt-5.5\u{3000}", Some("gpt-5.5".to_string())),
        (
            "\u{00a0}gemini-3-pro\u{202f}",
            Some("gemini-3-pro".to_string()),
        ),
        ("", None),
        (" \t ", None),
        ("bad\u{0007}model", None),
        ("bad\u{007f}model", None),
        ("雪-model", Some("雪-model".to_string())),
        (&"x".repeat(128), Some("x".repeat(128))),
        (&"x".repeat(129), None),
    ];

    for (input, expected) in cases {
        assert_eq!(normalize_model_name(input).unwrap(), expected, "{input:?}");
    }
}

#[test]
fn model_name_normalization_rejects_oversized_input_before_abi_copy() {
    assert_eq!(
        normalize_model_name(&" ".repeat(4 * 1024 + 1)).unwrap(),
        None
    );
}

#[test]
fn model_name_normalization_abi_rejects_bad_utf8_version_and_capacity() {
    let invalid = [0xff_u8];
    let mut output = [0xA5_u8; 8];
    let mut written = 99_i64;
    let call = |version, input: &[u8], capacity, output: &mut [u8], written: &mut i64| {
        // SAFETY: every pointer and length refers to a live test buffer for this call.
        unsafe {
            prodex_smart_context_model_name_normalize_v1(
                version,
                input.as_ptr() as usize as u64,
                input.len() as i64,
                output.as_mut_ptr() as usize as u64,
                capacity,
                written as *mut i64 as usize as u64,
            )
        }
    };

    assert_eq!(call(1, &invalid, 8, &mut output, &mut written), -2);
    assert_eq!(call(2, b"gpt-5", 8, &mut output, &mut written), -4);
    assert_eq!(call(1, b"gpt-5", 1, &mut output, &mut written), 3);
    assert_eq!(output, [0xA5; 8]);
    assert_eq!(written, 99);
}
