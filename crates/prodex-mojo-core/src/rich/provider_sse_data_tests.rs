use super::*;

#[test]
fn buffered_provider_sse_preserves_unicode_whitespace_empty_payloads_and_wire_boundaries() {
    let input = "event: error\r\n\u{2003}data:\u{3000}{\"code\":1} \r\n data: [DONE]\ndata:\nDATA: ignored\ndata\ndata: a\0東京\ndata: x\rdata:y\ndata: \u{200b}\n";
    assert_eq!(
        provider_sse_data_lines(input),
        Ok(vec![
            "{\"code\":1}",
            "[DONE]",
            "",
            "a\0東京",
            "x\rdata:y",
            "\u{200b}"
        ])
    );
    for input in [
        "",
        "\r\n\n",
        ":comment\nevent: ping",
        "\u{feff}data: ignored",
    ] {
        assert_eq!(provider_sse_data_lines(input), Ok(vec![]));
    }
    let long = format!("data: {}", "x".repeat(65_537));
    assert_eq!(provider_sse_data_lines(&long), Ok(vec![&long[6..]]));
}

#[test]
fn buffered_provider_sse_abi_validates_before_writes_and_measures_without_partial_rows() {
    let text = b"data:a\ndata:b\ndata:";
    let mut rows = [[71_u64; 2]; 3];
    let mut required = 71_u64;
    let destination = rows.as_mut_ptr() as u64;
    let metadata = &mut required as *mut u64 as u64;
    let call = |version, address, length, output, capacity, metadata| unsafe {
        prodex_sse_buffered_data_spans_v1(version, address, length, output, capacity, metadata)
    };
    assert_eq!(call(2, 0, 0, destination, 3, metadata), 4);
    assert_eq!(call(1, 0, 1, destination, 3, metadata), 1);
    assert_eq!(call(1, 0, -1, destination, 3, metadata), 1);
    assert_eq!(call(1, 0, 0, 1, 1, metadata), 1);
    assert_eq!(call(1, 0, 0, 0, 0, 1), 1);
    assert_eq!(call(1, 0, 0, destination, -1, metadata), 1);
    let invalid = [0xff_u8];
    assert_eq!(
        call(1, invalid.as_ptr() as u64, 1, destination, 3, metadata),
        1
    );
    assert_eq!(required, 71);
    assert_eq!(rows, [[71; 2]; 3]);
    assert_eq!(
        call(
            1,
            text.as_ptr() as u64,
            text.len() as i64,
            destination,
            2,
            metadata
        ),
        3
    );
    assert_eq!(required, 3);
    assert_eq!(rows, [[71; 2]; 3]);
    assert_eq!(
        call(
            1,
            text.as_ptr() as u64,
            text.len() as i64,
            destination,
            3,
            metadata
        ),
        0
    );
    assert_eq!(required, 3);
    assert_eq!(rows, [[5, 6], [12, 13], [19, 19]]);
}
