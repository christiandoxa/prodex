use super::*;

#[test]
fn codebase_version_contract_preserves_dev_prerelease_and_exact_input() {
    for (line, expected) in [
        ("codebase-memory-mcp dev", true),
        ("codebase-memory-mcp 0.9.1-rc.1", true),
        ("codebase-memory-mcp 0.9.1", true),
        ("codebase-memory-mcp 1.0.0-alpha+build.01", true),
        ("codebase-memory-mcp 0.9.1-rc.0", false),
        ("codebase-memory-mcp 0.9.0", false),
        ("codebase-memory-mcp v0.9.1", false),
        ("codebase-memory-mcp 0.9.1 ", false),
        ("codebase-memory-mcp \u{2003}0.9.1", false),
        ("codebase-memory-mcp 0.9.1 extra", false),
        (" codebase-memory-mcp 0.9.1", false),
        ("codebase-memory-mcp DEV", false),
    ] {
        assert_eq!(
            codebase_memory_version_supported(line, "0.9.1-rc.1"),
            Ok(expected),
            "{line}"
        );
    }
    assert_eq!(
        codebase_memory_version_supported("codebase-memory-mcp dev", "invalid"),
        Ok(true)
    );
}

#[test]
fn daemon_status_contract_preserves_success_and_prefix_semantics() {
    for success in [false, true] {
        for (line, expected) in [
            ("daemon: active (permanent)", success),
            ("daemon: active-other", success),
            ("daemon: not running", true),
            ("daemon: not running yet", true),
            (" daemon: active", false),
            ("Daemon: active", false),
            ("usage: daemon <command>", false),
            ("", false),
        ] {
            assert_eq!(
                codebase_memory_daemon_status_supported(success, line),
                Ok(expected)
            );
        }
    }
}

#[test]
fn newest_stable_version_preserves_total_order_exact_syntax_and_first_ties() {
    assert_eq!(optional_tool_newest_stable_version(&[]), Ok(None));
    assert_eq!(
        optional_tool_newest_stable_version(&["v99.0.0", " 9.0.0", "2.0.0-rc.1", "01.2.3"]),
        Ok(None)
    );
    assert_eq!(
        optional_tool_newest_stable_version(&["1.9.0", "1.10.0", "2.0.0-rc.1"]),
        Ok(Some(1))
    );
    assert_eq!(
        optional_tool_newest_stable_version(&["1.2.3", "1.2.3"]),
        Ok(Some(0))
    );
    assert_eq!(
        optional_tool_newest_stable_version(&["1.2.3+build.01", "1.2.3+build.2"]),
        Ok(Some(1))
    );
    assert_eq!(
        optional_tool_newest_stable_version(&[
            "18446744073709551616.0.0",
            "18446744073709551615.0.0"
        ]),
        Ok(Some(1))
    );
    assert_eq!(
        optional_tool_version_at_least("0.9.1-rc.1", "0.9.1-rc.1"),
        Ok(true)
    );
    assert_eq!(optional_tool_version_at_least("v1.0.0", "0.9.1"), Ok(false));
    assert_eq!(
        optional_tool_version_at_least("1.0.0", "invalid"),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn stable_manifest_version_distinguishes_syntax_from_compatibility() {
    for (version, expected) in [
        ("1.2.3", OptionalToolStableVersion::Supported),
        ("1.2.3+build.01", OptionalToolStableVersion::Supported),
        ("1.2.2", OptionalToolStableVersion::Incompatible),
        ("2.0.0-rc.1", OptionalToolStableVersion::Incompatible),
        ("v1.2.3", OptionalToolStableVersion::Malformed),
        ("1.2.03", OptionalToolStableVersion::Malformed),
        ("1.2.3\u{a0}", OptionalToolStableVersion::Malformed),
    ] {
        assert_eq!(optional_tool_stable_version(version, "1.2.3"), Ok(expected));
    }
}

#[test]
fn version_policy_abi_rejects_invalid_flags_views_and_alignment_without_writes() {
    let text = "1.0.0";
    let minimum = "0.9.1";
    let invalid_utf8 = [0xff_u8];
    let mut output = 77_i64;
    let pointer = &mut output as *mut i64 as u64;
    for (abi, operation, success, address, length, target, expected) in [
        (2, 0, 0, 0, 0, pointer, 4),
        (1, 4, 0, 0, 0, pointer, 1),
        (1, 0, 2, 0, 0, pointer, 1),
        (1, 0, 0, 0, -1, pointer, 1),
        (1, 0, 0, 0, 1, pointer, 1),
        (1, 0, 0, invalid_utf8.as_ptr() as u64, 1, pointer, 1),
        (
            1,
            0,
            0,
            text.as_ptr() as u64,
            text.len() as i64,
            pointer + 1,
            1,
        ),
    ] {
        assert_eq!(
            unsafe {
                prodex_optional_tool_version_policy_v1(
                    abi,
                    operation,
                    success,
                    address,
                    length,
                    minimum.as_ptr() as u64,
                    minimum.len() as i64,
                    target,
                )
            },
            expected
        );
        assert_eq!(output, 77);
    }
    let bad = [OptionalToolStringView {
        ptr: invalid_utf8.as_ptr() as u64,
        len: 1,
    }];
    assert_eq!(
        unsafe {
            prodex_optional_tool_newest_stable_version_v1(1, bad.as_ptr() as u64, 1, pointer)
        },
        1
    );
    assert_eq!(output, 77);
}
