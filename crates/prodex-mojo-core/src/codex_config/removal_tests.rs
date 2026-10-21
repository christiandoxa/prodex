use super::*;

#[test]
fn compiled_config_removal_preserves_pair_inline_separator_and_duplicate_contracts() {
    let arguments = [
        "--",
        "--config=model=inline",
        "-c",
        "\u{2003}model\u{00a0}=first",
        "-cmodel=short",
        "--config",
        "model=second",
        "-c",
        "other=keep",
        "-c",
    ];
    let views = arguments.map(|text| ConfigRemovalArgument {
        text,
        original_utf8: true,
    });
    assert_eq!(
        config_override_removal_mask(&views, "model", ConfigOverrideRemoval::FirstPair),
        Ok(vec![
            false, false, true, true, false, false, false, false, false, false
        ])
    );
    assert_eq!(
        config_override_removal_mask(&views, "model", ConfigOverrideRemoval::AllForms),
        Ok(vec![
            false, true, true, true, true, true, true, false, false, false
        ])
    );
    assert_eq!(
        config_override_removal_mask(&[], "model", ConfigOverrideRemoval::AllForms),
        Ok(vec![])
    );
    let overlapping = ["-c", "--config", "model=x"].map(|text| ConfigRemovalArgument {
        text,
        original_utf8: true,
    });
    assert_eq!(
        config_override_removal_mask(&overlapping, "model", ConfigOverrideRemoval::FirstPair),
        Ok(vec![false, true, true])
    );
}

#[test]
fn compiled_config_removal_keeps_non_utf8_paired_values_but_matches_lossy_inline_keys() {
    let views = [
        ConfigRemovalArgument {
            text: "-c",
            original_utf8: true,
        },
        ConfigRemovalArgument {
            text: "model=�",
            original_utf8: false,
        },
        ConfigRemovalArgument {
            text: "-cmodel=�",
            original_utf8: false,
        },
        ConfigRemovalArgument {
            text: "-cmodel_extra=x",
            original_utf8: true,
        },
    ];
    assert_eq!(
        config_override_removal_mask(&views, "model", ConfigOverrideRemoval::FirstPair),
        Ok(vec![false; 4])
    );
    assert_eq!(
        config_override_removal_mask(&views, "model", ConfigOverrideRemoval::AllForms),
        Ok(vec![false, false, true, false])
    );
}

#[test]
fn compiled_config_removal_abi_validates_all_records_before_writing_mask() {
    let text = "-cmodel=x";
    let input = [ArgumentView {
        address: text.as_ptr() as usize as u64,
        length: text.len() as u64,
        valid_utf8: 1,
    }];
    let mut output = [77_u8];
    let target = output.as_mut_ptr() as usize as u64;
    let key = "model";
    for (abi, mode, pointer, count, capacity, expected) in [
        (2, 0, 0, 0, 1, 4),
        (1, 2, 0, 0, 1, 1),
        (1, 0, 0, 1, 1, 1),
        (1, 0, 0, -1, 1, 1),
        (1, 0, 0, 1_048_577, 1_048_577, 1),
        (1, 0, input.as_ptr() as usize as u64 + 1, 1, 1, 1),
        (1, 0, input.as_ptr() as usize as u64, 1, 0, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_codex_config_removal_mask_v1(
                    abi,
                    mode,
                    pointer,
                    count,
                    key.as_ptr() as usize as u64,
                    key.len() as i64,
                    target,
                    capacity,
                )
            },
            expected
        );
        assert_eq!(output, [77]);
    }
    let malformed = [0xff_u8];
    for view in [
        ArgumentView {
            address: malformed.as_ptr() as usize as u64,
            length: 1,
            valid_utf8: 0,
        },
        ArgumentView {
            address: text.as_ptr() as usize as u64,
            length: text.len() as u64,
            valid_utf8: 2,
        },
    ] {
        assert_eq!(
            unsafe {
                prodex_codex_config_removal_mask_v1(
                    1,
                    1,
                    (&view as *const ArgumentView) as usize as u64,
                    1,
                    key.as_ptr() as usize as u64,
                    key.len() as i64,
                    target,
                    1,
                )
            },
            1
        );
        assert_eq!(output, [77]);
    }
    let mixed = [
        input[0],
        ArgumentView {
            address: malformed.as_ptr() as usize as u64,
            length: 1,
            valid_utf8: 0,
        },
    ];
    let mut untouched = [77_u8; 2];
    assert_eq!(
        unsafe {
            prodex_codex_config_removal_mask_v1(
                1,
                1,
                mixed.as_ptr() as usize as u64,
                2,
                key.as_ptr() as usize as u64,
                key.len() as i64,
                untouched.as_mut_ptr() as usize as u64,
                2,
            )
        },
        1
    );
    assert_eq!(untouched, [77; 2]);
    assert_eq!(
        unsafe {
            prodex_codex_config_removal_mask_v1(
                1,
                1,
                input.as_ptr() as usize as u64,
                1,
                key.as_ptr() as usize as u64,
                key.len() as i64,
                target,
                1,
            )
        },
        0
    );
    assert_eq!(output, [1]);
}
