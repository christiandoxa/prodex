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
