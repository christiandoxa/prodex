use super::{
    RuntimeLaunchTuiColor, RuntimeLaunchTuiLine, runtime_launch_tui_height,
    runtime_launch_tui_line_plan,
};

#[test]
fn launch_tui_bridge_preserves_precedence_unicode_slices_and_reset_foreground() {
    assert_eq!(
        runtime_launch_tui_line_plan("Proxy: disabled:").unwrap(),
        RuntimeLaunchTuiLine::Title
    );
    assert_eq!(
        runtime_launch_tui_line_plan("Ready").unwrap(),
        RuntimeLaunchTuiLine::Primary
    );
    for (input, label, value, color) in [
        (
            "Proxy: disabled enabled",
            "Proxy:",
            "disabled enabled",
            RuntimeLaunchTuiColor::Red,
        ),
        (
            "Binary: enabled",
            "Binary:",
            "enabled",
            RuntimeLaunchTuiColor::Green,
        ),
        (
            "Binary: opaque",
            "Binary:",
            "opaque",
            RuntimeLaunchTuiColor::Cyan,
        ),
        (
            "\u{2003}Label:\t 雪  ",
            "\u{2003}Label:",
            "雪  ",
            RuntimeLaunchTuiColor::Reset,
        ),
        (
            "Label:\u{001c}雪",
            "Label:",
            "\u{001c}雪",
            RuntimeLaunchTuiColor::Reset,
        ),
    ] {
        assert_eq!(
            runtime_launch_tui_line_plan(input).unwrap(),
            RuntimeLaunchTuiLine::Field {
                label,
                value,
                color
            }
        );
    }
}

#[test]
fn launch_tui_height_bridge_preserves_empty_crlf_trailing_newline_and_clamp_edges() {
    for (input, expected) in [("", 8), ("\n", 8), ("a\r\nb\r\nc\r\nd\r\n", 9)] {
        assert_eq!(runtime_launch_tui_height(input).unwrap(), expected);
    }
    for (rows, expected) in [(25, 30), (26, 31), (27, 32), (28, 32)] {
        assert_eq!(
            runtime_launch_tui_height(&"\n".repeat(rows)).unwrap(),
            expected
        );
    }
}
