use super::*;

#[test]
fn compiled_retention_preserves_active_runs_and_full_width_time_order() {
    let mut rows = vec![
        SuperRunRetentionInput {
            terminal: false,
            created_at: 0,
            finished_at: None,
        },
        SuperRunRetentionInput {
            terminal: true,
            created_at: 9,
            finished_at: Some(4),
        },
        SuperRunRetentionInput {
            terminal: true,
            created_at: 1,
            finished_at: Some(4),
        },
        SuperRunRetentionInput {
            terminal: true,
            created_at: 2,
            finished_at: None,
        },
        SuperRunRetentionInput {
            terminal: true,
            created_at: i64::MAX as u64 + 1,
            finished_at: Some(u64::MAX),
        },
        SuperRunRetentionInput {
            terminal: false,
            created_at: u64::MAX,
            finished_at: None,
        },
    ];
    rows.extend(
        [SuperRunRetentionInput {
            terminal: true,
            created_at: u64::MAX,
            finished_at: Some(u64::MAX - 1),
        }; 30],
    );
    let mut expected = vec![true; rows.len()];
    expected[2] = false;
    expected[3] = false;
    assert_eq!(super_run_retention_mask(&rows).unwrap(), expected);
    assert_eq!(super_run_retention_mask(&[]).unwrap(), []);
    let tied = [SuperRunRetentionInput {
        terminal: true,
        created_at: 1,
        finished_at: None,
    }; 33];
    let expected = std::iter::once(false).chain([true; 32]).collect::<Vec<_>>();
    assert_eq!(super_run_retention_mask(&tied).unwrap(), expected);
}

#[test]
fn compiled_retention_matches_fixed_unsigned_eviction_contracts() {
    let rows = (0..80_u64)
        .map(|index| SuperRunRetentionInput {
            terminal: index % 4 != 0,
            created_at: if index % 3 == 0 {
                u64::MAX - index % 9
            } else {
                index % 9
            },
            finished_at: (index % 5 != 0).then_some(index % 7),
        })
        .collect::<Vec<_>>();
    let cases: &[(usize, &[usize])] = &[
        (0, &[]),
        (1, &[]),
        (31, &[]),
        (32, &[]),
        (33, &[]),
        (60, &[1, 7, 10, 14, 21, 22, 29, 37, 42, 43, 49, 55, 57]),
        (
            80,
            &[
                1, 2, 7, 9, 10, 14, 21, 22, 23, 29, 31, 37, 38, 42, 43, 49, 51, 55, 57, 58, 59, 63,
                65, 71, 73, 77, 78, 79,
            ],
        ),
    ];
    for &(count, removed) in cases {
        let mut expected = vec![true; count];
        for &index in removed {
            expected[index] = false;
        }
        assert_eq!(super_run_retention_mask(&rows[..count]).unwrap(), expected);
    }
}

#[test]
fn compiled_retention_rejects_malformed_shapes_before_writing() {
    let mut rows = [
        Row {
            terminal: 1,
            created_at: 0,
            finished_at: 0,
            finished_present: 0,
        },
        Row {
            terminal: 2,
            created_at: 0,
            finished_at: 0,
            finished_present: 0,
        },
    ];
    let mut keep = [7_u8; 2];
    let mut scratch = [77_i64; 2];
    let mut call = |version, address, count, capacity| unsafe {
        prodex_super_run_retention_v1(
            version,
            address,
            count,
            keep.as_mut_ptr() as u64,
            capacity,
            scratch.as_mut_ptr() as u64,
            2,
        )
    };
    assert_eq!(call(2, rows.as_ptr() as u64, 2, 2), 4);
    assert_eq!(call(1, rows.as_ptr() as u64 + 1, 2, 2), 1);
    assert_eq!(call(1, rows.as_ptr() as u64, 2, 1), 3);
    assert_eq!(call(1, rows.as_ptr() as u64, 2, 2), 1);
    assert_eq!(call(1, rows.as_ptr() as u64, -1, 2), 1);
    assert_eq!(call(1, rows.as_ptr() as u64, i64::MAX / 32 + 1, 2), 1);
    rows[1].terminal = 1;
    rows[1].finished_at = 1;
    assert_eq!(call(1, rows.as_ptr() as u64, 2, 2), 1);
    assert_eq!(keep, [7; 2]);
    assert_eq!(scratch, [77; 2]);
}
#[test]
fn compiled_event_push_bounds_deque_and_saturates_unsigned_sequence() {
    for (count, seq, next_seq, drop_front) in [
        (0, 0, 1, 0),
        (255, u64::MAX - 1, u64::MAX, 0),
        (256, u64::MAX, u64::MAX, 1),
        (300, 4, 5, 45),
    ] {
        assert_eq!(
            super_run_event_push_plan(count, seq),
            Ok(SuperRunEventPushPlan {
                next_seq,
                drop_front
            }),
        );
    }
}

#[test]
fn compiled_event_pages_preserve_original_order_cap_and_cursor_gaps() {
    let sequences = (0..300).collect::<Vec<u64>>();
    assert_eq!(
        super_run_events_page_plan(&sequences, 0, 300, usize::MAX),
        Ok(SuperRunEventPagePlan {
            indices: (1..257).collect(),
            truncated: false
        }),
    );
    for (after, next, limit, indices, truncated) in [
        (8, u64::MAX, 3, vec![0, 2, 3], false),
        (0, u64::MAX, 2, vec![0, 1], true),
        (0, u64::MAX, 0, vec![], true),
        (u64::MAX, u64::MAX, 9, vec![], false),
    ] {
        assert_eq!(
            super_run_events_page_plan(&[9, 1, 9, u64::MAX, u64::MAX], after, next, limit),
            Ok(SuperRunEventPagePlan { indices, truncated }),
        );
    }
    for (after, truncated) in [(98, true), (99, false), (u64::MAX, false)] {
        assert_eq!(
            super_run_events_page_plan(&[], after, 100, 9),
            Ok(SuperRunEventPagePlan {
                indices: vec![],
                truncated
            }),
        );
    }
}

#[test]
fn compiled_event_abis_reject_invalid_shapes_without_writing() {
    let sequences = [0_u64, u64::MAX];
    let mut indices = [77_i64; 2];
    let mut output = [71_i64; 2];
    let mut call = |version, source, count, capacity| unsafe {
        prodex_super_run_events_page_v1(
            version,
            source,
            count,
            0,
            0,
            256,
            indices.as_mut_ptr() as u64,
            capacity,
            output.as_mut_ptr() as u64,
        )
    };
    assert_eq!(call(2, sequences.as_ptr() as u64, 2, 2), 4);
    assert_eq!(call(1, sequences.as_ptr() as u64 + 1, 2, 2), 1);
    assert_eq!(call(1, 0, 2, 2), 1);
    assert_eq!(call(1, sequences.as_ptr() as u64, -1, 2), 1);
    assert_eq!(call(1, sequences.as_ptr() as u64, i64::MAX / 8 + 1, 2), 1);
    assert_eq!(call(1, sequences.as_ptr() as u64, 2, 1), 3);
    assert_eq!(indices, [77; 2]);
    assert_eq!(output, [71; 2]);
    for (version, count, expected) in [(2, 0, 4), (1, -1, 1)] {
        assert_eq!(
            unsafe {
                prodex_super_run_event_push_v1(version, count, 0, output.as_mut_ptr() as u64)
            },
            expected
        );
        assert_eq!(output, [71; 2]);
    }
}

fn input<'a>(text: &'a str, flags: &'a [bool]) -> SuperRunOutputInput<'a> {
    SuperRunOutputInput {
        text,
        output_bytes: 0,
        output_truncated: false,
        next_seq: 7,
        marker_flags: flags,
        event_is_marker: false,
        event_text_limit: 8192,
        output_limit: 262144,
    }
}

#[test]
fn compiled_output_plan_keeps_exact_prefixes_and_saturated_events() {
    let mut facts = input("🦀tail", &[]);
    facts.output_bytes = 262142;
    facts.next_seq = u64::MAX;
    assert_eq!(
        super_run_output_plan(facts),
        Ok(SuperRunOutputPlan {
            text_end: 8,
            append_end: 0,
            truncated: true,
            event_push: Some(SuperRunEventPushPlan {
                next_seq: u64::MAX,
                drop_front: 0
            }),
            marker_push: Some(SuperRunEventPushPlan {
                next_seq: u64::MAX,
                drop_front: 0
            }),
        })
    );
    let text = format!("{}é", "x".repeat(8191));
    assert_eq!(
        super_run_output_plan(input(&text, &[])),
        Ok(SuperRunOutputPlan {
            text_end: 8191,
            append_end: 8191,
            truncated: false,
            event_push: Some(SuperRunEventPushPlan {
                next_seq: 8,
                drop_front: 0
            }),
            marker_push: None,
        })
    );
    let mut facts = input("", &[]);
    facts.output_bytes = usize::MAX;
    assert_eq!(
        super_run_output_plan(facts),
        Ok(SuperRunOutputPlan {
            text_end: 0,
            append_end: 0,
            truncated: false,
            event_push: None,
            marker_push: None,
        })
    );
}

#[test]
fn compiled_output_plan_checks_markers_after_normal_event_eviction() {
    for (position, expected) in [(0, true), (1, false), (255, false)] {
        let mut flags = [false; 256];
        flags[position] = true;
        let mut facts = input("tail", &flags);
        facts.output_bytes = usize::MAX;
        let plan = super_run_output_plan(facts).unwrap();
        assert_eq!(
            plan.marker_push,
            expected.then_some(SuperRunEventPushPlan {
                next_seq: 9,
                drop_front: 1
            })
        );
        assert_eq!(
            plan.event_push,
            Some(SuperRunEventPushPlan {
                next_seq: 8,
                drop_front: 1
            })
        );
    }
    let mut facts = input("marker", &[false; 256]);
    facts.event_is_marker = true;
    facts.output_truncated = true;
    assert_eq!(super_run_output_plan(facts).unwrap().marker_push, None);
    let mut facts = input("more", &[]);
    facts.output_truncated = true;
    assert!(super_run_output_plan(facts).unwrap().marker_push.is_some());
}

#[test]
fn compiled_output_plan_rejects_late_invalid_flags_before_writes() {
    let invalid_text = [255_u8];
    let mut flags = [0_u8, 2];
    let mut row = OutputRow {
        text_address: 0,
        text_length: 0,
        output_bytes: 0,
        output_truncated: 0,
        next_seq: 0,
        markers_address: flags.as_ptr() as u64,
        marker_count: 2,
        event_is_marker: 0,
        event_text_limit: 8192,
        output_limit: 262144,
    };
    let mut output = [71_u64; 9];
    let mut call = |version, address| unsafe {
        prodex_super_run_output_plan_v1(version, address, output.as_mut_ptr() as u64)
    };
    assert_eq!(call(2, (&row as *const OutputRow) as u64), 4);
    assert_eq!(call(1, (&row as *const OutputRow) as u64 + 1), 1);
    assert_eq!(call(1, (&row as *const OutputRow) as u64), 1);
    flags[1] = 0;
    row.markers_address = flags.as_ptr() as u64;
    row.text_address = invalid_text.as_ptr() as u64;
    row.text_length = 1;
    assert_eq!(call(1, (&row as *const OutputRow) as u64), 1);
    row.text_length = 0;
    row.output_truncated = 2;
    assert_eq!(call(1, (&row as *const OutputRow) as u64), 1);
    assert_eq!(output, [71; 9]);
}
