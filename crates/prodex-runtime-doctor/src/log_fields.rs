use std::collections::BTreeMap;

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

fn runtime_proxy_skip_log_whitespace(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_proxy_skip_log_field_value(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    if index >= bytes.len() {
        return index;
    }
    if bytes[index] == b'"' {
        index += 1;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if escaped {
                escaped = false;
                index += 1;
                continue;
            }
            match byte {
                b'\\' => {
                    escaped = true;
                    index += 1;
                }
                b'"' => {
                    index += 1;
                    break;
                }
                _ => index += 1,
            }
        }
        return index;
    }
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_proxy_parse_log_field_value(raw_value: &str) -> String {
    if raw_value.starts_with('"') {
        serde_json::from_str::<String>(raw_value)
            .unwrap_or_else(|_| raw_value.trim_matches('"').to_string())
    } else {
        raw_value.trim_matches('"').to_string()
    }
}

pub fn runtime_proxy_log_fields(message: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let bytes = message.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        index = runtime_proxy_skip_log_whitespace(message, index);
        if index >= bytes.len() {
            break;
        }

        let key_start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            continue;
        }

        let key = &message[key_start..index];
        index += 1;
        let value_start = index;
        let value_end = runtime_proxy_skip_log_field_value(message, index);
        index = value_end;
        let raw_value = &message[value_start..value_end];
        if key.is_empty() || raw_value.is_empty() {
            continue;
        }
        fields.insert(
            key.to_string(),
            runtime_proxy_parse_log_field_value(raw_value),
        );
    }
    fields
}
