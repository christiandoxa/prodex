use super::*;

fn item<'a>(order: u64, id: &'a str, byte_len: usize) -> RepoArtifactEvictionInput<'a> {
    RepoArtifactEvictionInput {
        order,
        id,
        byte_len,
    }
}

#[test]
fn compiled_artifact_eviction_selects_oldest_then_id_until_both_limits_hold() {
    let inputs = [
        item(1, "z", 4),
        item(1, "a", 3),
        item(2, "newer", 10),
        item(3, "latest", 1),
    ];
    assert_eq!(
        repo_artifact_eviction_plan(&inputs, 18, 2, 12),
        Ok(vec![1, 0])
    );
    assert_eq!(
        repo_artifact_eviction_plan(&inputs, 18, 4, 11),
        Ok(vec![1, 0])
    );
    assert_eq!(
        repo_artifact_eviction_plan(&inputs, 18, 2, 18),
        Ok(vec![1, 0])
    );
    assert_eq!(repo_artifact_eviction_plan(&inputs, 18, 4, 18), Ok(vec![]));
    assert_eq!(
        repo_artifact_eviction_plan(&inputs, 18, 4, 0),
        Ok(vec![1, 0, 2, 3])
    );
    assert_eq!(repo_artifact_eviction_plan(&[], 0, 0, 0), Ok(vec![]));
    assert_eq!(
        repo_artifact_eviction_plan(&[item(0, "x", usize::MAX)], 2, 0, 0),
        Ok(vec![0]),
        "total_bytes saturates to zero after evicting an oversized artifact",
    );
}

#[test]
fn compiled_artifact_eviction_handles_unicode_equal_order_and_large_batches() {
    let unicode = [
        item(4, "雪", 1),
        item(4, "é", 1),
        item(4, "a", 1),
        item(4, "a", 1),
    ];
    assert_eq!(
        repo_artifact_eviction_plan(&unicode, 4, 1, 4),
        Ok(vec![2, 3, 1])
    );
    let many = (0..1025)
        .map(|idx| item(idx as u64, "id", 1))
        .collect::<Vec<_>>();
    let chosen = repo_artifact_eviction_plan(&many, 1025, 1000, 1000).unwrap();
    assert_eq!(chosen, (0..25).collect::<Vec<_>>());
    assert_eq!(
        repo_artifact_eviction_plan(&many, 1025, 1025, 1025),
        Ok(vec![]),
    );
}

#[test]
fn compiled_artifact_eviction_abi_rejects_late_bad_input_without_partial_output() {
    let mut rows = [0_u64; 8];
    let mut scratch = [71_i64; 2];
    let mut output = [73_i64; 2];
    let mut written = 91_i64;
    let source = rows.as_ptr() as usize as u64;
    let work = scratch.as_mut_ptr() as usize as u64;
    let dest = output.as_mut_ptr() as usize as u64;
    let result = (&mut written as *mut i64) as usize as u64;
    for (abi, address, count, max_count, cap, out, expected) in [
        (2, source, 2, 1, 2, result, 4),
        (1, 0, 2, 1, 2, result, 1),
        (1, source, -1, 1, 2, result, 1),
        (1, source, 1_048_577, 1, 2, result, 1),
        (1, source, 2, -1, 2, result, 1),
        (1, source, 2, 1, 1, result, 1),
        (1, source, 2, 1, 2, 0, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_runtime_repo_artifact_eviction_v1(
                    abi, address, count, max_count, 2, 1, work, 2, dest, cap, out,
                )
            },
            expected,
        );
        assert_eq!(scratch, [71; 2]);
        assert_eq!(output, [73; 2]);
        assert_eq!(written, 91);
    }
    rows[6] = 1;
    assert_eq!(
        unsafe {
            prodex_runtime_repo_artifact_eviction_v1(
                1,
                rows.as_ptr() as usize as u64,
                2,
                1,
                2,
                1,
                work,
                2,
                dest,
                2,
                result,
            )
        },
        1,
    );
    assert_eq!(scratch, [71; 2]);
    assert_eq!(output, [73; 2]);
    assert_eq!(written, 91);
}
