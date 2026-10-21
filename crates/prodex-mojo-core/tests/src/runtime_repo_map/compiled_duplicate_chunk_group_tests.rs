use super::*;

fn record<'a>(hash: &'a str, byte_len: usize) -> RepoDuplicateChunkInput<'a> {
    RepoDuplicateChunkInput { hash, byte_len }
}

#[test]
fn compiled_duplicate_chunk_groups_sort_hash_and_size_with_source_order() {
    let inputs = [
        record("hashB", 3),
        record("hashA", 2),
        record("hashB", 3),
        record("hashA", 2),
        record("hashA", 5),
        record("hashA", 5),
        record("lonely", 2),
        record("hashB", 3),
    ];
    let plan = repo_duplicate_chunk_groups(&inputs, 4, 3).unwrap();
    assert_eq!(
        plan,
        RepoDuplicateChunkGroupsPlan {
            groups: vec![
                RepoDuplicateChunkGroup {
                    first_source: 1,
                    occurrence_count: 2,
                    occurrence_indices: vec![1, 3],
                },
                RepoDuplicateChunkGroup {
                    first_source: 4,
                    occurrence_count: 2,
                    occurrence_indices: vec![4, 5],
                },
                RepoDuplicateChunkGroup {
                    first_source: 0,
                    occurrence_count: 3,
                    occurrence_indices: vec![0, 2, 7],
                },
            ],
            complete: true,
        }
    );
    let limited = repo_duplicate_chunk_groups(&inputs, 2, 3).unwrap();
    assert_eq!(limited.groups.len(), 2);
    assert!(!limited.complete);
    let truncated = repo_duplicate_chunk_groups(&inputs, 4, 2).unwrap();
    assert_eq!(truncated.groups[2].occurrence_indices, vec![0, 2]);
    assert_eq!(truncated.groups[2].occurrence_count, 3);
    assert!(!truncated.complete);
    assert_eq!(
        repo_duplicate_chunk_groups(&[record("solo", 1)], 0, 0),
        Ok(RepoDuplicateChunkGroupsPlan {
            groups: vec![],
            complete: true,
        }),
    );
    assert_eq!(
        repo_duplicate_chunk_groups(&inputs, 0, 8).unwrap().complete,
        false,
    );
    assert_eq!(
        repo_duplicate_chunk_groups(&[], 64, 8),
        Ok(RepoDuplicateChunkGroupsPlan {
            groups: vec![],
            complete: true,
        }),
    );
}

#[test]
fn compiled_duplicate_chunk_groups_enforce_64_group_and_8_occurrence_limits() {
    let names = (0..65)
        .map(|index| format!("{index:04}"))
        .collect::<Vec<_>>();
    let mut cases = Vec::new();
    for name in &names {
        cases.push(record(name, 1));
        cases.push(record(name, 1));
    }
    let limit = repo_duplicate_chunk_groups(&cases, 64, 8).unwrap();
    assert_eq!(limit.groups.len(), 64);
    assert!(!limit.complete);
    assert_eq!(limit.groups[0].occurrence_indices, vec![0, 1]);
    assert_eq!(limit.groups[63].occurrence_indices, vec![126, 127]);
    assert_eq!(
        repo_duplicate_chunk_groups(&cases, 65, 8)
            .unwrap()
            .groups
            .len(),
        65
    );
    let repeated = vec![record("repeat", 1); 9];
    let plan = repo_duplicate_chunk_groups(&repeated, 64, 8).unwrap();
    assert_eq!(plan.groups.len(), 1);
    assert_eq!(plan.groups[0].occurrence_count, 9);
    assert_eq!(
        plan.groups[0].occurrence_indices,
        (0..8).collect::<Vec<_>>()
    );
    assert!(!plan.complete);
}

#[test]
fn compiled_duplicate_chunk_groups_abi_rejects_bad_late_rows_without_writes() {
    let mut rows = [0_u64; 6];
    let mut sorted = [73_i64; 2];
    let mut groups = [79_i64; 6];
    let mut result = [91_i64; 2];
    let source = rows.as_ptr() as usize as u64;
    let out = sorted.as_mut_ptr() as usize as u64;
    let group_ptr = groups.as_mut_ptr() as usize as u64;
    let result_ptr = result.as_mut_ptr() as usize as u64;
    for (version, input, count, max_group, max_occ, idx_cap, group_cap, result_addr, expected) in [
        (2, source, 2, 2, 3, 2, 6, result_ptr, 4),
        (1, 0, 2, 2, 3, 2, 6, result_ptr, 1),
        (1, source, -1, 2, 3, 2, 6, result_ptr, 1),
        (1, source, 65_537, 2, 3, 2, 6, result_ptr, 1),
        (1, source, 2, -1, 3, 2, 6, result_ptr, 1),
        (1, source, 2, 2, -1, 2, 6, result_ptr, 1),
        (1, source, 2, 2, 3, 1, 6, result_ptr, 1),
        (1, source, 2, 2, 3, 2, 5, result_ptr, 1),
        (1, source, 2, 2, 3, 2, 6, 0, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_runtime_repo_duplicate_groups_v1(
                    version,
                    input,
                    count,
                    max_group,
                    max_occ,
                    out,
                    idx_cap,
                    group_ptr,
                    group_cap,
                    result_addr,
                )
            },
            expected,
        );
        assert_eq!(sorted, [73; 2]);
        assert_eq!(groups, [79; 6]);
        assert_eq!(result, [91; 2]);
    }
    rows[4] = 1;
    assert_eq!(
        unsafe {
            prodex_runtime_repo_duplicate_groups_v1(
                1,
                rows.as_ptr() as usize as u64,
                2,
                2,
                3,
                out,
                2,
                group_ptr,
                6,
                result_ptr,
            )
        },
        1,
    );
    assert_eq!(sorted, [73; 2]);
    assert_eq!(groups, [79; 6]);
    assert_eq!(result, [91; 2]);
}
