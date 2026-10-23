use super::*;

#[test]
fn manifest_header_and_version_diagnostics_preserve_both_tool_contracts() {
    for (kind, name, id) in [
        (OptionalToolPolicyId::Caveman, "Caveman", "caveman"),
        (OptionalToolPolicyId::Ponytail, "Ponytail", "ponytail"),
    ] {
        assert_eq!(optional_tool_manifest_header_error(kind, 1, id), Ok(None));
        assert_eq!(
            optional_tool_manifest_header_error(kind, 0, "wrong"),
            Ok(Some(format!("unsupported {name} manifest schema")))
        );
        assert_eq!(
            optional_tool_manifest_header_error(kind, 1, "wrong"),
            Ok(Some(format!("{name} manifest id must be {id}")))
        );
        assert_eq!(
            optional_tool_manifest_version_error(
                kind,
                OptionalToolStableVersion::Supported,
                "9.0.0",
                "1.0.0",
                "2.0.0"
            ),
            Ok(None)
        );
        assert_eq!(
            optional_tool_manifest_version_error(
                kind,
                OptionalToolStableVersion::Malformed,
                "v9.0.0",
                "1.0.0",
                "2.0.0"
            ),
            Ok(Some(format!("invalid {name} version v9.0.0")))
        );
        assert_eq!(
            optional_tool_manifest_version_error(
                kind,
                OptionalToolStableVersion::Incompatible,
                "0.9.0",
                "1.0.0",
                "2.0.0"
            ),
            Ok(Some(format!(
                "{name} 0.9.0 is incompatible; Prodex requires 1.0.0 or newer. Update {name} to the latest stable release (release-qualified reference: 2.0.0)"
            )))
        );
    }
}

#[test]
fn manifest_identity_preserves_first_issue_and_ascii_digest_rules() {
    let commit = "a".repeat(40);
    let digest = "F".repeat(64);
    let input = OptionalToolManifestIdentity {
        version: "9.0.0",
        directory_version: Some("9.0.0"),
        source: "https://example.com/source",
        expected_source: "https://example.com/source",
        commit: &commit,
        tree_sha256: &digest,
    };
    for (kind, name) in [
        (OptionalToolPolicyId::Caveman, "Caveman"),
        (OptionalToolPolicyId::Ponytail, "Ponytail"),
    ] {
        assert_eq!(optional_tool_manifest_identity_error(kind, input), Ok(None));
        for (invalid, expected) in [
            (
                OptionalToolManifestIdentity {
                    directory_version: None,
                    source: "wrong",
                    commit: "bad",
                    ..input
                },
                format!("{name} version directory does not match manifest version 9.0.0"),
            ),
            (
                OptionalToolManifestIdentity {
                    directory_version: Some("8.0.0"),
                    ..input
                },
                format!("{name} version directory does not match manifest version 9.0.0"),
            ),
            (
                OptionalToolManifestIdentity {
                    source: "wrong",
                    commit: "bad",
                    ..input
                },
                format!("unexpected {name} source"),
            ),
            (
                OptionalToolManifestIdentity {
                    commit: "bad",
                    tree_sha256: "bad",
                    ..input
                },
                format!("{name} manifest commit must be a 40-character Git SHA"),
            ),
            (
                OptionalToolManifestIdentity {
                    tree_sha256: "bad",
                    ..input
                },
                format!("{name} manifest tree digest must be SHA-256"),
            ),
        ] {
            assert_eq!(
                optional_tool_manifest_identity_error(kind, invalid),
                Ok(Some(expected))
            );
        }
        for invalid in [
            "0".repeat(39),
            "0".repeat(41),
            "g".repeat(40),
            "é".repeat(20),
        ] {
            assert_eq!(
                optional_tool_manifest_identity_error(
                    kind,
                    OptionalToolManifestIdentity {
                        commit: &invalid,
                        ..input
                    }
                ),
                Ok(Some(format!(
                    "{name} manifest commit must be a 40-character Git SHA"
                )))
            );
        }
    }
}

#[test]
fn manifest_abi_rejects_invalid_headers_and_views_without_output_writes() {
    let id = "caveman";
    let mut fields = [OptionalToolStringView { ptr: 0, len: 0 }; 6];
    fields[0] = OptionalToolStringView {
        ptr: id.as_ptr() as u64,
        len: id.len() as u64,
    };
    let mut output = [77_u8; 32];
    let mut written = 99_i64;
    for (abi, tool, phase, tag, count, measuring, expected) in [
        (2, 0, 0, 1, 6, 0, 4),
        (1, 2, 0, 1, 6, 0, 1),
        (1, 0, 3, 1, 6, 0, 1),
        (1, 0, 1, 3, 6, 0, 1),
        (1, 0, 2, 2, 6, 0, 1),
        (1, 0, 0, 1, 5, 0, 1),
        (1, 0, 0, 1, 6, 2, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_optional_tool_install_policy_v1(
                    abi,
                    tool,
                    phase,
                    tag,
                    fields.as_ptr() as u64,
                    count,
                    measuring,
                    output.as_mut_ptr() as u64,
                    output.len() as i64,
                    &mut written as *mut i64 as u64,
                )
            },
            expected
        );
        assert_eq!(output, [77; 32]);
        assert_eq!(written, 99);
    }
    let invalid = [0xff_u8];
    fields[0] = OptionalToolStringView {
        ptr: invalid.as_ptr() as u64,
        len: 1,
    };
    assert_eq!(
        unsafe {
            prodex_optional_tool_install_policy_v1(
                1,
                0,
                0,
                1,
                fields.as_ptr() as u64,
                6,
                0,
                output.as_mut_ptr() as u64,
                output.len() as i64,
                &mut written as *mut i64 as u64,
            )
        },
        1
    );
    assert_eq!(written, 99);
}
