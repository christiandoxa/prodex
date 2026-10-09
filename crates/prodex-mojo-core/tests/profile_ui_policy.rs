#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::profile_ui_policy::{
    ProfileValueColor, normalized_terminal_height, scroll_body_height, scroll_max_offset,
    tui_height, value_color,
};

#[test]
fn profile_geometry_preserves_terminal_and_scroll_contract() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    assert_eq!(normalized_terminal_height(0).unwrap(), 24);
    assert_eq!(normalized_terminal_height(10).unwrap(), 10);
    assert_eq!(scroll_body_height(10).unwrap(), 4);
    assert_eq!(scroll_body_height(3).unwrap(), 1);
    assert_eq!(scroll_max_offset(20, 4).unwrap(), 16);
    assert_eq!(scroll_max_offset(3, 4).unwrap(), 0);
    assert_eq!(tui_height(0, 10).unwrap(), 4);
    assert_eq!(tui_height(20, 10).unwrap(), 10);
    assert_eq!(tui_height(0, 0).unwrap(), 4);
    assert_eq!(tui_height(20, 0).unwrap(), 24);
    assert_eq!(tui_height(usize::MAX, 10).unwrap(), 10);
    assert_eq!(tui_height(i64::MAX as usize, 10).unwrap(), 10);
    assert_eq!(
        scroll_body_height(u16::MAX).unwrap(),
        usize::from(u16::MAX) - 6
    );
    assert_eq!(scroll_max_offset(0, 0).unwrap(), 0);
    assert_eq!(scroll_max_offset(15, 0).unwrap(), 14);
}

#[test]
fn profile_value_colors_keep_priority_and_case_contract() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    assert_eq!(
        value_color("Status", "No active profile."),
        Ok(ProfileValueColor::Red)
    );
    assert_eq!(
        value_color("Status", "missing home"),
        Ok(ProfileValueColor::Red)
    );
    assert_eq!(value_color("Active", "main"), Ok(ProfileValueColor::Green));
    assert_eq!(value_color("Status", "YES"), Ok(ProfileValueColor::Green));
    assert_eq!(
        value_color("Provider", "OpenAI"),
        Ok(ProfileValueColor::Cyan)
    );
    assert_eq!(
        value_color("provider", "OpenAI"),
        Ok(ProfileValueColor::Reset)
    );
    assert_eq!(value_color("Other", "value"), Ok(ProfileValueColor::Reset));
    assert_eq!(value_color("Active", "ERROR"), Ok(ProfileValueColor::Red));
    assert_eq!(
        value_color("Auth", "no active credentials"),
        Ok(ProfileValueColor::Red)
    );
    assert_eq!(
        value_color("Identity", "unknown"),
        Ok(ProfileValueColor::Cyan)
    );
    assert_eq!(value_color("other", "ACTIVE"), Ok(ProfileValueColor::Green));
}
