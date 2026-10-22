use super::*;

#[test]
fn compiled_shape_diagnostics_preserve_all_exact_messages() {
    use OptionalToolConfigShapeIssue::*;
    for (issue, expected) in [
        (
            Features,
            "configuration entry `features` must be a TOML table",
        ),
        (
            Marketplaces,
            "configuration entry `marketplaces` must be a TOML table",
        ),
        (
            Plugins,
            "configuration entry `plugins` must be a TOML table",
        ),
        (
            PonytailMarketplace,
            "configuration entry `ponytail` must be a TOML table",
        ),
        (
            PonytailPlugin,
            "configuration entry `ponytail@ponytail` must be a TOML table",
        ),
        (McpServers, "mcp_servers must be a TOML table"),
        (
            CodebaseMemoryMcp,
            "mcp_servers.codebase-memory-mcp must be a TOML table",
        ),
        (Playwright, "mcp_servers.playwright must be a TOML table"),
    ] {
        assert_eq!(
            optional_tool_config_shape_diagnostic(issue).unwrap(),
            expected
        );
    }
}

#[test]
fn compiled_required_mcp_diagnostics_preserve_names_and_exact_messages() {
    for name in ["", "playwright", "fixture-東京\nname"] {
        for (issue, suffix) in [
            (
                RequiredMcpConfigIssue::Disabled,
                "is disabled (`enabled = false`); refusing to override user configuration",
            ),
            (
                RequiredMcpConfigIssue::EnabledNotBoolean,
                "cannot be safely verified: `enabled` must be a boolean",
            ),
            (
                RequiredMcpConfigIssue::CommandOrArgsMismatch,
                "cannot be safely verified: inherited configuration is custom or incomplete; refusing to replace user configuration",
            ),
        ] {
            assert_eq!(
                optional_tool_required_mcp_diagnostic(issue, name).unwrap(),
                format!("required MCP server `mcp_servers.{name}` {suffix}")
            );
        }
    }
}

#[test]
fn diagnostic_abi_rejects_bad_tags_names_capacity_and_alignment_without_partial_output() {
    let mut output = [73_u8; 512];
    let mut written = 83_i64;
    let result = &mut written as *mut i64 as u64;
    let mut call = |version, group, issue, name: &[u8], capacity, result| unsafe {
        prodex_optional_tool_diagnostic_v1(
            version,
            group,
            issue,
            name.as_ptr() as u64,
            name.len() as i64,
            output.as_mut_ptr() as u64,
            capacity,
            result,
        )
    };
    assert_eq!(call(2, 0, 1, b"", 512, result), 4);
    assert_eq!(call(1, -1, 1, b"", 512, result), 1);
    assert_eq!(call(1, 2, 1, b"", 512, result), 1);
    assert_eq!(call(1, 0, 0, b"", 512, result), 1);
    assert_eq!(call(1, 0, 9, b"", 512, result), 1);
    assert_eq!(call(1, 1, 4, b"", 512, result), 1);
    assert_eq!(call(1, 0, 1, b"unexpected", 512, result), 1);
    assert_eq!(call(1, 1, 1, &[0xff], 512, result), 1);
    assert_eq!(call(1, 0, 1, b"", -1, result), 1);
    assert_eq!(call(1, 0, 1, b"", 1, result), 3);
    assert_eq!(call(1, 0, 1, b"", 512, 0), 1);
    assert_eq!(call(1, 0, 1, b"", 512, result + 1), 1);
    assert_eq!(output, [73; 512]);
    assert_eq!(written, 83);
}
