use super::*;

#[test]
fn claw_compactor_capability_probe_uses_help() {
    assert_eq!(command_capability_probe_args("claw-compactor"), &["--help"]);
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
    let candidates = managed_optimizer_command_candidates_for_super_status(&root, "sqz-mcp");
    assert!(candidates.contains(&root.join("sqz/target/release/sqz-mcp")));

    let candidates = managed_optimizer_command_candidates_for_super_status(&root, "token-savior");
    assert!(candidates.contains(&root.join("token-savior/.venv/bin/token-savior")));

    let candidates = managed_optimizer_command_candidates_for_super_status(&root, "claw-compactor");
    assert!(candidates.contains(&root.join("claw-compactor/.venv/bin/claw-compactor")));
}

#[test]
fn super_status_reports_selected_mem0_backend() {
    let paths = AppPaths::discover().expect("paths");
    let status = memory_tool_status(
        &paths,
        SuperMemoryStatusMode::Selected(crate::SuperMemoryBackend::Mem0),
    );
    assert_eq!(status.name, "prodex-memory");
    assert!(status.status.contains("managed Mem0 selected"));
    assert!(status.detail.contains("managed Mem0 env"));
}

#[test]
fn super_status_reports_memory_disabled_for_default_launch() {
    let paths = AppPaths::discover().expect("paths");
    let status = memory_tool_status(&paths, SuperMemoryStatusMode::Disabled);
    assert_eq!(status.name, "prodex-memory");
    assert_eq!(status.status, "disabled");
    assert!(status.detail.contains("enabled only"));
}
