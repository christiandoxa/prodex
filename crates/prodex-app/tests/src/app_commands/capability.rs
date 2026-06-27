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
fn capability_value_color_highlights_status() {
    assert_eq!(capability_value_color("fail (missing)"), Color::Red);
    assert_eq!(capability_value_color("disabled (not checked)"), Color::Red);
    assert_eq!(capability_value_color("ok (built-in)"), Color::Green);
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
fn super_status_token_savior_readiness_detects_venv_layout() {
    let path = PathBuf::from("/tmp/prodex-optimizers/token-savior/.venv/bin/token-savior");
    assert_eq!(
        python_venv_root_for_super_status_command(&path),
        Some(Path::new("/tmp/prodex-optimizers/token-savior/.venv"))
    );
    assert!(!python_venv_has_module_for_super_status(
        Path::new("/tmp/prodex-missing-venv"),
        "mcp"
    ));
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
