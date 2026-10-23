use super::*;

fn original_width(raw: &str) -> usize {
    raw.as_bytes()
        .split(|byte| *byte != b'\\')
        .map(<[u8]>::len)
        .filter(|width| *width > 0)
        .min()
        .unwrap_or(1)
}

fn original_decode(raw: &str) -> String {
    let width = original_width(raw);
    let mut output = String::new();
    let mut run = 0_usize;
    for character in raw.chars() {
        if character == '\\' {
            run += 1;
        } else {
            output.extend(std::iter::repeat_n('\\', run.div_ceil(width)));
            run = 0;
            output.push(character);
        }
    }
    output.extend(std::iter::repeat_n('\\', run.div_ceil(width)));
    output
}

#[test]
fn compiled_session_path_codec_preserves_original_windows_text() {
    let rendered = "C:\\test-user\\账户🙂\0\\image.png\\";
    let mut cases = vec![
        String::new(),
        "plain/账户🙂\0/image.png".to_string(),
        r"C:\test-user\file.png".to_string(),
        r"C:\\test-user\\file.png".to_string(),
        r"\\server\\share\\file.png".to_string(),
        "\\\\\\".to_string(),
    ];
    for width in 1..=12 {
        for extra in 0..=width + 1 {
            cases.push(format!(
                "C:{}账户🙂\0{}file.png{}",
                "\\".repeat(width),
                "\\".repeat(width + extra),
                "\\".repeat(width * 2 + extra),
            ));
        }
    }
    cases.push(format!("{}{}", "账户🙂\\\\".repeat(4096), "\\".repeat(37)));
    for raw in cases {
        assert_eq!(decode_session_path(&raw), Ok(original_decode(&raw)));
        assert_eq!(
            render_session_path(rendered, &raw),
            Ok(rendered.replace('\\', &"\\".repeat(original_width(&raw)))),
        );
    }
}

#[test]
fn session_path_codec_abi_validates_before_output_or_metadata_writes() {
    let input = r"C:\\test-user\\file.png";
    let mut output = [0x5a_u8; 128];
    let mut metadata = [-7_i64; 2];
    let mut call = |abi, mode, pointer, length, phase, capacity| unsafe {
        prodex_shared_session_path_codec_v1(
            abi,
            mode,
            pointer,
            length,
            input.as_ptr() as u64,
            input.len() as i64,
            phase,
            output.as_mut_ptr() as u64,
            capacity,
            metadata.as_mut_ptr() as u64,
        )
    };
    let address = input.as_ptr() as u64;
    let length = input.len() as i64;
    for (abi, mode, pointer, count, phase, capacity, expected) in [
        (2, 0, address, length, 0, 128, 4),
        (1, 2, address, length, 0, 128, 1),
        (1, 0, 0, length, 0, 128, 1),
        (1, 0, address, -1, 0, 128, 1),
        (1, 0, address, 67_108_865, 0, 128, 1),
        (1, 0, address, length, 2, 128, 1),
        (1, 0, address, length, 1, 128, 1),
        (1, 0, address, length, 0, 0, 3),
        (1, 0, b"\xff".as_ptr() as u64, 1, 0, 128, 1),
    ] {
        assert_eq!(call(abi, mode, pointer, count, phase, capacity), expected);
    }
    assert_eq!(output, [0x5a; 128]);
    assert_eq!(metadata, [-7; 2]);
}
