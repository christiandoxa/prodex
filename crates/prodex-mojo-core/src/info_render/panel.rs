//! Typed compiled value-color and field-visibility decisions for prodex info.
use super::{MojoError, render};

const INFO_PANEL_VALUE_COLOR: i64 = 35;
const INFO_PANEL_FIELD_VISIBLE: i64 = 36;

/// Semantic color selected by the compiled main-info panel classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoPanelValueColor {
    Reset,
    Red,
    Green,
    Cyan,
}

/// Classify a value while leaving terminal color materialization to the host.
pub fn info_panel_value_color(label: &str, value: &str) -> Result<InfoPanelValueColor, MojoError> {
    match render(INFO_PANEL_VALUE_COLOR, &[], &[], &[label, value], 0)?.as_str() {
        "0" => Ok(InfoPanelValueColor::Reset),
        "1" => Ok(InfoPanelValueColor::Red),
        "2" => Ok(InfoPanelValueColor::Green),
        "3" => Ok(InfoPanelValueColor::Cyan),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// A zero 5h quota source hides precisely the two five-hour pool/runway
/// rows. No Rust fallback recomputes the exact field-name policy.
pub fn info_panel_field_visible(
    label: &str,
    five_hour_profiles_with_data: usize,
) -> Result<bool, MojoError> {
    let count = u64::try_from(five_hour_profiles_with_data).map_err(|_| MojoError::InvalidInput)?;
    let text = render(INFO_PANEL_FIELD_VISIBLE, &[], &[count], &[label], 0)?;
    match text.as_str() {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use InfoPanelValueColor::{Cyan, Green, Red, Reset};

    #[test]
    fn compiled_panel_value_classifier_preserves_status_priority_and_ascii_case() {
        for (label, value, expected) in [
            ("x", "missing file", Red),
            ("x", "critical load", Red),
            ("x", "READY but WARNING", Red),
            ("x", "healthy", Green),
            ("x", "up to date", Green),
            ("Active profile", "main", Green),
            ("Codex quota data", "unavailable=0", Green),
            ("Codex quota data", "unavailable=1", Red),
            ("Other Codex quota data", "unavailable=0", Red),
            ("Runtime limits", "unmatched", Cyan),
            ("Codex config", "unmatched", Cyan),
            ("Other", "unmatched", Reset),
            ("Other", "状態正常", Reset),
            ("Codex quota data", "unavailable=0 but CRITICAL", Red),
            ("Codex quota data", "unavailable=01", Green),
            ("codex quota data", "unavailable=0", Red),
            ("active profile", "unmatched", Reset),
            ("Other", "nothing", Red),
            ("Other", "READY\0WARNING", Red),
        ] {
            assert_eq!(
                info_panel_value_color(label, value),
                Ok(expected),
                "{label}: {value}"
            );
        }
    }

    #[test]
    fn compiled_panel_visibility_targets_only_exact_five_hour_rows() {
        for (label, count, visible) in [
            ("Codex 5h pool", 0, false),
            ("Codex 5h runway", 0, false),
            ("Codex weekly pool", 0, true),
            ("Other Codex 5h pool", 0, true),
            ("codex 5h pool", 0, true),
            ("Codex 5h pool ", 0, true),
            ("Codex 5h pool", 1, true),
            ("Codex 5h runway", usize::MAX, true),
        ] {
            assert_eq!(info_panel_field_visible(label, count), Ok(visible));
        }
    }
}
