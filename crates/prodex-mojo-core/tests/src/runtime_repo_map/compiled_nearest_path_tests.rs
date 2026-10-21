use super::*;

fn range(start: usize, end: usize, has_path: bool) -> RepoNearestPathInput {
    RepoNearestPathInput {
        start,
        end,
        has_path,
    }
}

#[test]
fn compiled_repo_nearest_path_uses_distance_then_start_then_first_index() {
    let cases = [
        range(1, 2, true),
        range(4, 5, true),
        range(2, 100, false),
        range(4, 5, true),
        range(1, 2, true),
    ];
    assert_eq!(repo_nearest_path_index(&cases, 3), Ok(Some(0)));
    assert_eq!(repo_nearest_path_index(&cases, 5), Ok(Some(1)));
    assert_eq!(repo_nearest_path_index(&cases, 99), Ok(Some(1)));
    assert_eq!(repo_nearest_path_index(&cases, 0), Ok(Some(0)));
    assert_eq!(repo_nearest_path_index(&cases, 4), Ok(Some(1)));
    assert_eq!(repo_nearest_path_index(&[], 4), Ok(None));
    assert_eq!(
        repo_nearest_path_index(&[range(1, 2, false), range(20, 30, false)], 9),
        Ok(None),
    );
    let same = [range(10, 20, true), range(10, 20, true)];
    assert_eq!(repo_nearest_path_index(&same, 12), Ok(Some(0)));
    let large = (0..1025_usize)
        .map(|start| range(start * 2, start * 2 + 1, true))
        .collect::<Vec<_>>();
    assert_eq!(repo_nearest_path_index(&large, 2048), Ok(Some(1024)));
}

#[test]
fn compiled_repo_nearest_path_abi_rejects_invalid_late_row_without_writes() {
    let mut rows = [0_i64; 6];
    rows[2] = 1;
    let mut output = 93_i64;
    let source = rows.as_ptr() as usize as u64;
    let result = (&mut output as *mut i64) as usize as u64;
    for (abi, ptr, count, line, dest, expected) in [
        (2, source, 2, 3, result, 4),
        (1, 0, 2, 3, result, 1),
        (1, source, -1, 3, result, 1),
        (1, source, 1_048_577, 3, result, 1),
        (1, source, 2, -1, result, 1),
        (1, source, 2, 3, 0, 1),
    ] {
        assert_eq!(
            unsafe { prodex_runtime_repo_nearest_path_v1(abi, ptr, count, line, dest) },
            expected,
        );
        assert_eq!(output, 93);
    }
    rows[5] = 2;
    assert_eq!(
        unsafe {
            prodex_runtime_repo_nearest_path_v1(1, rows.as_ptr() as usize as u64, 2, 3, result)
        },
        1,
    );
    assert_eq!(output, 93);
    rows[5] = 1;
    rows[3] = 30;
    rows[4] = 20;
    assert_eq!(
        unsafe {
            prodex_runtime_repo_nearest_path_v1(1, rows.as_ptr() as usize as u64, 2, 3, result)
        },
        1,
    );
    assert_eq!(output, 93);
    assert_eq!(
        unsafe { prodex_runtime_repo_nearest_path_v1(1, 0, 0, 3, result) },
        0,
    );
    assert_eq!(output, -1);
}
