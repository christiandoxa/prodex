use super::*;

#[test]
fn compiled_repo_map_path_set_filters_rust_unicode_whitespace_and_deduplicates_originals() {
    let paths = [
        Some("src/雪.rs"),
        None,
        Some("\u{2003} \t"),
        Some("src/a.rs"),
        Some("src/雪.rs"),
        Some(""),
        Some(" src/a.rs "),
        Some("\u{3000}"),
        Some("src/é.rs"),
        Some("src/a.rs"),
        Some("src/🦀.rs"),
    ];
    let indices = repo_map_unique_path_indices(&paths).unwrap();
    assert_eq!(
        indices
            .iter()
            .map(|index| paths[*index].unwrap())
            .collect::<Vec<_>>(),
        [
            " src/a.rs ",
            "src/a.rs",
            "src/é.rs",
            "src/雪.rs",
            "src/🦀.rs",
        ]
    );
    assert_eq!(indices, [6, 3, 8, 0, 10]);
    assert_eq!(repo_map_unique_path_indices(&[]), Ok(Vec::new()));
    assert_eq!(
        repo_map_unique_path_indices(&[None, Some(" "), Some("")]),
        Ok(Vec::new())
    );
    assert_eq!(
        repo_map_unique_path_indices(&[Some("a"), Some("a")]),
        Ok(vec![0])
    );

    let strings = (0..1025)
        .rev()
        .map(|i| format!("src/{i:04}.rs"))
        .collect::<Vec<_>>();
    let borrowed = strings.iter().map(|s| Some(s.as_str())).collect::<Vec<_>>();
    let indices = repo_map_unique_path_indices(&borrowed).unwrap();
    assert_eq!(indices.len(), 1025);
    assert_eq!(indices[0], 1024);
    assert_eq!(indices[1024], 0);
}

#[test]
fn compiled_repo_map_path_set_rejects_late_bad_input_without_writing() {
    let mut rows = [0_u64; 6];
    let mut output = [73_i64; 2];
    let mut selected = 91_i64;
    let input = rows.as_ptr() as usize as u64;
    let destination = output.as_mut_ptr() as usize as u64;
    let selected_ptr = (&mut selected as *mut i64) as usize as u64;
    for (abi, source, count, capacity, answer, expected) in [
        (2, input, 2, 2, selected_ptr, 4),
        (1, 0, 2, 2, selected_ptr, 1),
        (1, input, -1, 2, selected_ptr, 1),
        (1, input, 1_048_577, 2, selected_ptr, 1),
        (1, input, 2, 1, selected_ptr, 1),
        (1, input, 2, 2, 0, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_runtime_repo_map_path_set_v1(
                    abi,
                    source,
                    count,
                    destination,
                    capacity,
                    answer,
                )
            },
            expected,
        );
        assert_eq!(output, [73; 2]);
        assert_eq!(selected, 91);
    }
    rows[3] = 2;
    assert_eq!(
        unsafe {
            prodex_runtime_repo_map_path_set_v1(
                1,
                rows.as_ptr() as usize as u64,
                2,
                destination,
                2,
                selected_ptr,
            )
        },
        1,
    );
    assert_eq!(output, [73; 2]);
    assert_eq!(selected, 91);
    rows[3] = 1;
    rows[5] = 1;
    assert_eq!(
        unsafe {
            prodex_runtime_repo_map_path_set_v1(
                1,
                rows.as_ptr() as usize as u64,
                2,
                destination,
                2,
                selected_ptr,
            )
        },
        1,
        "present path with nonempty length and no address is malformed",
    );
    assert_eq!(output, [73; 2]);
    assert_eq!(selected, 91);
}
