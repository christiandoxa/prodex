use super::*;
#[cfg(unix)]
use crate::TestEnvVarGuard;
#[cfg(unix)]
use std::path::PathBuf;

#[test]
fn rtk_capability_probe_uses_version() {
    assert_eq!(command_capability_probe_args("rtk"), &["--version"]);
}

#[test]
fn capability_tui_text_contains_panels() {
    let panels = vec![CapabilityPanel {
        title: "Capabilities".to_string(),
        fields: vec![("rtk".to_string(), "ok (ready)".to_string())],
    }];
    let text = format!("{:?}", capability_tui_text(&panels));
    assert!(text.contains("Capabilities"));
    assert!(text.contains("rtk"));
    assert!(text.contains("ok"));
}

#[test]
fn capability_tui_text_does_not_pad_between_panels() {
    let panels = vec![
        CapabilityPanel {
            title: "One".to_string(),
            fields: vec![("rtk".to_string(), "ok".to_string())],
        },
        CapabilityPanel {
            title: "Two".to_string(),
            fields: vec![("codex".to_string(), "ok".to_string())],
        },
    ];

    let lines = capability_tui_text(&panels).lines;
    assert_eq!(lines.len(), 4);
    assert!(format!("{:?}", lines[2]).contains("Two"));
}

#[test]
fn capability_value_color_highlights_status() {
    assert_eq!(capability_value_color("fail (missing)"), Color::Red);
    assert_eq!(capability_value_color("disabled (not checked)"), Color::Red);
    assert_eq!(capability_value_color("ok (built-in)"), Color::Green);
}

#[test]
fn capability_detail_redacts_secret_like_material() {
    let detail = capability_redacted_detail(
        "failed: Authorization: Bearer fixture-token-123 url=https://example.test?api_key=sk-fixture-123",
    );

    assert!(detail.contains("Authorization: Bearer <redacted>"));
    assert!(detail.contains("api_key=<redacted>"));
    assert!(!detail.contains("fixture-token-123"));
    assert!(!detail.contains("sk-fixture-123"));
}

#[test]
fn capability_detail_removes_terminal_control_characters() {
    assert_eq!(
        capability_redacted_detail("ok\n\u{1b}[31mred"),
        "ok  [31mred"
    );
}

#[test]
fn capability_failed_status_redacts_secret_like_chain() {
    let err = anyhow::anyhow!("failed: Authorization: Bearer capability-token")
        .context("capability check failed");

    let status = capability_failed_status(&err);

    assert!(status.starts_with("fail ("));
    assert!(status.contains("capability check failed"));
    assert!(status.contains("Authorization: Bearer <redacted>"));
    assert!(!status.contains("capability-token"));
}

#[test]
fn capabilities_include_super_mcp_defaults() {
    let capabilities = collect_capabilities();
    assert!(
        capabilities
            .iter()
            .any(|capability| capability.name == "playwright-mcp")
    );
}

#[cfg(unix)]
#[test]
fn capabilities_use_runtime_binary_overrides_and_include_antigravity() {
    let _codex = TestEnvVarGuard::set("PRODEX_CODEX_BIN", "/bin/true");
    let _claude = TestEnvVarGuard::set("PRODEX_CLAUDE_BIN", "/bin/true");
    let _gemini = TestEnvVarGuard::set("PRODEX_GEMINI_BIN", "/bin/true");
    let _copilot = TestEnvVarGuard::set("PRODEX_COPILOT_BIN", "/bin/true");
    let _kiro = TestEnvVarGuard::set("PRODEX_KIRO_BIN", "/bin/true");
    let _agy = TestEnvVarGuard::set("PRODEX_AGY_BIN", "/bin/true");

    let capabilities = collect_capabilities();
    for name in [
        "codex",
        "claude",
        "gemini",
        "copilot",
        "kiro",
        "antigravity",
    ] {
        let capability = capabilities.iter().find(|item| item.name == name).unwrap();
        assert_eq!(capability.command.as_deref(), Some("/bin/true"));
        assert_eq!(capability.status, "available");
    }
}

#[cfg(unix)]
#[test]
fn setup_dry_run_uses_passive_binary_discovery() {
    let _codex = TestEnvVarGuard::set("PRODEX_CODEX_BIN", "/bin/true");
    let paths = AppPaths {
        root: PathBuf::from("/tmp/prodex-dry-run-test"),
        state_file: PathBuf::from("/tmp/prodex-dry-run-test/state.json"),
        managed_profiles_root: PathBuf::from("/tmp/prodex-dry-run-test/profiles"),
        shared_codex_root: PathBuf::from("/tmp/prodex-dry-run-test/shared-codex"),
        legacy_shared_codex_root: PathBuf::from("/tmp/prodex-dry-run-test/shared"),
    };

    let rows = collect_install_check_rows_passive(&paths);

    assert!(
        rows.iter()
            .any(|(name, status)| { name == "Codex CLI" && status.starts_with("available (") })
    );
    assert!(
        rows.iter()
            .any(|(name, status)| { name == "Codex auth" && status == "not checked (dry-run)" })
    );
}

#[test]
fn setup_optional_tool_verification_includes_non_caveman_catalog_entries() {
    let ids = prodex_optional_tools::OptionalToolSet::super_defaults()
        .iter()
        .collect::<Vec<_>>();

    assert!(ids.contains(&prodex_optional_tools::OptionalToolId::Caveman));
    assert!(ids.contains(&prodex_optional_tools::OptionalToolId::Rtk));
    assert!(!ids.contains(&prodex_optional_tools::OptionalToolId::PlaywrightMcp));

    let caveman = prodex_optional_tools::ToolHealth {
        id: prodex_optional_tools::OptionalToolId::Caveman,
        status: prodex_optional_tools::ToolHealthStatus::Installed,
        source: None,
        path: None,
        version: None,
        digest: None,
        can_activate: true,
        detail: "installed and validated".to_string(),
    };
    let report = setup_optional_tool_verification_json(Some(&caveman), None, true, true);
    assert_eq!(report["id"], "caveman");
    assert_eq!(report["status"], "installed");
    assert_eq!(report["ready"], serde_json::Value::Null);
    let report_ids = report["tools"]
        .as_array()
        .expect("optional-tool report should contain tools")
        .iter()
        .filter_map(|tool| tool["id"].as_str())
        .collect::<Vec<_>>();
    assert!(report_ids.contains(&"rtk"));
    assert!(!report_ids.contains(&"playwright-mcp"));
    assert!(
        report["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["status"] == "not checked (dry-run)")
    );
}

#[test]
fn setup_optional_tool_verification_reports_dry_run_without_a_probe() {
    let report = setup_optional_tool_verification_json(None, None, true, true);

    assert_eq!(report["status"], "not checked (dry-run)");
    assert_eq!(report["detail"], "verification skipped during dry-run");
}

#[test]
fn setup_optional_tool_verification_exit_checks_non_caveman_tools() {
    let missing_rtk = prodex_optional_tools::ToolHealth {
        id: prodex_optional_tools::OptionalToolId::Rtk,
        status: prodex_optional_tools::ToolHealthStatus::Missing,
        source: None,
        path: None,
        version: None,
        digest: None,
        can_activate: false,
        detail: "rtk was not found".to_string(),
    };

    let error = ensure_optional_tools_installed(&[missing_rtk])
        .expect_err("missing non-Caveman tools must fail verification");
    assert!(error.to_string().contains("rtk"));
}
