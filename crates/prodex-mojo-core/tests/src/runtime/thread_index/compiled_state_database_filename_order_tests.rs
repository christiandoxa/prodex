use super::*;

#[test]
fn compiled_thread_state_database_filename_order_descends_lexically_and_stably() {
    let names = [
        "state_10.sqlite",
        "state_9.sqlite",
        "state_雪.sqlite",
        "state_a.sqlite",
        "state_é.sqlite",
        "state_10.sqlite",
    ];
    assert_eq!(
        thread_state_database_filename_order(&names),
        Ok(vec![2, 4, 3, 1, 0, 5]),
    );
    assert_eq!(thread_state_database_filename_order(&[]), Ok(vec![]));
    let descending = (0..1025)
        .map(|index| format!("state_{index:04}.sqlite"))
        .collect::<Vec<_>>();
    let borrowed = descending.iter().map(String::as_str).collect::<Vec<_>>();
    let indices = thread_state_database_filename_order(&borrowed).unwrap();
    assert_eq!(indices.len(), 1025);
    assert_eq!(indices[0], 1024);
    assert_eq!(indices[1024], 0);
    assert!(
        indices
            .windows(2)
            .all(|pair| borrowed[pair[0]] >= borrowed[pair[1]])
    );
    assert_eq!(
        thread_state_database_filename_order(&["x".repeat(4097).as_str()]),
        Err(MojoError::InvalidInput),
    );
}

#[test]
fn compiled_thread_state_database_filename_order_abi_rejects_bad_late_row_without_writes() {
    let mut views = [0_u64; 4];
    let mut output = [79_i64; 2];
    let source = views.as_ptr() as usize as u64;
    let dest = output.as_mut_ptr() as usize as u64;
    for (version, address, count, capacity, expected) in [
        (2, source, 2, 2, 4),
        (1, 0, 2, 2, 1),
        (1, source, -1, 2, 1),
        (1, source, 65_537, 2, 1),
        (1, source, 2, 1, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_runtime_thread_index_database_order_v1(
                    version, address, count, dest, capacity,
                )
            },
            expected,
        );
        assert_eq!(output, [79; 2]);
    }
    views[3] = u64::MAX;
    assert_eq!(
        unsafe {
            prodex_runtime_thread_index_database_order_v1(
                1,
                views.as_ptr() as usize as u64,
                2,
                dest,
                2,
            )
        },
        1,
    );
    assert_eq!(output, [79; 2]);
    let invalid = [255_u8; 2];
    views[0] = invalid.as_ptr() as usize as u64;
    views[1] = 2;
    views[3] = 0;
    assert_eq!(
        unsafe {
            prodex_runtime_thread_index_database_order_v1(
                1,
                views.as_ptr() as usize as u64,
                2,
                dest,
                2,
            )
        },
        1,
    );
    assert_eq!(output, [79; 2]);
}
