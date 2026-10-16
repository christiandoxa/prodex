use super::*;

#[test]
fn mojo_rate_limit_header_nonempty_matches_unicode_whitespace_contract() {
    for (value, expected) in [
        ("", false),
        (" \t\n", false),
        ("\u{2003}\u{3000}\u{a0}", false),
        ("rate_limit_reached", true),
        (" \u{2003}workspace_member_credits_depleted\u{3000}", true),
        ("-", true),
        ("🧭", true),
    ] {
        assert_eq!(
            rate_limit_header_nonempty(value).unwrap(),
            expected,
            "{value:?}"
        );
    }
    assert_eq!(
        rate_limit_header_class(" rate_limit_reached "),
        Ok(RATE_LIMIT_HEADER_CLASS_RATE)
    );
    assert_eq!(
        rate_limit_header_class("\u{2003}workspace_member_credits_depleted\u{3000}"),
        Ok(RATE_LIMIT_HEADER_CLASS_QUOTA)
    );
}

#[test]
fn mojo_rate_limit_header_nonempty_strict_abi_rejects_malformed_shapes() {
    let valid = b"quota";
    let address = valid.as_ptr() as usize as u64;
    let invalid = [0xff_u8];
    unsafe {
        assert_eq!(prodex_mojo_rate_limit_header_nonempty_v1(2, address, 5), -4);
        assert_eq!(prodex_mojo_rate_limit_header_nonempty_v1(1, 0, 5), -1);
        assert_eq!(
            prodex_mojo_rate_limit_header_nonempty_v1(1, address, -1),
            -1
        );
        assert_eq!(
            prodex_mojo_rate_limit_header_nonempty_v1(1, invalid.as_ptr() as usize as u64, 1),
            -1
        );
        assert_eq!(prodex_mojo_rate_limit_header_nonempty_v1(1, address, 5), 1);
        assert_eq!(prodex_mojo_rate_limit_header_nonempty_v1(1, 0, 0), 0);
    }
}
