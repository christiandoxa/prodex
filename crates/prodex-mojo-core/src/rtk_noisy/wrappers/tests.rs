use super::*;
#[path = "tests/original.rs"]
mod original;

#[test]
fn compiled_wrapper_scripts_match_complete_original_outputs_on_both_platforms() {
    for platform in [RtkWrapperPlatform::Posix, RtkWrapperPlatform::Windows] {
        for kind in [
            RtkWrapperKind::Command,
            RtkWrapperKind::Shell,
            RtkWrapperKind::Unavailable,
        ] {
            for states in [
                &[][..],
                &["/tmp/state's 10%/history.db", "/tmp/账户/tee", "/tmp/audit"][..],
            ] {
                for arguments in [
                    &[][..],
                    &["test", "run|build", "100%", "quote'", "账户🙂"][..],
                ] {
                    for command in [
                        "",
                        "/usr/bin/cargo",
                        "C:\\test-user's 10%\\账户🙂\\tool.exe",
                    ] {
                        let input = RtkWrapperInput {
                            platform,
                            kind,
                            command,
                            arguments,
                            state_paths: states,
                            rtk: "/opt/test-user's 10%/rtk",
                        };
                        let expected = original::expected(&input);
                        assert_eq!(render_rtk_wrapper(input), Ok(expected));
                    }
                }
            }
        }
    }
}

#[test]
fn wrapper_abi_rejects_invalid_shapes_before_output() {
    let text = "cargo";
    let texts = [StringView::from("rtk"), StringView::from(text)];
    let invalid_utf8 = [
        text.as_ptr() as u64,
        text.len() as u64,
        b"\xff".as_ptr() as u64,
        1,
    ];
    let mut output = [0x5a_u8; 64];
    let mut metadata = [-7_i64; 2];
    for (version, platform, kind, arguments, states, phase, capacity, address, expected) in [
        (2, 0, 0, 0, 0, 0, 64, texts.as_ptr() as u64, 4),
        (1, 2, 0, 0, 0, 0, 64, texts.as_ptr() as u64, 1),
        (1, 0, 3, 0, 0, 0, 64, texts.as_ptr() as u64, 1),
        (1, 0, 0, 4097, 0, 0, 64, texts.as_ptr() as u64, 1),
        (1, 0, 0, 0, 1, 0, 64, texts.as_ptr() as u64, 1),
        (1, 0, 0, 0, 0, 2, 64, texts.as_ptr() as u64, 1),
        (1, 0, 0, 0, 0, 1, 64, texts.as_ptr() as u64, 1),
        (1, 0, 0, 0, 0, 0, 0, texts.as_ptr() as u64, 3),
        (1, 0, 0, 0, 0, 0, 64, invalid_utf8.as_ptr() as u64, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_rtk_wrapper_script_v1(
                    version,
                    platform,
                    kind,
                    address,
                    0,
                    arguments,
                    0,
                    states,
                    phase,
                    output.as_mut_ptr() as u64,
                    capacity,
                    metadata.as_mut_ptr() as u64,
                )
            },
            expected
        );
        assert_eq!(output, [0x5a; 64]);
        assert_eq!(metadata, [-7; 2]);
    }
}
