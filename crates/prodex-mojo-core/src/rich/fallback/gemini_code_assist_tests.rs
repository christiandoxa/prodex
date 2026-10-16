use super::{
    gemini_code_assist_model_allowed, prodex_mojo_rich_gemini_code_assist_model_allowed_v1,
};
use crate::rich::{
    RICH_ABI_VERSION, RICH_STATUS_UTF8, RichStringView, mojo_mut_pointer_address,
    mojo_pointer_address,
};

#[test]
fn gemini_code_assist_filter_preserves_trim_case_and_unbounded_input_behavior() {
    for (model, expected) in [
        ("gemini-3.1-pro-preview-customtools", false),
        ("\u{3000}gemini-3.5-flash\u{2003}", false),
        ("gemini-3-flash", false),
        ("customTools", true),
        (" GEMINI-3.5-FLASH ", true),
        ("", true),
        ("\u{3000}", true),
    ] {
        assert_eq!(
            gemini_code_assist_model_allowed(model),
            Ok(expected),
            "{model:?}"
        );
    }
    assert!(gemini_code_assist_model_allowed(&"x".repeat(4_097)).unwrap());
}

#[test]
fn gemini_code_assist_filter_rejects_malformed_utf8_at_the_abi() {
    let invalid = [0xff_u8];
    let model = RichStringView {
        ptr: mojo_pointer_address(invalid.as_ptr()),
        len: invalid.len() as u64,
    };
    let mut allowed = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_gemini_code_assist_model_allowed_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&model),
            mojo_mut_pointer_address(&mut allowed),
        )
    };

    assert_eq!(status, RICH_STATUS_UTF8);
    assert_eq!(allowed, -1);
}
