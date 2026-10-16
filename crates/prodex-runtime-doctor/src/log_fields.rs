use std::collections::BTreeMap;

pub use runtime_proxy_crate::runtime_proxy_log_fields;

pub(crate) fn runtime_doctor_log_message_has_marker(message: &str, marker: &str) -> bool {
    prodex_mojo_core::rich::runtime_doctor_log_message_has_marker(message, marker)
        .expect("Mojo runtime-doctor log marker matching returned invalid output")
}

pub(crate) fn runtime_doctor_string_field(
    fields: &BTreeMap<String, String>,
    key: &str,
) -> Option<String> {
    fields
        .get(key)
        .filter(|value| !runtime_doctor_ignored_log_value(value))
        .cloned()
}

pub(crate) fn runtime_doctor_parse_usize_field(
    fields: &BTreeMap<String, String>,
    key: &str,
) -> Option<usize> {
    let value = fields.get(key).map(String::as_str);
    prodex_mojo_core::rich::runtime_doctor_unsigned_field(value)
        .expect("Mojo runtime-doctor unsigned log field returned invalid output")
}

pub(crate) fn runtime_doctor_split_reason_field(
    fields: &BTreeMap<String, String>,
    key: &str,
) -> Vec<String> {
    fields
        .get(key)
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !runtime_doctor_ignored_log_value(value))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn runtime_doctor_ignored_log_value(value: &str) -> bool {
    prodex_mojo_core::rich::runtime_doctor_log_value_is_ignored(value)
        .expect("Mojo runtime-doctor log-value policy returned invalid output")
}

#[cfg(test)]
mod marker_boundary_tests {
    use super::runtime_doctor_log_message_has_marker;

    #[test]
    fn matches_exact_unicode_whitespace_delimited_marker_after_punctuation() {
        for message in [
            "[2026-10-16] smart_context_autopilot request=1",
            "alpha\u{a0}(smart_context_autopilot);\u{3000}beta",
            "`\"smart_context_autopilot\"`",
            "smart_context_autopilot",
        ] {
            assert!(
                runtime_doctor_log_message_has_marker(message, "smart_context_autopilot"),
                "{message}"
            );
        }
        for message in [
            "smart_context_autopilot_extra",
            "prefixsmart_context_autopilot",
            "smart_context_autopilot=1",
            "alpha\u{a0}smart_context_autopilotz",
            "",
        ] {
            assert!(
                !runtime_doctor_log_message_has_marker(message, "smart_context_autopilot"),
                "{message}"
            );
        }
        assert!(runtime_doctor_log_message_has_marker("...", ""));
        assert!(!runtime_doctor_log_message_has_marker("", ""));
    }
}
