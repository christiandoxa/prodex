use super::*;

#[test]
fn compiled_mcp_chunk_plan_selects_first_lf_and_preserves_first_byte_across_chunks() {
    assert_eq!(
        line_chunk_plan(b" \t{\nsecond", 0, None, McpLineLimit::FirstLine),
        Ok(McpLineChunkPlan {
            take: 4,
            first_non_whitespace: Some(b'{'),
            limit: MESSAGE_MAX_BYTES,
            too_large: false,
            complete: true,
        })
    );
    let whitespace = line_chunk_plan(b" \t\x0c\r", 0, None, McpLineLimit::FirstLine).unwrap();
    assert_eq!(
        whitespace,
        McpLineChunkPlan {
            take: 4,
            first_non_whitespace: None,
            limit: 4096,
            too_large: false,
            complete: false
        }
    );
    let array = line_chunk_plan(
        b"[1]\nignored",
        whitespace.take,
        whitespace.first_non_whitespace,
        McpLineLimit::FirstLine,
    )
    .unwrap();
    assert_eq!(
        array,
        McpLineChunkPlan {
            take: 4,
            first_non_whitespace: Some(b'['),
            limit: MESSAGE_MAX_BYTES,
            too_large: false,
            complete: true
        }
    );
    let continued = line_chunk_plan(b"C\xff\n", 10, Some(b'{'), McpLineLimit::FirstLine).unwrap();
    assert_eq!(
        continued,
        McpLineChunkPlan {
            take: 3,
            first_non_whitespace: Some(b'{'),
            limit: MESSAGE_MAX_BYTES,
            too_large: false,
            complete: true
        }
    );
    let vertical_tab = line_chunk_plan(b"\x0b{\n", 0, None, McpLineLimit::FirstLine).unwrap();
    assert_eq!(vertical_tab.first_non_whitespace, Some(11));
    assert_eq!(vertical_tab.limit, 4096);
}

#[test]
fn compiled_mcp_chunk_plan_enforces_exact_caps_without_unsigned_overflow() {
    assert!(
        !line_chunk_plan(b"x\n", 4094, None, McpLineLimit::FirstLine)
            .unwrap()
            .too_large
    );
    assert!(
        line_chunk_plan(b"x\n", 4095, None, McpLineLimit::FirstLine)
            .unwrap()
            .too_large
    );
    assert!(
        !line_chunk_plan(b"x\n", 2, None, McpLineLimit::Fixed(4))
            .unwrap()
            .too_large
    );
    assert!(
        line_chunk_plan(b"x\n", 3, None, McpLineLimit::Fixed(4))
            .unwrap()
            .too_large
    );
    assert!(
        line_chunk_plan(b"x\n", usize::MAX, None, McpLineLimit::Fixed(4))
            .unwrap()
            .too_large
    );
    assert_eq!(
        line_chunk_plan(b"", 0, None, McpLineLimit::Fixed(0)),
        Ok(McpLineChunkPlan {
            take: 0,
            first_non_whitespace: None,
            limit: 0,
            too_large: false,
            complete: false
        })
    );
    assert_eq!(
        line_chunk_plan(b"x", 0, None, McpLineLimit::Fixed(MESSAGE_MAX_BYTES + 1)),
        Err(MojoError::InvalidInput)
    );
    for byte in 0..=255 {
        let first = line_chunk_plan(&[byte], 0, None, McpLineLimit::FirstLine).unwrap();
        assert_eq!(
            first.first_non_whitespace,
            (!byte.is_ascii_whitespace()).then_some(byte)
        );
    }
}

#[test]
fn compiled_mcp_chunk_abi_rejects_invalid_shapes_without_writing() {
    let input = [b'{', 0xff, b'\n'];
    let source = input.as_ptr() as usize as u64;
    let mut output = [77_u64; 5];
    let address = output.as_mut_ptr() as usize as u64;
    for (version, mode, pointer, length, first, fixed, target, expected) in [
        (2, 0, 0, 0, -1, 0, address, 4),
        (1, 2, 0, 0, -1, 0, address, 1),
        (1, 0, 0, 1, -1, 0, address, 1),
        (1, 0, 0, -1, -1, 0, address, 1),
        (1, 0, 0, MESSAGE_MAX_BYTES as i64 + 2, -1, 0, address, 1),
        (1, 0, source, 3, -2, 0, address, 1),
        (1, 0, source, 3, 256, 0, address, 1),
        (
            1,
            1,
            source,
            3,
            -1,
            MESSAGE_MAX_BYTES as u64 + 1,
            address,
            1,
        ),
        (1, 0, source, 3, -1, 0, 0, 1),
        (1, 0, source, 3, -1, 0, address + 1, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_mcp_line_chunk_v1(version, mode, pointer, length, 0, first, fixed, target)
            },
            expected
        );
        assert_eq!(output, [77; 5]);
    }
    assert_eq!(
        unsafe { prodex_mcp_line_chunk_v1(1, 0, source, 3, u64::MAX, -1, 0, address) },
        0
    );
    assert_eq!(output, [3, 123, MESSAGE_MAX_BYTES as u64, 1, 1]);
}
