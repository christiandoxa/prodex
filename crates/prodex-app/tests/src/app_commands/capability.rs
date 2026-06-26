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
    assert_eq!(
        capability_value_color("disabled (not checked)"),
        Color::Yellow
    );
    assert_eq!(capability_value_color("ok (built-in)"), Color::Green);
}
