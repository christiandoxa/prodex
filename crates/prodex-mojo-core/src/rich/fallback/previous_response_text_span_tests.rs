use super::*;

#[test]
fn compiled_previous_response_text_span_selects_trimmed_original_unicode() {
    for (text, expected) in [
        ("", None),
        (" \t\u{3000}\n", None),
        (
            " \u{2003}previous_response_not_found \u{3000}\t",
            Some("previous_response_not_found"),
        ),
        (
            "previous_response_not_found",
            Some("previous_response_not_found"),
        ),
        (
            " Previous response with id 'resp-雪' not found. ",
            Some("Previous response with id 'resp-雪' not found."),
        ),
        (
            "unrelated previous_response_not_found in documentation",
            None,
        ),
        ("ordinary output", None),
        (" \u{2003}unrelated content\t ", None),
    ] {
        assert_eq!(
            previous_response_text_message_span(text),
            Ok(expected),
            "{text:?}",
        );
    }
}

#[test]
fn compiled_previous_response_text_span_abi_rejects_invalid_shape_before_output_write() {
    let text = "previous_response_not_found";
    let mut output = [91_i64; 3];
    let ptr = text.as_ptr() as usize as u64;
    let destination = output.as_mut_ptr() as usize as u64;
    for (version, address, length, out, expected) in [
        (5, ptr, text.len() as i64, destination, 4),
        (6, 0, text.len() as i64, destination, 1),
        (6, ptr, -1, destination, 1),
        (6, ptr, text.len() as i64, 0, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_mojo_previous_response_text_message_span_v1(version, address, length, out)
            },
            expected
        );
        assert_eq!(output, [91; 3]);
    }
    let invalid = [255_u8, 254_u8];
    assert_eq!(
        unsafe {
            prodex_mojo_previous_response_text_message_span_v1(
                6,
                invalid.as_ptr() as usize as u64,
                2,
                destination,
            )
        },
        1,
    );
    assert_eq!(output, [91; 3]);
}
