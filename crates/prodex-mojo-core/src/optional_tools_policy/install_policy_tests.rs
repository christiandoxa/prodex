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
fn manifest_content_preserves_release_precedence_and_compatibility_rules() {
    let latest = OptionalToolReleaseIdentity {
        version: "9.0.0",
        commit: "latest-commit",
        tree_sha256: "latest-tree",
    };
    let previous = OptionalToolReleaseIdentity {
        version: "8.0.0",
        commit: "previous-commit",
        tree_sha256: "previous-tree",
    };
    let legacy = OptionalToolReleaseIdentity {
        version: "7.0.0",
        commit: "legacy-commit",
        tree_sha256: "legacy-tree",
    };
    for (kind, name, legacy_identity) in [
        (OptionalToolPolicyId::Caveman, "Caveman", Some(legacy)),
        (OptionalToolPolicyId::Ponytail, "Ponytail", None),
    ] {
        let input = OptionalToolManifestContent {
            version: latest.version,
            commit: latest.commit,
            tree_sha256: latest.tree_sha256,
            actual_tree_sha256: latest.tree_sha256,
            latest,
            previous,
            legacy: legacy_identity,
            legacy_manifest_tree_sha256: "vetted-alias",
        };
        assert_eq!(optional_tool_manifest_content_error(kind, input), Ok(None));
        for (invalid, expected) in [
            (
                OptionalToolManifestContent {
                    commit: "changed",
                    tree_sha256: "changed",
                    actual_tree_sha256: "changed",
                    ..input
                },
                format!("{name} latest-stable commit does not match release-qualified metadata"),
            ),
            (
                OptionalToolManifestContent {
                    tree_sha256: "changed",
                    actual_tree_sha256: "changed",
                    ..input
                },
                format!(
                    "{name} latest-stable manifest tree digest does not match release-qualified metadata"
                ),
            ),
            (
                OptionalToolManifestContent {
                    actual_tree_sha256: "changed",
                    ..input
                },
                format!("{name} tree digest mismatch: expected latest-tree, got changed"),
            ),
            (
                OptionalToolManifestContent {
                    version: "10.0.0",
                    actual_tree_sha256: "changed",
                    ..input
                },
                format!("{name} tree digest mismatch: manifest latest-tree, got changed"),
            ),
        ] {
            assert_eq!(
                optional_tool_manifest_content_error(kind, invalid),
                Ok(Some(expected))
            );
        }
        assert_eq!(
            optional_tool_manifest_content_error(
                kind,
                OptionalToolManifestContent {
                    version: "10.0.0",
                    commit: "new-commit",
                    ..input
                }
            ),
            Ok(None)
        );
        let prior = OptionalToolManifestContent {
            version: previous.version,
            commit: previous.commit,
            tree_sha256: previous.tree_sha256,
            actual_tree_sha256: previous.tree_sha256,
            ..input
        };
        assert_eq!(optional_tool_manifest_content_error(kind, prior), Ok(None));
        let previous_error = if kind == OptionalToolPolicyId::Caveman {
            "previously release-qualified Caveman 3.2.0 identity or content changed"
        } else {
            "Ponytail previous-stable metadata does not match release-qualified compatibility metadata"
        };
        for invalid in [
            OptionalToolManifestContent {
                commit: "changed",
                ..prior
            },
            OptionalToolManifestContent {
                tree_sha256: "changed",
                actual_tree_sha256: "changed",
                ..prior
            },
            OptionalToolManifestContent {
                actual_tree_sha256: "changed",
                ..prior
            },
        ] {
            assert_eq!(
                optional_tool_manifest_content_error(kind, invalid),
                Ok(Some(previous_error.to_string()))
            );
        }
        let expected_alias = if kind == OptionalToolPolicyId::Caveman {
            Some(previous_error.to_string())
        } else {
            None
        };
        assert_eq!(
            optional_tool_manifest_content_error(
                kind,
                OptionalToolManifestContent {
                    tree_sha256: "vetted-alias",
                    ..prior
                }
            ),
            Ok(expected_alias)
        );
        if kind == OptionalToolPolicyId::Caveman {
            let old = OptionalToolManifestContent {
                version: legacy.version,
                commit: legacy.commit,
                tree_sha256: "vetted-alias",
                actual_tree_sha256: legacy.tree_sha256,
                ..input
            };
            assert_eq!(optional_tool_manifest_content_error(kind, old), Ok(None));
            assert_eq!(
                optional_tool_manifest_content_error(
                    kind,
                    OptionalToolManifestContent {
                        tree_sha256: legacy.tree_sha256,
                        ..old
                    }
                ),
                Ok(None)
            );
            // A failed Caveman legacy exception still allows ordinary digest equality.
            assert_eq!(
                optional_tool_manifest_content_error(
                    kind,
                    OptionalToolManifestContent {
                        commit: "changed",
                        tree_sha256: "self-consistent",
                        actual_tree_sha256: "self-consistent",
                        ..old
                    }
                ),
                Ok(None)
            );
            assert_eq!(
                optional_tool_manifest_content_error(
                    kind,
                    OptionalToolManifestContent {
                        commit: "changed",
                        ..old
                    }
                ),
                Ok(Some(
                    "Caveman tree digest mismatch: manifest vetted-alias, got legacy-tree"
                        .to_string()
                ))
            );
        }
    }
}

#[test]
fn manifest_content_abi_rejects_reserved_views_and_wrong_tags() {
    let mut fields = [OptionalToolStringView { ptr: 0, len: 0 }; 14];
    let nonempty = "unexpected";
    fields[10] = OptionalToolStringView {
        ptr: nonempty.as_ptr() as u64,
        len: nonempty.len() as u64,
    };
    let mut output = [77_u8; 32];
    let mut written = 99_i64;
    for (tool, tag, count) in [(4, 0, 14), (0, 1, 14), (0, 0, 13)] {
        assert_eq!(
            unsafe {
                prodex_optional_tool_install_policy_v1(
                    1,
                    tool,
                    3,
                    tag,
                    fields.as_ptr() as u64,
                    count,
                    0,
                    output.as_mut_ptr() as u64,
                    output.len() as i64,
                    &mut written as *mut i64 as u64,
                )
            },
            1
        );
        assert_eq!(output, [77; 32]);
        assert_eq!(written, 99);
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
