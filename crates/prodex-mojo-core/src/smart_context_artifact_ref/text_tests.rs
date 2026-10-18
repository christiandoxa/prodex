use super::*;

#[test]
fn compiled_artifact_lines_preserve_endings_ranges_and_legacy_precedence() {
    for (source, ranges, expected) in [
        ("a\r\nb\nc", vec![(1, 99), (3, 3)], Some("a\nb\nc\nc")),
        ("a\r", vec![(1, 99)], Some("a\r")),
        ("a\n\n", vec![(2, 2)], Some("")),
        ("\r\n", vec![(1, 1)], Some("")),
        ("", vec![(1, 1)], None),
        ("a\nb", vec![(3, 4)], None),
        ("a\nb", vec![(1, 1), (3, 4)], None),
        ("東京\r\n🧭\n", vec![], Some("東京\r\n🧭\n")),
        (
            "\0東京\r\n🧭\n",
            vec![(2, usize::MAX), (1, 1)],
            Some("🧭\n\0東京"),
        ),
        ("a\nb", vec![(usize::MAX, usize::MAX)], None),
    ] {
        let ranges = ranges
            .into_iter()
            .map(|(start, end)| ArtifactLineRange { start, end })
            .collect::<Vec<_>>();
        assert_eq!(
            rehydrate_artifact_lines(source, &ranges, None)
                .unwrap()
                .as_deref(),
            expected,
            "{source:?}/{ranges:?}"
        );
        assert_eq!(
            artifact_line_byte_len(source, &ranges, None).unwrap(),
            expected.map(str::len),
            "{source:?}/{ranges:?}"
        );
    }
    let legacy = Some(ArtifactLineRange { start: 2, end: 3 });
    assert_eq!(
        rehydrate_artifact_lines("a\nb\nc", &[], legacy)
            .unwrap()
            .as_deref(),
        Some("b\nc")
    );
    assert_eq!(
        rehydrate_artifact_lines("a\nb\nc", &[ArtifactLineRange { start: 1, end: 1 }], legacy)
            .unwrap()
            .as_deref(),
        Some("a")
    );
    assert_eq!(
        rehydrate_artifact_lines("a", &[ArtifactLineRange { start: 0, end: 1 }], None),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        rehydrate_artifact_lines("a", &[ArtifactLineRange { start: 2, end: 1 }], None),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_artifact_replacement_preserves_summary_and_marker_precedence() {
    for (text, marker, replacement, has_line_range, expected) in [
        (
            "psc art psc:x\nsummary",
            "psc:x",
            "exact",
            false,
            Some(ArtifactReplacement::Complete("exact".into())),
        ),
        (
            "prodex-sc artifact prodex-artifact:sc:x\nsummary",
            "none",
            "exact",
            false,
            Some(ArtifactReplacement::Complete("exact".into())),
        ),
        (
            "psc art psc:x\nsummary",
            "psc:x",
            "line",
            true,
            Some(ArtifactReplacement::Continue(
                "psc art line\nsummary".into(),
            )),
        ),
        (
            "target psc:x",
            "target",
            "🧭",
            false,
            Some(ArtifactReplacement::Continue("🧭 psc:x".into())),
        ),
        (
            "prodex-artifact:sc:x psc:x",
            "none",
            "🧭",
            false,
            Some(ArtifactReplacement::Continue("🧭 psc:x".into())),
        ),
        (
            "psc:x/psc:x",
            "none",
            "🧭",
            false,
            Some(ArtifactReplacement::Continue("🧭/🧭".into())),
        ),
        (
            "  \u{2003}sc:x\n",
            "none",
            "exact",
            false,
            Some(ArtifactReplacement::Continue("exact".into())),
        ),
        (
            "aaaaa",
            "aa",
            "x",
            false,
            Some(ArtifactReplacement::Continue("xxa".into())),
        ),
        (
            "東京🧭",
            "",
            "|",
            false,
            Some(ArtifactReplacement::Continue("|東|京|🧭|".into())),
        ),
        (
            "psc:x",
            "psc:x",
            "",
            false,
            Some(ArtifactReplacement::Continue("".into())),
        ),
        ("unchanged", "none", "exact", false, None),
    ] {
        assert_eq!(
            replace_artifact_reference(ArtifactReplacementInput {
                text,
                id: "sc:x",
                marker,
                replacement,
                has_line_range
            })
            .unwrap(),
            expected,
            "{text:?}/{marker:?}"
        );
    }
    for (id, marker) in [
        ("sc:x", "psc:x"),
        ("sc2:x", "psc2:x"),
        ("legacy東京", "psc:legacy東京"),
        ("", "psc:"),
    ] {
        assert_eq!(
            replace_artifact_reference(ArtifactReplacementInput {
                text: &format!("need {marker}"),
                id,
                marker: "missing",
                replacement: "body",
                has_line_range: false,
            })
            .unwrap(),
            Some(ArtifactReplacement::Continue("need body".into()))
        );
    }
}

#[test]
fn compiled_artifact_replacement_handles_long_repeated_prefixes_without_shared_state() {
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                let text = format!("{}b", "a".repeat(120_000));
                let marker = format!("{}b", "a".repeat(1_000));
                assert_eq!(
                    replace_artifact_reference(ArtifactReplacementInput {
                        text: &text,
                        id: "sc:x",
                        marker: &marker,
                        replacement: "X",
                        has_line_range: false,
                    })
                    .unwrap(),
                    Some(ArtifactReplacement::Continue(format!(
                        "{}X",
                        "a".repeat(119_000)
                    )))
                );
            });
        }
    });
}

#[test]
fn compiled_artifact_text_abi_rejects_invalid_input_and_capacity_without_output_writes() {
    let text = b"a\nb";
    let ranges = [1_u64, 99];
    let mut scratch = [0_i64; 4];
    let valid = Input {
        text_address: text.as_ptr() as usize as u64,
        text_length: text.len() as u64,
        ranges: ranges.as_ptr() as usize as u64,
        range_count: 1,
        scratch: scratch.as_mut_ptr() as usize as u64,
        scratch_words: scratch.len() as i64,
        ..Input::default()
    };
    let mut output = [57_u8; 64];
    let mut metadata = [89_i64; 4];
    let address = (&valid as *const Input) as usize as u64;
    let target = output.as_mut_ptr() as usize as u64;
    let meta = metadata.as_mut_ptr() as usize as u64;
    let call = |abi, input, measuring, output, capacity, metadata, count| unsafe {
        prodex_smart_context_artifact_text_v1(
            abi, input, measuring, output, capacity, metadata, count,
        )
    };
    for args in [
        (2, address, 0, target, 64, meta, 4, 4),
        (1, 0, 0, target, 64, meta, 4, 1),
        (1, address, 2, target, 64, meta, 4, 1),
        (1, address, 0, 0, 64, meta, 4, 1),
        (1, address, 0, target, -1, meta, 4, 1),
        (1, address, 0, target, 1, meta, 4, 3),
        (1, address, 0, target, 64, 0, 4, 1),
        (1, address, 0, target, 64, meta, 3, 1),
    ] {
        assert_eq!(
            call(args.0, args.1, args.2, args.3, args.4, args.5, args.6),
            args.7
        );
        assert_eq!(output, [57; 64]);
        assert_eq!(metadata, [89; 4]);
    }
    let invalid_utf8 = [0xff_u8];
    for (field, value, expected) in [
        (0, 2, 1),
        (1, -1, 1),
        (2, 2, 1),
        (3, 0, 1),
        (4, 1, 3),
        (5, 0, 1),
        (6, 67_108_865, 1),
        (7, 0, 1),
        (8, invalid_utf8.as_ptr() as usize as i64, 1),
    ] {
        let mut bad = Input { ..valid };
        match field {
            0 => bad.operation = value,
            1 => bad.range_count = value,
            2 => bad.has_line_range = value,
            3 => bad.scratch = value as u64,
            4 => bad.scratch_words = value,
            5 => bad.ranges = value as u64,
            6 => bad.text_length = value as u64,
            7 => bad.text_address = value as u64,
            8 => {
                bad.text_address = value as u64;
                bad.text_length = 1;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            call(
                1,
                (&bad as *const Input) as usize as u64,
                0,
                target,
                64,
                meta,
                4
            ),
            expected,
            "field {field}"
        );
        assert_eq!(output, [57; 64]);
        assert_eq!(metadata, [89; 4]);
    }
    assert_eq!(call(1, address, 0, target, 3, meta, 4), 0);
    assert_eq!(&output[..3], text);
    assert_eq!(metadata, [1, 3, 0, 0]);
    assert!(output[3..].iter().all(|byte| *byte == 57));
}

#[test]
fn compiled_utf8_validation_preserves_multibyte_and_invalid_sequences_at_ascii_block_edges() {
    let sequences: &[(&[u8], bool)] = &[
        ("🧭".as_bytes(), true),
        ("ñ".as_bytes(), true),
        ("雪".as_bytes(), true),
        (&[0xc0, 0xaf], false),
        (&[0xed, 0xa0, 0x80], false),
        (&[0xf4, 0x90, 0x80, 0x80], false),
        (&[0xf0, 0x9f, 0xa7], false),
        (&[0x80], false),
    ];
    for offset in 0..129 {
        for &(sequence, valid) in sequences {
            for tail in [b"".as_slice(), b"ascii suffix".as_slice()] {
                let bytes = [vec![b'A'; offset], sequence.to_vec(), tail.to_vec()].concat();
                let input = Input {
                    text_address: bytes.as_ptr() as usize as u64,
                    text_length: bytes.len() as u64,
                    ..Input::default()
                };
                let mut output = [57_u8; 256];
                let mut metadata = [89_i64; 4];
                let status = unsafe {
                    prodex_smart_context_artifact_text_v1(
                        1,
                        (&input as *const Input) as usize as u64,
                        0,
                        output.as_mut_ptr() as usize as u64,
                        output.len() as i64,
                        metadata.as_mut_ptr() as usize as u64,
                        metadata.len() as i64,
                    )
                };
                if valid {
                    assert_eq!(status, 0);
                    assert_eq!(&output[..bytes.len()], bytes);
                    assert_eq!(metadata, [1, bytes.len() as i64, 0, 1]);
                } else {
                    assert_eq!(status, 1, "offset {offset}, sequence {sequence:?}");
                    assert_eq!(output, [57; 256]);
                    assert_eq!(metadata, [89; 4]);
                }
            }
        }
    }
}

#[test]
fn compiled_artifact_identity_plans_name_the_exact_existing_source() {
    let original = "unchanged\0東京🧭";
    assert!(matches!(
        rehydrate_artifact_lines(original, &[], None).unwrap(),
        Some(Cow::Borrowed(text)) if std::ptr::eq(text, original)
    ));
    let replacement = "restored\0東京🧭";
    let mut scratch = [0_i64; 32];
    for (operation, expected_source, expected) in [(0, 1, original), (1, 2, replacement)] {
        let input = Input {
            operation,
            text_address: original.as_ptr() as usize as u64,
            text_length: original.len() as u64,
            marker_address: original.as_ptr() as usize as u64,
            marker_length: original.len() as u64,
            replacement_address: replacement.as_ptr() as usize as u64,
            replacement_length: replacement.len() as u64,
            scratch: scratch.as_mut_ptr() as usize as u64,
            scratch_words: scratch.len() as i64,
            ..Input::default()
        };
        let mut metadata = [-1_i64; 4];
        assert_eq!(
            unsafe {
                prodex_smart_context_artifact_text_v1(
                    1,
                    (&input as *const Input) as usize as u64,
                    1,
                    0,
                    0,
                    metadata.as_mut_ptr() as usize as u64,
                    metadata.len() as i64,
                )
            },
            0
        );
        assert_eq!(metadata, [1, expected.len() as i64, 0, expected_source]);
    }
}
