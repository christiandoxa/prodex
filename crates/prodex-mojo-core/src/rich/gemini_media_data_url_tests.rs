use super::*;

#[test]
fn compiled_gemini_data_url_preserves_raw_prefix_segments_and_borrowed_spans() {
    for (input, expected) in [
        ("", None),
        ("data:", None),
        ("DATA:image/png;base64,X", None),
        (" data:image/png;base64,X", None),
        ("data:image/png; base64,X", None),
        ("data:image/png,X", None),
        ("data:image/png;base64", None),
        ("data:base64,X", Some(("base64", "X"))),
        ("data:;BASE64,", Some(("application/octet-stream", ""))),
        (
            "data: \u{2003};BaSe64,X ",
            Some(("application/octet-stream", "X ")),
        ),
        ("data: image/png ;base64,X", Some((" image/png ", "X"))),
        (
            "data:画像/png;charset=utf-8;base64;ignored,a,b\0🧭",
            Some(("画像/png", "a,b\0🧭")),
        ),
    ] {
        assert_eq!(
            gemini_media_data_url_parts(input),
            Ok(expected),
            "{input:?}"
        );
        if let Some((_, data)) = expected {
            let actual = gemini_media_data_url_parts(input).unwrap().unwrap();
            assert_eq!(
                actual.1.as_ptr(),
                input[input.len() - data.len()..].as_ptr()
            );
        }
    }
}

#[test]
fn compiled_gemini_data_url_keeps_large_payload_and_segment_counts_unbounded() {
    let payload = "🧭".repeat(1_048_577);
    let input = format!("data:image/png;base64,{payload}");
    assert_eq!(
        gemini_media_data_url_parts(&input),
        Ok(Some(("image/png", payload.as_str())))
    );
    let input = "data:".to_string() + &";".repeat(65_537) + "base64,AA==";
    assert_eq!(
        gemini_media_data_url_parts(&input),
        Ok(Some(("application/octet-stream", "AA==")))
    );
}

#[test]
fn compiled_gemini_data_url_abi_rejects_invalid_inputs_without_output_writes() {
    let text = "data:;BASE64,x";
    let mut output = [71_i64; 6];
    let address = text.as_ptr() as usize as u64;
    let dest = output.as_mut_ptr() as usize as u64;
    let call = |version, source, length, out| unsafe {
        prodex_mojo_gemini_data_url_parts_v1(version, source, length, out)
    };
    assert_eq!(call(2, address, text.len() as i64, dest), 4);
    assert_eq!(call(1, address, -1, dest), 1);
    assert_eq!(call(1, address, text.len() as i64, 1), 1);
    assert_eq!(call(1, 0, 1, dest), 2);
    let invalid = [0xff_u8];
    assert_eq!(call(1, invalid.as_ptr() as usize as u64, 1, dest), 2);
    assert_eq!(output, [71; 6]);
    assert_eq!(call(1, address, text.len() as i64, dest), 0);
    assert_eq!(output, [1, 1, 0, 0, 13, 14]);
}
