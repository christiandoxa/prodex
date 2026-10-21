use super::*;

#[test]
fn compiled_artifact_order_prioritizes_latest_then_id_with_stable_duplicates() {
    let inputs = [
        RepoArtifactOrderInput {
            order: 1,
            id: "last",
        },
        RepoArtifactOrderInput {
            order: 10,
            id: "雪",
        },
        RepoArtifactOrderInput { order: 10, id: "a" },
        RepoArtifactOrderInput {
            order: 10, id: "é"
        },
        RepoArtifactOrderInput { order: 10, id: "a" },
        RepoArtifactOrderInput { order: 0, id: "" },
    ];
    assert_eq!(repo_artifact_order(&inputs), Ok(vec![2, 4, 3, 1, 0, 5]));
    assert_eq!(repo_artifact_order(&[]), Ok(vec![]));
    let lots = (0..1025)
        .map(|index| RepoArtifactOrderInput {
            order: (index % 10) as u64,
            id: if index % 2 == 0 { "a" } else { "b" },
        })
        .collect::<Vec<_>>();
    let selected = repo_artifact_order(&lots).unwrap();
    assert_eq!(selected.len(), lots.len());
    assert!(selected.windows(2).all(|window| {
        let left = lots[window[0]];
        let right = lots[window[1]];
        left.order > right.order
            || left.order == right.order
                && (left.id < right.id || left.id == right.id && window[0] < window[1])
    }));
}

#[test]
fn compiled_artifact_order_abi_rejects_malformed_late_rows_without_output() {
    let mut rows = [0_u64; 6];
    let mut output = [79_i64; 2];
    let source = rows.as_ptr() as usize as u64;
    let dest = output.as_mut_ptr() as usize as u64;
    for (abi, ptr, count, capacity, expected) in [
        (2, source, 2, 2, 4),
        (1, 0, 2, 2, 1),
        (1, source, -1, 2, 1),
        (1, source, 65_537, 2, 1),
        (1, source, 2, 1, 1),
    ] {
        assert_eq!(
            unsafe { prodex_runtime_repo_artifact_order_v1(abi, ptr, count, dest, capacity) },
            expected,
        );
        assert_eq!(output, [79; 2]);
    }
    rows[5] = 2;
    assert_eq!(
        unsafe {
            prodex_runtime_repo_artifact_order_v1(1, rows.as_ptr() as usize as u64, 2, dest, 2)
        },
        1,
    );
    assert_eq!(output, [79; 2]);
    assert_eq!(
        unsafe { prodex_runtime_repo_artifact_order_v1(1, 0, 0, dest, 2) },
        0,
    );
    assert_eq!(output, [79; 2]);
}
