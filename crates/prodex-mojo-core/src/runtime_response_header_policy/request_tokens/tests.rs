use super::*;

fn original_tokens(values: &[&str]) -> Vec<String> {
    values
        .iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|token| {
            !token.is_empty()
                && token
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn compiled_request_tokens_preserve_full_grammar_trimming_and_order() {
    let bytes = (1_u8..=127)
        .map(|byte| char::from(byte).to_string())
        .collect::<Vec<_>>();
    let values = bytes.iter().map(String::as_str).collect::<Vec<_>>();
    assert_eq!(
        request_connection_tokens(&values),
        Ok(original_tokens(&values))
    );
    let values = [
        "",
        ",,",
        "Keep-Alive, X-Local-Hop, x-local-hop",
        "\u{0085}X-One\u{00a0},\u{1680}X-Two\u{2003},\u{2028}X-Three\u{202f}",
        "\u{205f}X-Four\u{3000},\u{200b}invalid\u{200b}",
        "bad name, bad:colon, bad/segment, \0, é, 🙂, X-Good",
    ];
    assert_eq!(
        request_connection_tokens(&values),
        Ok(original_tokens(&values))
    );
    assert_eq!(request_connection_tokens(&[]), Ok(vec![]));
    let large = "x, ".repeat(20_000);
    assert_eq!(
        request_connection_tokens(&[&large]),
        Ok(vec!["x".to_string(); 20_000])
    );
}

#[test]
fn request_token_abi_validates_entire_batch_before_output() {
    let text = "X-First, X-Second";
    let views = [StringView::from(text)];
    let malformed = [
        text.as_ptr() as u64,
        text.len() as u64,
        b"\xff".as_ptr() as u64,
        1,
    ];
    let mut output = [[-7_i64; 3]; 2];
    let mut written = -9_i64;
    for (version, address, count, phase, capacity, expected) in [
        (2, views.as_ptr() as u64, 1, 0, 2, 4),
        (1, 0, 1, 0, 2, 1),
        (1, views.as_ptr() as u64 + 1, 1, 0, 2, 1),
        (1, views.as_ptr() as u64, -1, 0, 2, 1),
        (1, views.as_ptr() as u64, 1, 2, 2, 1),
        (1, views.as_ptr() as u64, 1, 1, 2, 1),
        (1, views.as_ptr() as u64, 1, 0, 1, 3),
        (1, malformed.as_ptr() as u64, 2, 0, 2, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_runtime_request_connection_tokens_v1(
                    version,
                    address,
                    count,
                    phase,
                    output.as_mut_ptr() as u64,
                    capacity,
                    (&mut written as *mut i64) as u64,
                )
            },
            expected
        );
        assert_eq!(output, [[-7; 3]; 2]);
        assert_eq!(written, -9);
    }
}
