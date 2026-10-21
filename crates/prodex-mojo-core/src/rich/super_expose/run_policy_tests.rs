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
fn compiled_retention_matches_the_original_eviction_order() {
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
    for count in [0, 1, 31, 32, 33, 60, 80] {
        let rows = &rows[..count];
        let mut expected = vec![true; rows.len()];
        while rows
            .iter()
            .zip(&expected)
            .filter(|(row, keep)| row.terminal && **keep)
            .count()
            > 32
        {
            let index = rows
                .iter()
                .enumerate()
                .filter(|(index, row)| row.terminal && expected[*index])
                .min_by_key(|(_, row)| (row.finished_at.unwrap_or(row.created_at), row.created_at))
                .map(|(index, _)| index)
                .unwrap();
            expected[index] = false;
        }
        assert_eq!(super_run_retention_mask(rows).unwrap(), expected);
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
