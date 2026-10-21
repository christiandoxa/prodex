use super::*;

#[test]
fn compiled_live_log_json_projection_escapes_keys_and_nested_utf8_strings() {
    use crate::json::JsonKind;
    let raw = "9007199254740991";
    let nodes = [
        JsonNode {
            kind: JsonKind::Object,
            key: "",
            text: "",
            first_child: Some(1),
            next_sibling: None,
            parent: None,
            raw_start: 0,
            raw_length: 0,
        },
        JsonNode {
            kind: JsonKind::Array,
            key: "key\n\"雪",
            text: "",
            first_child: Some(2),
            next_sibling: None,
            parent: Some(0),
            raw_start: 0,
            raw_length: 0,
        },
        JsonNode {
            kind: JsonKind::String,
            key: "",
            text: "line\n\r\t\"\\雪",
            first_child: None,
            next_sibling: Some(3),
            parent: Some(1),
            raw_start: 0,
            raw_length: 0,
        },
        JsonNode {
            kind: JsonKind::Object,
            key: "",
            text: "",
            first_child: Some(4),
            next_sibling: None,
            parent: Some(1),
            raw_start: 0,
            raw_length: 0,
        },
        JsonNode {
            kind: JsonKind::Number,
            key: "large",
            text: "",
            first_child: None,
            next_sibling: Some(5),
            parent: Some(3),
            raw_start: 0,
            raw_length: raw.len(),
        },
        JsonNode {
            kind: JsonKind::True,
            key: "flag",
            text: "",
            first_child: None,
            next_sibling: None,
            parent: Some(3),
            raw_start: 0,
            raw_length: 0,
        },
    ];
    assert_eq!(
            clipped_json_projection(&nodes, raw),
            Ok("{\"key\\n\\\"雪\":[\"line\\n\\r\\t\\\"\\\\雪\",{\"large\":9007199254740991,\"flag\":true}]}".to_string()),
        );
    let scalar = JsonNode {
        kind: JsonKind::Null,
        key: "",
        text: "",
        parent: None,
        first_child: None,
        next_sibling: None,
        raw_start: 0,
        raw_length: 0,
    };
    assert_eq!(
        clipped_json_projection(&[scalar], ""),
        Ok("null".to_string())
    );
}

#[test]
fn compiled_live_log_json_projection_clips_exact_utf8_prefix_without_recursive_host_policy() {
    use crate::json::JsonKind;
    let string = format!("{}é{}", "x".repeat(8191), "y".repeat(8192));
    let nodes = [JsonNode {
        kind: JsonKind::String,
        key: "",
        text: &string,
        parent: None,
        first_child: None,
        next_sibling: None,
        raw_start: 0,
        raw_length: 0,
    }];
    let expected = format!("\"{}é …[truncated]\"", "x".repeat(8191));
    assert_eq!(clipped_json_projection(&nodes, ""), Ok(expected));

    let mut deep = Vec::new();
    for i in 0..1024_usize {
        deep.push(JsonNode {
            kind: JsonKind::Array,
            key: "",
            text: "",
            parent: i.checked_sub(1),
            first_child: Some(i + 1),
            next_sibling: None,
            raw_start: 0,
            raw_length: 0,
        });
    }
    deep.push(JsonNode {
        kind: JsonKind::String,
        key: "",
        text: "雪",
        parent: Some(1023),
        first_child: None,
        next_sibling: None,
        raw_start: 0,
        raw_length: 0,
    });
    let result = clipped_json_projection(&deep, "").unwrap();
    assert!(result.starts_with(&"[".repeat(1024)));
    assert!(result.ends_with(&"]".repeat(1024)));
    assert!(result.contains("\"雪\""));
}

#[test]
fn compiled_live_log_json_projection_abi_rejects_version_flags_and_capacity_before_metadata() {
    use crate::json::{JsonKind, ffi_nodes};
    let nodes = [JsonNode {
        kind: JsonKind::String,
        key: "",
        text: "hello",
        parent: None,
        first_child: None,
        next_sibling: None,
        raw_start: 0,
        raw_length: 0,
    }];
    let raw = "";
    let ffi = ffi_nodes(&nodes, raw).unwrap();
    let mut scratch = [0_u64; 2];
    let mut metadata = [83_i64; 2];
    let mut output = [71_u8; 3];
    let ptr = ffi.as_ptr() as usize as u64;
    let scratch_ptr = scratch.as_mut_ptr() as usize as u64;
    let metadata_ptr = metadata.as_mut_ptr() as usize as u64;
    let output_ptr = output.as_mut_ptr() as usize as u64;
    for (abi, operation, flag, count, measuring, destination, capacity, expected) in [
        (2, 0, 0, 1, 1, 0, 0, 4),
        (1, 1, 0, 1, 1, 0, 0, 1),
        (1, 0, 1, 1, 1, 0, 0, 1),
        (1, 0, 0, 0, 1, 0, 0, 1),
        (1, 0, 0, 1, 0, output_ptr, 3, 3),
    ] {
        assert_eq!(
            unsafe {
                prodex_live_log_json_projection_v1(
                    abi,
                    operation,
                    flag,
                    ptr,
                    count,
                    0,
                    0,
                    scratch_ptr,
                    2,
                    measuring,
                    destination,
                    capacity,
                    metadata_ptr,
                )
            },
            expected,
        );
        assert_eq!(metadata, [83; 2]);
    }
    // A rejected malformed tree must not mutate metadata either.
    let invalid = [JsonNode {
        parent: Some(0),
        ..nodes[0]
    }];
    let invalid_ffi = ffi_nodes(&invalid, "").unwrap();
    assert_eq!(
        unsafe {
            prodex_live_log_json_projection_v1(
                1,
                0,
                0,
                invalid_ffi.as_ptr() as usize as u64,
                1,
                0,
                0,
                scratch_ptr,
                2,
                1,
                0,
                0,
                metadata_ptr,
            )
        },
        1,
    );
    assert_eq!(metadata, [83; 2]);
}

#[test]
fn compiled_live_log_compact_metadata_serializes_sorted_keys_and_newline() {
    assert_eq!(
        compact_json_metadata(Some("\"雪\""), Some("42"), Some("\"2026-10-20T00:00:00Z\"")),
        Ok(concat!(
            "{\"event\":\"雪\",",
            "\"message\":\"[live log record truncated]\",",
            "\"pid\":42,",
            "\"timestamp\":\"2026-10-20T00:00:00Z\"}\n",
        )
        .to_string()),
    );
    assert_eq!(
        compact_json_metadata(None, None, None),
        Ok("{\"message\":\"[live log record truncated]\"}\n".to_string()),
    );
    assert_eq!(
            compact_json_metadata(
                Some("\"a\\n\\\\b\""),
                None,
                Some("null"),
            ),
            Ok("{\"event\":\"a\\n\\\\b\",\"message\":\"[live log record truncated]\",\"timestamp\":null}\n".to_string()),
        );
}

#[test]
fn compiled_live_log_compact_metadata_abi_rejects_bad_shapes_before_writes() {
    let mut fields = [0_u64; 9];
    let mut written = 87_i64;
    let output = mutable_pointer_address(&mut written as *mut i64);
    for (version, count, input, expected) in [
        (2, 3, pointer_address(fields.as_ptr()), 4),
        (1, 2, pointer_address(fields.as_ptr()), 1),
        (1, 3, 0, 1),
    ] {
        assert_eq!(
            unsafe { prodex_live_log_compact_json_v1(version, input, count, 0, 0, 1, output,) },
            expected,
        );
        assert_eq!(written, 87);
    }
    fields[0] = 3;
    assert_eq!(
        unsafe {
            prodex_live_log_compact_json_v1(1, pointer_address(fields.as_ptr()), 3, 0, 0, 1, output)
        },
        1
    );
    assert_eq!(written, 87);
    fields[0] = 0;
    let mut buffer = [79_u8; 4];
    assert_eq!(
        unsafe {
            prodex_live_log_compact_json_v1(
                1,
                pointer_address(fields.as_ptr()),
                3,
                mutable_pointer_address(buffer.as_mut_ptr()),
                4,
                0,
                output,
            )
        },
        2
    );
    assert_eq!(written, 87);
}

#[test]
fn compiled_live_log_retention_bounds_keep_exact_thresholds_and_ties() {
    use LiveLogRetentionScope::{Path, Store};
    for (scope, entries, bytes, paths, expected) in [
        (Path, 512, 2 * 1024 * 1024, 100, false),
        (Path, 513, 0, 0, true),
        (Path, 0, 2 * 1024 * 1024 + 1, 0, true),
        (Store, 2048, 8 * 1024 * 1024, 64, false),
        (Store, 2049, 0, 0, true),
        (Store, 0, 8 * 1024 * 1024 + 1, 0, true),
        (Store, 0, 0, 65, true),
        (Store, usize::MAX, usize::MAX, usize::MAX, true),
    ] {
        assert_eq!(
            retention_trim_required(scope, entries, bytes, paths).unwrap(),
            expected
        );
    }
    assert_eq!(oldest_retention_path(&[12, 7, 7, 9]).unwrap(), 1);
    assert_eq!(oldest_retention_path(&[u64::MAX, 0, 1]).unwrap(), 1);
    assert_eq!(oldest_retention_path(&[7]).unwrap(), 0);
    assert_eq!(oldest_retention_path(&[]), Err(MojoError::InvalidInput));
    assert_eq!(
        oldest_retention_path(&vec![1_u64; 66]),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_live_log_retention_abi_rejects_bad_input_before_output_writes() {
    let mut decision = 91_i64;
    let result = (&mut decision as *mut i64) as usize as u64;
    for (abi, scope, address, expected) in [
        (2, 0, result, 4),
        (1, -1, result, 1),
        (1, 2, result, 1),
        (1, 0, 0, 1),
    ] {
        assert_eq!(
            unsafe { prodex_live_log_retention_trim_v1(abi, scope, 512, 0, 0, address) },
            expected
        );
        assert_eq!(decision, 91);
    }
    let mut oldest = 72_i64;
    let output = (&mut oldest as *mut i64) as usize as u64;
    for (abi, count, sequences, expected) in
        [(2, 1, 8, 4), (1, 0, 8, 1), (1, 66, 8, 1), (1, 1, 0, 1)]
    {
        assert_eq!(
            unsafe { prodex_live_log_oldest_path_v1(abi, sequences, count, output) },
            expected
        );
        assert_eq!(oldest, 72);
    }
}

#[test]
fn runtime_live_log_mojo_adapter_checks_bounds_and_selected_metadata() {
    assert!(!record_exceeds_bound(MAX_RECORD_BYTES).unwrap());
    assert!(record_exceeds_bound(MAX_RECORD_BYTES + 1).unwrap());
    assert_eq!(
        json_plan(MAX_RECORD_BYTES).unwrap(),
        LiveLogJsonPlan {
            compact_metadata: false,
            timestamp: false,
            pid: false,
            event: false,
        }
    );
    assert_eq!(
        json_plan(MAX_RECORD_BYTES + 1).unwrap(),
        LiveLogJsonPlan {
            compact_metadata: true,
            timestamp: true,
            pid: true,
            event: true,
        }
    );
}

#[test]
fn runtime_live_log_mojo_adapter_returns_utf8_clip_boundaries() {
    let ascii = "a".repeat(8 * 1024 + 1);
    assert_eq!(nested_string_clip_end(&ascii).unwrap(), 8 * 1024);
    let unicode = format!("{}é-tail", "a".repeat(8 * 1024 - 1));
    assert_eq!(nested_string_clip_end(&unicode).unwrap(), 8 * 1024 + 1);
}

#[test]
fn runtime_live_log_mojo_adapter_pages_after_cursor_and_clamps_limit() {
    assert_eq!(
        snapshot_plan(&[7, 9, 11], 7, usize::MAX, 2, 20, 3).unwrap(),
        LiveLogSnapshotPlan {
            start: 1,
            count: 2,
            cursor: 11,
            dropped: 3,
        }
    );
    assert_eq!(
        snapshot_plan(&[7, 9, 11], 11, 2, 2, 20, 3).unwrap(),
        LiveLogSnapshotPlan {
            start: 3,
            count: 0,
            cursor: 20,
            dropped: 3,
        }
    );
}

#[test]
fn runtime_live_log_mojo_adapter_rejects_unsorted_sequences() {
    assert_eq!(
        snapshot_plan(&[9, 7], 0, 1, 2, 9, 0),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn runtime_log_follow_mojo_adapter_classifies_line_and_pending_bounds() {
    const { assert!(crate::MOJO_REQUIRED) };

    let plan = follow_line_plan(
        &[FOLLOW_MAX_LINE_BYTES, FOLLOW_MAX_LINE_BYTES + 1],
        FOLLOW_MAX_PENDING_BYTES + 1,
    )
    .unwrap();
    assert_eq!(plan.keep_lines, [true, false]);
    assert_eq!(plan.dropped_lines, 1);
    assert!(plan.pending_gap);
    assert_eq!(
        follow_line_plan(&vec![0; 2 * 1024 * 1024 + 1], 0),
        Err(MojoError::InvalidInput)
    );
}
