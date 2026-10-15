use super::*;

#[test]
fn mojo_retry_after_text_preserves_case_unicode_and_unit_precedence() {
    for (message, expected) in [
        ("Please TRY AGAIN IN 11.054s.", Some(11_054)),
        ("try again in　1.1ms", Some(2)),
        ("try again in\t0.9991second", Some(1000)),
        ("try again in 301S", Some(300_000)),
        ("try again in 0s", None),
        ("try again in 1 minute", None),
        ("try again in 1.2.3s", None),
        ("try again in NaNs", None),
        ("unrelated 42s", None),
        (
            "try again in 340282366920938463463374607431768211456ms",
            None,
        ),
    ] {
        assert_eq!(
            runtime_retry_after_text_millis(RUNTIME_RETRY_AFTER_TEXT_MESSAGE, message).unwrap(),
            expected,
            "{message:?}",
        );
    }
    for (header, expected) in [
        ("   +1  ", None),
        ("　301\t", Some(300_000)),
        (" 1 ", Some(1000)),
        ("0", None),
        ("1.1", None),
        ("18446744073709551616", None),
        ("", None),
    ] {
        assert_eq!(
            runtime_retry_after_text_millis(RUNTIME_RETRY_AFTER_TEXT_HEADER, header).unwrap(),
            expected,
            "{header:?}",
        );
    }
}

#[test]
fn mojo_retry_after_text_rejects_invalid_abi_and_mode() {
    let bytes = b"try again in 1s";
    unsafe {
        assert_eq!(
            prodex_mojo_rich_retry_after_text_v1(
                RICH_ABI_VERSION + 1,
                1,
                mojo_pointer_address(bytes.as_ptr()),
                bytes.len() as i64,
            ),
            -2,
        );
        assert_eq!(
            prodex_mojo_rich_retry_after_text_v1(
                RICH_ABI_VERSION,
                2,
                mojo_pointer_address(bytes.as_ptr()),
                bytes.len() as i64,
            ),
            -2,
        );
        assert_eq!(
            prodex_mojo_rich_retry_after_text_v1(RICH_ABI_VERSION, 1, 0, bytes.len() as i64,),
            -2,
        );
    }
}
