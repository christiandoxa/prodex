//! JSON body extraction and replacement helpers for Presidio redaction.

pub(super) fn collect_json_string_values(
    value: &serde_json::Value,
    redact_string: bool,
    values: &mut Vec<String>,
) {
    match value {
        serde_json::Value::String(text) if redact_string => values.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_json_string_values(item, redact_string, values);
            }
        }
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                collect_json_string_values(value, should_redact_json_string_field(key), values);
            }
        }
        _ => {}
    }
}

pub(super) fn replace_json_string_values<'a>(
    value: &mut serde_json::Value,
    redact_string: bool,
    values: &mut impl Iterator<Item = &'a str>,
) {
    match value {
        serde_json::Value::String(text) if redact_string => {
            if let Some(redacted) = values.next() {
                *text = redacted.to_string();
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                replace_json_string_values(item, redact_string, values);
            }
        }
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                replace_json_string_values(value, should_redact_json_string_field(key), values);
            }
        }
        _ => {}
    }
}

fn should_redact_json_string_field(key: &str) -> bool {
    matches!(
        key,
        "arguments" | "content" | "input" | "instructions" | "output" | "text"
    )
}

pub(super) fn presidio_json_value_separator(values: &[String]) -> String {
    let mut separator = "\u{e000}PRODEX_PRESIDIO_VALUE\u{e001}".to_string();
    while values.iter().any(|value| value.contains(&separator)) {
        separator.push('\u{e002}');
    }
    separator
}
