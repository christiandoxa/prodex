use std::collections::BTreeMap;

pub use runtime_proxy_crate::runtime_proxy_log_fields;

pub(crate) fn runtime_doctor_log_message_has_marker(message: &str, marker: &str) -> bool {
    message.split_whitespace().any(|token| {
        token.trim_matches(|c: char| {
            matches!(
                c,
                '`' | '\'' | '"' | '[' | ']' | '(' | ')' | '{' | '}' | ',' | '.' | ';' | ':'
            )
        }) == marker
    })
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
    runtime_doctor_string_field(fields, key)?.parse().ok()
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
    value.is_empty() || value == "-"
}
