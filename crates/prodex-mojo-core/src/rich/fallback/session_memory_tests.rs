use super::{model_session_memory_allowed, prodex_mojo_rich_model_session_memory_allowed_v1};
use crate::rich::{
    RICH_ABI_VERSION, RICH_STATUS_UTF8, RichStringView, mojo_mut_pointer_address,
    mojo_pointer_address,
};

#[test]
fn session_memory_policy_accepts_only_empty_auto_or_default_models() {
    for (model, expected) in [
        ("", true),
        ("  auto ", true),
        ("\u{3000}DEFAULT\u{2003}", true),
        ("claude-sonnet-4-6", false),
        ("automatic", false),
    ] {
        assert_eq!(
            model_session_memory_allowed(model),
            Ok(expected),
            "{model:?}"
        );
    }
}

#[test]
fn session_memory_policy_rejects_malformed_utf8_at_the_abi() {
    let invalid = [0xff_u8];
    let model = RichStringView {
        ptr: mojo_pointer_address(invalid.as_ptr()),
        len: invalid.len() as u64,
    };
    let mut allowed = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_model_session_memory_allowed_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&model),
            mojo_mut_pointer_address(&mut allowed),
        )
    };

    assert_eq!(status, RICH_STATUS_UTF8);
    assert_eq!(allowed, -1);
}
