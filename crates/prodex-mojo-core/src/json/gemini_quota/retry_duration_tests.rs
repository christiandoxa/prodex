use super::*;

#[test]
fn compiled_retry_text_preserves_float_grammar_saturation_and_integer_headers() {
    use GeminiRetryTextKind::*;
    for (kind, text, expected) in [
        (Duration, "\u{2003}+1.5s\u{3000}", Some(1500)),
        (Duration, "9e1s", Some(90_000)),
        (Duration, "1e-9ms", Some(1)),
        (Duration, "5e-324ms", Some(1)),
        (Duration, "1e-999s", None),
        (Duration, "1e308s", Some(u64::MAX)),
        (Duration, "18446744073709551616ms", Some(u64::MAX)),
        (Duration, "0s", None),
        (Duration, "NaNs", None),
        (Duration, "infs", None),
        (Duration, "-1s", None),
        (Duration, "1MS", None),
        (Duration, "1 s", Some(1000)),
        (HeaderSeconds, "\u{2003}+002\u{3000}", Some(2000)),
        (HeaderSeconds, "18446744073709551615", Some(u64::MAX)),
        (HeaderSeconds, "18446744073709551616", None),
        (HeaderSeconds, "0", None),
        (HeaderSeconds, "-0", None),
        (HeaderSeconds, "1.5", None),
    ] {
        assert_eq!(
            gemini_retry_text_delay_ms(kind, text),
            Ok(expected),
            "{kind:?} {text:?}"
        );
    }
}

#[test]
fn compiled_retry_message_preserves_marker_priority_and_first_occurrence_rules() {
    use GeminiRetryTextKind::Message;
    for (text, expected) in [
        ("Ω PLEASE RETRY IN 1.5SECONDS", Some(1500)),
        ("suggested retry after 1s; please retry in 2s", Some(2000)),
        ("please retry in .s; suggested retry after 250MS", Some(250)),
        ("please retry in invalid; please retry in 2s", None),
        ("please retry in 1e3s", None),
        ("please retry in 1 s", None),
        ("please retry in 0s; suggested retry after 1s", Some(1000)),
        ("please retry in \u{3000}0.01ms", Some(1)),
        ("retry later", None),
    ] {
        assert_eq!(
            gemini_retry_text_delay_ms(Message, text),
            Ok(expected),
            "{text:?}"
        );
    }
    let large = "x".repeat(65_537) + " please retry in 1s";
    assert_eq!(gemini_retry_text_delay_ms(Message, &large), Ok(Some(1000)));
}

#[test]
fn retry_text_abis_reject_bad_shapes_before_writes() {
    let mut plan = [71_u64; 9];
    let address = plan.as_mut_ptr() as u64;
    unsafe {
        assert_eq!(prodex_gemini_retry_text_plan_v1(2, 0, 0, 0, address, 9), 4);
        assert_eq!(prodex_gemini_retry_text_plan_v1(1, 3, 0, 0, address, 9), 1);
        assert_eq!(prodex_gemini_retry_text_plan_v1(1, 0, 0, -1, address, 9), 1);
        assert_eq!(prodex_gemini_retry_text_plan_v1(1, 0, 0, 1, address, 9), 1);
        assert_eq!(prodex_gemini_retry_text_plan_v1(1, 0, 0, 0, address, 8), 3);
        assert_eq!(prodex_gemini_retry_text_plan_v1(1, 0, 0, 0, 1, 9), 1);
        let invalid = [0xff_u8];
        assert_eq!(
            prodex_gemini_retry_text_plan_v1(1, 0, invalid.as_ptr() as u64, 1, address, 9),
            1
        );
        assert_eq!(
            prodex_gemini_retry_text_finish_v1(2, 1, 1, 1.0, 0.0, 1, 0, address, 2),
            4
        );
        assert_eq!(
            prodex_gemini_retry_text_finish_v1(1, 0, 0, 1.0, 0.0, 1, 0, address, 2),
            1
        );
        assert_eq!(
            prodex_gemini_retry_text_finish_v1(1, 1, 2, 1.0, 0.0, 1, 0, address, 2),
            1
        );
        assert_eq!(
            prodex_gemini_retry_text_finish_v1(1, 2, 3, 1.0, 0.0, 1, 2, address, 2),
            1
        );
        assert_eq!(
            prodex_gemini_retry_text_finish_v1(1, 1, 1, 1.0, 0.0, 1, 0, address, 1),
            3
        );
    }
    assert_eq!(plan, [71; 9]);
}
