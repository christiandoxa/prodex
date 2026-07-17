use super::*;
#[cfg(unix)]
use crate::TestEnvVarGuard;

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
fn super_status_managed_optimizer_candidates_match_overlay_layouts() {
    let root = PathBuf::from("/tmp/prodex-optimizers");
    let candidates =
        managed_optimizer_command_candidates_for_super_status(&root, "codebase-memory-mcp");
    assert!(candidates.contains(&root.join("codebase-memory-mcp/build/c/codebase-memory-mcp")));
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
