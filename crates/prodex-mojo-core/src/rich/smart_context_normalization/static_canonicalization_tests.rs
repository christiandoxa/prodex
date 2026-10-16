use super::*;

#[test]
fn mojo_scope_parse_preserves_strict_lowercase_normalization_policy() {
    let valid = format!("scscope1:{}", "abcdef0123456789".repeat(4));
    let uppercase = format!("scscope1:{}", "ABCDEF0123456789".repeat(4));
    assert_eq!(
        smart_context_scope_parse(&valid).unwrap().as_deref(),
        Some(valid.as_str())
    );
    assert_eq!(
        smart_context_scope_parse(&uppercase).unwrap().as_deref(),
        Some(valid.as_str())
    );
    for invalid in ["", "scscope1:", "SCSCOPE1:", "scscope1:123", "scscope1:🧭"] {
        assert_eq!(
            smart_context_scope_parse(invalid).unwrap(),
            None,
            "{invalid:?}"
        );
    }
    assert_eq!(
        smart_context_scope_parse(&format!("scscope1:{}g", "0".repeat(63))).unwrap(),
        None
    );
    assert_eq!(
        smart_context_scope_parse(&format!("SCSCOPE1:{}", "A".repeat(64))).unwrap(),
        None
    );
}

#[test]
fn mojo_scope_parse_validated_output_and_invalid_abi() {
    let input = view("scscope1:abc");
    let mut output = [39u8; 73];
    let mut written = 88i64;
    let mut valid = 99i64;
    unsafe {
        assert_eq!(
            prodex_mojo_smart_context_scope_parse_v1(
                RICH_ABI_VERSION + 1,
                mojo_pointer_address(&input),
                mojo_mut_pointer_address(output.as_mut_ptr()),
                output.len() as i64,
                mojo_mut_pointer_address(&mut written),
                mojo_mut_pointer_address(&mut valid),
            ),
            4
        );
    }
    assert_eq!(output, [39; 73]);
    assert_eq!(written, 88);
    assert_eq!(valid, 99);
}

#[test]
fn mojo_scope_preimage_preserves_each_field_boundary_and_utf8_bytes() {
    let fields = ["tenant", "東京🧭", "openai", "/work/repo", ""];
    let output = smart_context_scope_preimage(fields).unwrap();
    let mut expected = b"prodex-smart-context-scope-v1\0".to_vec();
    for field in fields {
        expected.extend_from_slice(&(field.len() as u64).to_be_bytes());
        expected.extend_from_slice(field.as_bytes());
    }
    assert_eq!(output, expected);
    assert_ne!(
        smart_context_scope_preimage(["a", "bc", "", "", ""]).unwrap(),
        smart_context_scope_preimage(["ab", "c", "", "", ""]).unwrap(),
    );
}

#[test]
fn mojo_scope_digest_emits_exact_prefixed_lowercase_sha256_hex() {
    let mut digest = [0u8; 32];
    digest[0] = 0xab;
    digest[31] = 0x0f;
    assert_eq!(
        smart_context_scope_digest_text(&digest).unwrap(),
        format!("scscope1:ab{}0f", "00".repeat(30)),
    );
    assert_eq!(
        smart_context_scope_digest_text(&[0u8; 32]).unwrap(),
        format!("scscope1:{}", "00".repeat(32)),
    );
}

#[test]
fn mojo_scope_abi_rejects_version_and_short_capacity() {
    let mut output = [77u8; 73];
    let mut written = 123i64;
    let digest = [0u8; 32];
    let views = ["t", "p", "r", "w", "s"].map(view);
    unsafe {
        assert_eq!(
            prodex_mojo_smart_context_scope_preimage_v1(
                RICH_ABI_VERSION + 1,
                mojo_pointer_address(views.as_ptr()),
                5,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                output.len() as i64,
                mojo_mut_pointer_address(&mut written),
            ),
            4,
        );
        assert_eq!(
            prodex_mojo_smart_context_scope_preimage_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(views.as_ptr()),
                4,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                output.len() as i64,
                mojo_mut_pointer_address(&mut written),
            ),
            1,
        );
        assert_eq!(
            prodex_mojo_smart_context_scope_digest_text_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(digest.as_ptr()),
                31,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                output.len() as i64,
                mojo_mut_pointer_address(&mut written),
            ),
            1,
        );
        assert_eq!(
            prodex_mojo_smart_context_scope_digest_text_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(digest.as_ptr()),
                32,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                72,
                mojo_mut_pointer_address(&mut written),
            ),
            3,
        );
    }
    assert_eq!(output, [77; 73]);
    assert_eq!(written, 123);
}

#[test]
fn mojo_legacy_fnv_preserves_64_bit_hash_and_text_formats() {
    for (input, expected) in [
        (&b""[..], "cbf29ce484222325"),
        (&b"a"[..], "af63dc4c8601ec8c"),
        (&b"foobar"[..], "85944171f73967e8"),
        (&b"\0"[..], "af63bd4c8601b7df"),
    ] {
        assert_eq!(
            smart_context_legacy_fnv_text(input, false).unwrap(),
            expected
        );
        assert_eq!(
            smart_context_legacy_fnv_text(input, true).unwrap(),
            format!("scv:{expected}"),
        );
    }
    let input = "東京🧭".as_bytes();
    let fingerprint = smart_context_legacy_fnv_text(input, false).unwrap();
    assert_eq!(fingerprint.len(), 16);
    assert_eq!(
        smart_context_legacy_fnv_text(input, true).unwrap(),
        format!("scv:{fingerprint}"),
    );
}

#[test]
fn mojo_legacy_fnv_abi_fails_before_writing_for_invalid_input() {
    let mut output = [73u8; 20];
    let mut written = 99i64;
    unsafe {
        assert_eq!(
            prodex_mojo_smart_context_fnv_hash_text_v1(
                RICH_ABI_VERSION + 1,
                0,
                0,
                0,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                20,
                mojo_mut_pointer_address(&mut written),
            ),
            4,
        );
        assert_eq!(
            prodex_mojo_smart_context_fnv_hash_text_v1(
                RICH_ABI_VERSION,
                2,
                0,
                0,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                20,
                mojo_mut_pointer_address(&mut written),
            ),
            1,
        );
        assert_eq!(
            prodex_mojo_smart_context_fnv_hash_text_v1(
                RICH_ABI_VERSION,
                0,
                0,
                3,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                20,
                mojo_mut_pointer_address(&mut written),
            ),
            1,
        );
        assert_eq!(
            prodex_mojo_smart_context_fnv_hash_text_v1(
                RICH_ABI_VERSION,
                0,
                0,
                0,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                15,
                mojo_mut_pointer_address(&mut written),
            ),
            3,
        );
    }
    assert_eq!(output, [73; 20]);
    assert_eq!(written, 99);
}

#[test]
fn bounded_static_text_uses_mojo_prefix_normalization_and_lazy_digest() {
    use std::cell::Cell;
    assert_eq!(
        smart_context_static_bounded_text(
            "Generated at: 2026-10-15\nRule: Keep affinity\n",
            || {
                panic!("short text must not request a digest");
            },
        )
        .unwrap(),
        "Rule: Keep affinity",
    );
    let text = "🧭".repeat(65_537);
    let count = Cell::new(0);
    let result = smart_context_static_bounded_text(&text, || {
        count.set(count.get() + 1);
        "sc2:verified".to_string()
    })
    .unwrap();
    assert_eq!(count.get(), 1);
    assert_eq!(
        result,
        format!(
            "{}\npsc static fingerprint item truncated bytes={} hash=sc2:verified",
            "🧭".repeat(65_536),
            text.len()
        )
    );
    let exact = "é".repeat(131_072);
    assert_eq!(
        smart_context_static_bounded_text(&exact, || {
            panic!("exact 256 KiB source must not be truncated");
        })
        .unwrap(),
        exact
    );
}

#[test]
fn bounded_static_text_abi_requires_digest_without_changing_output() {
    let text = "🧭".repeat(65_537);
    let input = view(&text);
    let mut output = [77_u8; 8];
    let mut written = 55_i64;
    unsafe {
        assert_eq!(
            prodex_mojo_smart_context_static_bounded_text_v1(
                RICH_ABI_VERSION + 1,
                mojo_pointer_address(&input),
                0,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                8,
                mojo_mut_pointer_address(&mut written),
            ),
            4,
        );
        assert_eq!(
            prodex_mojo_smart_context_static_bounded_text_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&input),
                0,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                8,
                mojo_mut_pointer_address(&mut written),
            ),
            5,
        );
    }
    assert_eq!(output, [77; 8]);
    assert_eq!(written, 55);
}

#[test]
fn bounded_static_id_uses_mojo_utf8_cutoff_and_lazy_host_hash() {
    use std::cell::Cell;
    assert_eq!(
        smart_context_static_bounded_id("  \\folder\\🧭  ", || {
            panic!("a short ID must not require SHA-256")
        })
        .unwrap(),
        "/folder/🧭"
    );
    assert_eq!(
        smart_context_static_bounded_id(&"é".repeat(128), || {
            panic!("an exactly 256-byte ID is not truncated")
        })
        .unwrap(),
        "é".repeat(128),
    );
    let input = "🧭".repeat(65);
    let hashes = Cell::new(0);
    let output = smart_context_static_bounded_id(&input, || {
        hashes.set(hashes.get() + 1);
        "sc2:verified".to_owned()
    })
    .unwrap();
    assert_eq!(hashes.get(), 1);
    assert_eq!(output, format!("{}<id-hash=sc2:verified>", "🧭".repeat(64)));
    assert_eq!(
        smart_context_static_bounded_id("  \u{2003}\t  ", || {
            panic!("whitespace-only input must not be hashed")
        })
        .unwrap(),
        "",
    );
}

#[test]
fn bounded_static_id_rejects_invalid_abi_and_short_capacity() {
    let id = view("hello");
    let mut output = [21_u8; 2];
    let mut written = 77_i64;
    unsafe {
        assert_eq!(
            prodex_mojo_smart_context_static_bounded_id_v1(
                RICH_ABI_VERSION + 1,
                mojo_pointer_address(&id),
                0,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                2,
                mojo_mut_pointer_address(&mut written),
            ),
            4,
        );
        assert_eq!(
            prodex_mojo_smart_context_static_bounded_id_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&id),
                0,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                2,
                mojo_mut_pointer_address(&mut written),
            ),
            3,
        );
    }
}

#[test]
fn static_payload_serialization_is_mojo_owned_and_byte_exact() {
    assert_eq!(
        smart_context_static_prompt_payload(&[]).unwrap(),
        "prodex-smart-context-static-prompt-cache-v1\n",
    );
    let entries = [
        SmartContextStaticPayloadItem {
            id: "AGENTS.md",
            canonical_text: "rules\n",
            byte_len: 6,
        },
        SmartContextStaticPayloadItem {
            id: "🧭",
            canonical_text: "東京",
            byte_len: 6,
        },
    ];
    assert_eq!(
        smart_context_static_prompt_payload(&entries).unwrap(),
        "prodex-smart-context-static-prompt-cache-v1\n\
         id-bytes:9\nAGENTS.md\ntext-bytes:6\nrules\n\n\
         id-bytes:4\n🧭\ntext-bytes:6\n東京\n",
    );
}

#[test]
fn static_payload_rejects_abi_and_count_before_writing_output() {
    let entry = view("hello");
    let mut output = [71_u8; 64];
    let mut written = 88_i64;
    unsafe {
        assert_eq!(
            prodex_mojo_smart_context_static_prompt_payload_v1(
                RICH_ABI_VERSION + 1,
                mojo_pointer_address(&entry),
                mojo_pointer_address(&entry),
                0,
                1,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                64,
                mojo_mut_pointer_address(&mut written),
            ),
            4,
        );
        assert_eq!(
            prodex_mojo_smart_context_static_prompt_payload_v1(
                RICH_ABI_VERSION,
                0,
                0,
                0,
                1,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                64,
                mojo_mut_pointer_address(&mut written),
            ),
            1,
        );
    }
    assert_eq!(output, [71; 64]);
    assert_eq!(written, 88);
}

#[test]
fn mojo_static_digest_and_truncation_text_preserves_checksum_protocol() {
    let digest = [0_u8; 32];
    let hex = "00".repeat(32);
    assert_eq!(
        smart_context_static_digest_text(&digest, false).unwrap(),
        hex
    );
    assert_eq!(
        smart_context_static_digest_text(&digest, true).unwrap(),
        format!("scpc2:{hex}"),
    );
    let mut digest = [0_u8; 32];
    digest[0] = 0xab;
    digest[31] = 0xef;
    assert_eq!(
        smart_context_static_digest_text(&digest, false).unwrap(),
        format!("ab{}ef", "00".repeat(30)),
    );
    assert_eq!(
        smart_context_static_truncation_line(129, 9_999, &hex).unwrap(),
        format!("psc static fingerprint truncated 129 9999 {hex}\n"),
    );
    assert_eq!(
        smart_context_static_truncation_line(usize::MAX, 0, &hex).unwrap(),
        format!("psc static fingerprint truncated {} 0 {hex}\n", usize::MAX),
    );
}

#[test]
fn mojo_static_digest_and_truncation_abi_rejects_untrusted_fields() {
    let digest = [0_u8; 32];
    let bad_hash_storage = "gg".repeat(32);
    let bad_hash = view(&bad_hash_storage);
    let mut output = [89_u8; 160];
    let mut written = 71_i64;
    unsafe {
        assert_eq!(
            prodex_mojo_smart_context_static_digest_text_v1(
                RICH_ABI_VERSION + 1,
                1,
                mojo_pointer_address(digest.as_ptr()),
                32,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                160,
                mojo_mut_pointer_address(&mut written),
            ),
            4,
        );
        assert_eq!(
            prodex_mojo_smart_context_static_digest_text_v1(
                RICH_ABI_VERSION,
                2,
                mojo_pointer_address(digest.as_ptr()),
                32,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                160,
                mojo_mut_pointer_address(&mut written),
            ),
            1,
        );
        assert_eq!(
            prodex_mojo_smart_context_static_truncation_line_v1(
                RICH_ABI_VERSION,
                129,
                256,
                mojo_pointer_address(&bad_hash),
                mojo_mut_pointer_address(output.as_mut_ptr()),
                160,
                mojo_mut_pointer_address(&mut written),
            ),
            2,
        );
    }
    assert_eq!(output, [89; 160]);
    assert_eq!(written, 71);
}

#[test]
fn mojo_static_overflow_preimage_matches_legacy_binary_contract() {
    let id = "🧭/α";
    let hash = "sc2:abcdef";
    let length = usize::MAX;
    let bytes = smart_context_static_overflow_preimage(id, hash, length).unwrap();
    let mut expected = Vec::new();
    expected.extend_from_slice(id.as_bytes());
    expected.push(0);
    expected.extend_from_slice(hash.as_bytes());
    expected.push(0);
    expected.extend_from_slice(&length.to_le_bytes());
    assert_eq!(bytes, expected);
    assert_eq!(
        smart_context_static_overflow_preimage("", "", 0).unwrap(),
        [vec![0_u8, 0_u8], vec![0_u8; size_of::<usize>()]].concat(),
    );
}

#[test]
fn mojo_static_overflow_accumulator_wraps_each_digest_byte_independently() {
    let aggregate = [255_u8; 32];
    let digest = [1_u8; 32];
    assert_eq!(
        smart_context_static_overflow_accumulate(&aggregate, &digest).unwrap(),
        [0_u8; 32],
    );
    let aggregate = [200_u8; 32];
    let digest = [200_u8; 32];
    assert_eq!(
        smart_context_static_overflow_accumulate(&aggregate, &digest).unwrap(),
        [144_u8; 32],
    );
    assert_eq!(
        smart_context_static_overflow_accumulate(&[0_u8; 32], &[255_u8; 32]).unwrap(),
        [255_u8; 32],
    );
}

#[test]
fn mojo_static_overflow_abi_rejects_bad_width_and_capacity_before_writes() {
    let id = view("id");
    let hash = view("sc2:hash");
    let mut bytes = [42_u8; 40];
    let mut written = 77_i64;
    let mut accumulated = [42_u8; 32];
    let aggregate = [1_u8; 32];
    let digest = [2_u8; 32];
    unsafe {
        assert_eq!(
            prodex_mojo_smart_context_static_overflow_preimage_v1(
                RICH_ABI_VERSION + 1,
                mojo_pointer_address(&id),
                mojo_pointer_address(&hash),
                4,
                8,
                mojo_mut_pointer_address(bytes.as_mut_ptr()),
                40,
                mojo_mut_pointer_address(&mut written),
            ),
            4,
        );
        assert_eq!(
            prodex_mojo_smart_context_static_overflow_preimage_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&id),
                mojo_pointer_address(&hash),
                4,
                3,
                mojo_mut_pointer_address(bytes.as_mut_ptr()),
                40,
                mojo_mut_pointer_address(&mut written),
            ),
            1,
        );
        assert_eq!(
            prodex_mojo_smart_context_static_overflow_preimage_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&id),
                mojo_pointer_address(&hash),
                4,
                8,
                mojo_mut_pointer_address(bytes.as_mut_ptr()),
                1,
                mojo_mut_pointer_address(&mut written),
            ),
            3,
        );
        assert_eq!(
            prodex_mojo_smart_context_static_overflow_accumulate_v1(
                RICH_ABI_VERSION + 1,
                mojo_pointer_address(aggregate.as_ptr()),
                mojo_pointer_address(digest.as_ptr()),
                mojo_mut_pointer_address(accumulated.as_mut_ptr()),
                32,
            ),
            4,
        );
        assert_eq!(
            prodex_mojo_smart_context_static_overflow_accumulate_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(aggregate.as_ptr()),
                mojo_pointer_address(digest.as_ptr()),
                mojo_mut_pointer_address(accumulated.as_mut_ptr()),
                31,
            ),
            1,
        );
    }
    assert_eq!(bytes, [42; 40]);
    assert_eq!(written, 77);
    assert_eq!(accumulated, [42; 32]);
}

#[test]
fn static_pipeline_canonicalizes_newlines_noise_and_unicode_without_rust_fallback() {
    let input = "\r\n \t\r\nGenerated at: 2026-10-15T11:00:00Z\r\n \
                 Rule: Keep profile affinity  \u{2003}\r\n\r\n\
                 request_id=123e4567-e89b-12d3-a456-426614174000\r\n\
                 Rule: Keep route identity\n  \n";
    assert_eq!(
        canonicalize_smart_context_static_text(input).unwrap(),
        " Rule: Keep profile affinity\n\nRule: Keep route identity",
    );
    for (source, expected) in [
        ("", ""),
        ("\n\r\n\r", ""),
        ("  \u{2003}hello  \u{00a0}\r\n", "  \u{2003}hello"),
        ("alpha\n\n\n", "alpha"),
        ("\n\nalpha\n\nbeta\n\n", "alpha\n\nbeta"),
        ("first\rsecond\r\nthird\n", "first\nsecond\nthird"),
        ("\u{1f9ed} a\u{2028}b  ", "\u{1f9ed} a\u{2028}b"),
    ] {
        assert_eq!(
            canonicalize_smart_context_static_text(source).unwrap(),
            expected,
            "{source:?}",
        );
    }
}

#[test]
fn static_pipeline_keeps_semantic_text_and_redacts_inline_volatile_ids() {
    let before = "Trace request_id=123e4567-e89b-12d3-a456-426614174000 \
                  at 2026-05-04T01:02:03Z\nRule: Keep affinity";
    let after = "Trace request_id=123e4567-e89b-12d3-a456-426614174999 \
                 at 2026-05-05T09:08:07Z\nRule: Keep affinity";
    let expected = "Trace request_id=<id> at <timestamp>\nRule: Keep affinity";
    assert_eq!(
        canonicalize_smart_context_static_text(before).unwrap(),
        expected
    );
    assert_eq!(
        canonicalize_smart_context_static_text(after).unwrap(),
        expected
    );
}

#[test]
fn static_pipeline_rejects_invalid_abi_before_mutating_output() {
    let input = view("hi");
    let mut output = [93_u8; 8];
    let mut written = 77_i64;
    unsafe {
        assert_eq!(
            prodex_mojo_smart_context_static_canonicalize_v1(
                RICH_ABI_VERSION + 1,
                mojo_pointer_address(&input),
                mojo_mut_pointer_address(output.as_mut_ptr()),
                8,
                mojo_mut_pointer_address(&mut written),
            ),
            4,
        );
        assert_eq!(
            prodex_mojo_smart_context_static_canonicalize_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&input),
                mojo_mut_pointer_address(output.as_mut_ptr()),
                0,
                mojo_mut_pointer_address(&mut written),
            ),
            1,
        );
    }
    assert_eq!(output, [93; 8]);
    assert_eq!(written, 77);
}
