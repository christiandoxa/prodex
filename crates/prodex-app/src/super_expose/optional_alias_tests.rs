use super::*;

#[test]
fn compiled_optional_alias_plans_preserve_executables_prefixes_and_playwright_spelling() {
    let cases: &[(&str, OptionalToolId, &str, &[&str])] = &[
        (
            "optional:rtk",
            OptionalToolId::Rtk,
            "/synthetic/native",
            &[],
        ),
        (
            "optional: RTK ",
            OptionalToolId::Rtk,
            "/synthetic/native",
            &[],
        ),
        (
            "optional:cbm",
            OptionalToolId::CodebaseMemoryMcp,
            "/synthetic/native",
            &[],
        ),
        (
            "optional:playwright-mcp",
            OptionalToolId::PlaywrightMcp,
            "/synthetic/native",
            &["--no-install", "@playwright/mcp"],
        ),
        (
            "optional:PLAYWRIGHT",
            OptionalToolId::PlaywrightMcp,
            "/synthetic/native",
            &["--no-install", "@playwright/mcp"],
        ),
        (
            "optional: playwright ",
            OptionalToolId::PlaywrightMcp,
            "/synthetic/native",
            &["--no-install", "@playwright/mcp"],
        ),
        (
            "optional:playwright",
            OptionalToolId::PlaywrightMcp,
            "/synthetic/prodex",
            &[
                "super",
                "--no-sub-agent",
                "--no-presidio",
                "--tool",
                "playwright",
            ],
        ),
        (
            "optional:ponytail",
            OptionalToolId::Ponytail,
            "/synthetic/prodex",
            &[
                "super",
                "--no-sub-agent",
                "--no-presidio",
                "--tool",
                "ponytail",
            ],
        ),
        (
            "optional:presidio",
            OptionalToolId::Presidio,
            "/synthetic/prodex",
            &["super", "--no-sub-agent", "--presidio"],
        ),
    ];
    for &(alias, id, executable, prefix) in cases {
        let context = ExposeOptionalTools::with_tool_for_tests(
            id,
            Some(PathBuf::from("/synthetic/native")),
            PathBuf::from("/synthetic/prodex"),
        );
        let tail = vec![
            OsString::from("argument one"),
            OsString::from("\"quoted 🧭\""),
        ];
        let result = context.resolve_exec(alias, tail.clone()).unwrap();
        assert_eq!(result.program, PathBuf::from(executable), "{alias}");
        let mut expected = prefix.iter().map(OsString::from).collect::<Vec<_>>();
        expected.extend(tail);
        assert_eq!(result.args, expected, "{alias}");
        assert_eq!(result.optional_tool, Some(id), "{alias}");
    }
}

#[test]
fn compiled_optional_alias_preserves_unknown_unavailable_and_missing_path_errors() {
    let context = ExposeOptionalTools::empty_for_tests();
    assert_eq!(
        context
            .resolve_exec("optional:caveman", vec![])
            .unwrap_err(),
        "unknown optional-tool exec alias: caveman"
    );
    assert_eq!(
        context
            .resolve_exec("optional:unknown", vec![])
            .unwrap_err(),
        "unknown optional-tool exec alias: unknown"
    );
    assert_eq!(
        context.resolve_exec("optional:rtk", vec![]).unwrap_err(),
        "optional tool rtk is unavailable: unavailable in test fixture"
    );
    let native = ExposeOptionalTools::with_tool_for_tests(
        OptionalToolId::Rtk,
        None,
        PathBuf::from("/synthetic/prodex"),
    );
    assert_eq!(
        native.resolve_exec("optional:rtk", vec![]).unwrap_err(),
        "validated optional tool rtk has no executable path"
    );
    let browser = ExposeOptionalTools::with_tool_for_tests(
        OptionalToolId::PlaywrightMcp,
        None,
        PathBuf::from("/synthetic/prodex"),
    );
    assert_eq!(
        browser
            .resolve_exec("optional:playwright-mcp", vec![])
            .unwrap_err(),
        "validated Playwright MCP has no npx executable path"
    );
    assert_eq!(
        browser
            .resolve_exec("optional:playwright", vec![])
            .unwrap()
            .program,
        PathBuf::from("/synthetic/prodex")
    );
}

#[test]
fn compiled_optional_alias_boundary_preserves_plain_os_programs() {
    let context = ExposeOptionalTools::empty_for_tests();
    for program in ["/synthetic/🧭/program", "OPTIONAL:rtk", " optional:rtk"] {
        let args = vec![OsString::from("kept")];
        let result = context.resolve_exec(program, args.clone()).unwrap();
        assert_eq!(result.program, PathBuf::from(program));
        assert_eq!(result.args, args);
        assert_eq!(result.optional_tool, None);
    }
}

#[cfg(unix)]
#[test]
fn compiled_optional_alias_keeps_non_utf8_os_argument_bytes() {
    use std::os::unix::ffi::OsStringExt;
    let context = ExposeOptionalTools::with_tool_for_tests(
        OptionalToolId::Ponytail,
        None,
        PathBuf::from("/synthetic/prodex"),
    );
    let argument = OsString::from_vec(vec![255, b'-', b'x']);
    let result = context
        .resolve_exec("optional:ponytail", vec![argument.clone()])
        .unwrap();
    assert_eq!(result.args.last(), Some(&argument));
}
