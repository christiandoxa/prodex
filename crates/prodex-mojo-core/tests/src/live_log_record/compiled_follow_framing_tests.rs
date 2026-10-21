use super::*;

#[test]
fn compiled_follow_last_newline_preserves_binary_and_incomplete_tail() {
    for (bytes, expected) in [
        (b"".as_slice(), 0),
        (b"without newline", 0),
        (b"a\n".as_slice(), 2),
        (b"first\nsecond\n".as_slice(), 13),
        (b"a\r\nb\npartial".as_slice(), 5),
        (b"\xff\xfe\ntrailing".as_slice(), 3),
        (b"one\n\xf0\x9f\x8c".as_slice(), 4),
        (b"\r\n\n\n".as_slice(), 4),
    ] {
        assert_eq!(
            follow_last_complete_line_end(bytes),
            Ok(expected),
            "{bytes:?}"
        );
    }
}

#[test]
fn compiled_follow_utf8_line_lengths_match_exact_split_inclusive() {
    for text in [
        "",
        "one",
        "one\n",
        "\n\n\n",
        "a\r\nb\r\n",
        "雪\n🦀\n",
        "first\nsecond",
        "�\ninvalid replacement\n",
        "a\n\nlast",
    ] {
        let original = text.split_inclusive('\n').map(str::len).collect::<Vec<_>>();
        assert_eq!(follow_utf8_line_lengths(text), Ok(original), "{text:?}");
    }
    let many = "x\n".repeat(1025);
    assert_eq!(follow_utf8_line_lengths(&many), Ok(vec![2; 1025]),);
    let binary = [255_u8, 254, b'\n'];
    let mut output = [-7_i64; 2];
    let mut written = -7_i64;
    assert_eq!(
        unsafe {
            prodex_live_log_follow_line_lengths_v1(
                1,
                binary.as_ptr() as usize as u64,
                binary.len() as i64,
                output.as_mut_ptr() as usize as u64,
                2,
                0,
                (&mut written as *mut i64) as usize as u64,
            )
        },
        1,
    );
    assert_eq!(output, [-7; 2]);
    assert_eq!(written, -7);
}

#[test]
fn compiled_follow_framing_abi_rejects_invalid_and_short_capacity_without_writes() {
    let bytes = b"a\nb\n";
    let mut output = [71_i64; 3];
    let mut count = 83_i64;
    let source = bytes.as_ptr() as usize as u64;
    let out = output.as_mut_ptr() as usize as u64;
    let result = (&mut count as *mut i64) as usize as u64;
    for (version, address, length, capacity, measuring, dest, expected) in [
        (2, source, 4, 3, 0, result, 4),
        (1, 0, 4, 3, 0, result, 1),
        (1, source, -1, 3, 0, result, 1),
        (1, source, 4, 1, 0, result, 2),
        (1, source, 4, 3, 2, result, 1),
        (1, source, 4, 3, 0, 0, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_live_log_follow_line_lengths_v1(
                    version, address, length, out, capacity, measuring, dest,
                )
            },
            expected,
        );
        assert_eq!(output, [71; 3]);
        assert_eq!(count, 83);
    }
    for (version, address, length, dest, expected) in [
        (2, source, 4, result, 4),
        (1, 0, 4, result, 1),
        (1, source, -1, result, 1),
        (1, source, 4, 0, 1),
    ] {
        assert_eq!(
            unsafe { prodex_live_log_follow_last_newline_v1(version, address, length, dest) },
            expected,
        );
        assert_eq!(count, 83);
    }
}
