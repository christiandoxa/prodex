use super::*;

#[test]
fn compiled_image_tag_batch_preserves_quoted_unicode_and_malformed_tags() {
    let text = r#"before <image none> <image path="/tmp/猫.png"> <image path=\"/tmp/second.png\"> <image path=""> trailing <image path="unfinished"#;
    let rows = image_tag_path_ranges(text, ImageTagScan::WholeTags).unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| &text[row.start..row.end])
            .collect::<Vec<_>>(),
        ["/tmp/猫.png", "/tmp/second.png", ""]
    );
    assert_eq!(
        image_tag_path_ranges("plain <image unfinished", ImageTagScan::WholeTags),
        Ok(vec![])
    );
    assert_eq!(
        image_tag_path_ranges("", ImageTagScan::RewritePaths),
        Ok(vec![])
    );
    let escaped_priority = r#"<image path="ignored" extra path=\"chosen\">"#;
    let rows = image_tag_path_ranges(escaped_priority, ImageTagScan::WholeTags).unwrap();
    assert_eq!(&escaped_priority[rows[0].start..rows[0].end], "chosen");
}

#[test]
fn compiled_image_tag_batch_retains_distinct_observation_and_rewrite_cursors() {
    let nested = r#"<image path="outer" <image path="inner">>"#;
    let observe = image_tag_path_ranges(nested, ImageTagScan::WholeTags).unwrap();
    let rewrite = image_tag_path_ranges(nested, ImageTagScan::RewritePaths).unwrap();
    assert_eq!(
        observe
            .iter()
            .map(|row| &nested[row.start..row.end])
            .collect::<Vec<_>>(),
        ["outer"]
    );
    assert_eq!(
        rewrite
            .iter()
            .map(|row| &nested[row.start..row.end])
            .collect::<Vec<_>>(),
        ["outer", "inner"]
    );
    let many = "<image path=\"x\">".repeat(2_000);
    let rows = image_tag_path_ranges(&many, ImageTagScan::WholeTags).unwrap();
    assert_eq!(rows.len(), 2_000);
    assert!(rows.iter().all(|row| &many[row.start..row.end] == "x"));
}

#[test]
fn compiled_image_tag_batch_abi_rejects_invalid_inputs_and_short_capacity_atomically() {
    let text = r#"<image path="one"> <image path="two">"#;
    let mut output = [77_u64; 4];
    let mut required = 91_i64;
    let address = output.as_mut_ptr() as usize as u64;
    let metadata = (&mut required as *mut i64) as usize as u64;
    for (abi, mode, source, length, target, capacity, expected) in [
        (2, 0, 0, 0, address, 2, 4),
        (1, 2, 0, 0, address, 2, 1),
        (1, 0, 0, 1, address, 2, 1),
        (1, 0, 0, -1, address, 2, 1),
        (1, 0, 0, MAX_BYTES as i64 + 1, address, 2, 1),
        (1, 0, 0, 0, address + 1, 2, 1),
        (1, 0, 0, 0, address, -1, 1),
        (1, 0, 0, 0, 0, 2, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_shared_image_tag_paths_v1(
                    abi, mode, source, length, target, capacity, metadata,
                )
            },
            expected
        );
        assert_eq!(output, [77; 4]);
        assert_eq!(required, 91);
    }
    let invalid_utf8 = [0xff_u8];
    assert_eq!(
        unsafe {
            prodex_shared_image_tag_paths_v1(
                1,
                0,
                invalid_utf8.as_ptr() as usize as u64,
                1,
                address,
                2,
                metadata,
            )
        },
        1
    );
    assert_eq!(output, [77; 4]);
    assert_eq!(required, 91);
    assert_eq!(
        unsafe {
            prodex_shared_image_tag_paths_v1(
                1,
                0,
                text.as_ptr() as usize as u64,
                text.len() as i64,
                address,
                1,
                metadata,
            )
        },
        3
    );
    assert_eq!(required, 2);
    assert_eq!(output, [77; 4]);
    assert_eq!(
        unsafe {
            prodex_shared_image_tag_paths_v1(
                1,
                0,
                text.as_ptr() as usize as u64,
                text.len() as i64,
                address,
                2,
                metadata,
            )
        },
        0
    );
    assert_eq!(required, 2);
    assert_eq!(output, [13, 16, 32, 35]);
}
