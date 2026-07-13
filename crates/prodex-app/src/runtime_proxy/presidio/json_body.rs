//! Bounded schema-aware JSON content walking for Presidio inspection.

use prodex_domain::InspectionCoverage;
use std::error::Error;
use std::fmt;

pub(super) const MAX_PRESIDIO_JSON_DEPTH: usize = 32;
pub(super) const MAX_PRESIDIO_JSON_VALUES: usize = 256;
pub(super) const MAX_PRESIDIO_JSON_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PresidioJsonString {
    pub path: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PresidioJsonContent {
    pub values: Vec<PresidioJsonString>,
    pub coverage: InspectionCoverage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PresidioJsonContentError {
    TooDeep,
    TooManyValues,
    TextBudgetExhausted,
}

impl fmt::Display for PresidioJsonContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "request content exceeds inspection limits")
    }
}

impl Error for PresidioJsonContentError {}

pub(super) fn collect_json_content(
    value: &serde_json::Value,
) -> Result<PresidioJsonContent, PresidioJsonContentError> {
    let mut state = PresidioJsonWalkState::default();
    collect_json_content_at(value, false, "$", 0, &mut state)?;
    let coverage = if state.values.is_empty() {
        InspectionCoverage::Unsupported
    } else if state.unsupported_modality {
        InspectionCoverage::Partial
    } else {
        InspectionCoverage::Full
    };
    Ok(PresidioJsonContent {
        values: state.values,
        coverage,
    })
}

#[derive(Default)]
struct PresidioJsonWalkState {
    values: Vec<PresidioJsonString>,
    total_text_bytes: usize,
    unsupported_modality: bool,
}

fn collect_json_content_at(
    value: &serde_json::Value,
    inspect_strings: bool,
    path: &str,
    depth: usize,
    state: &mut PresidioJsonWalkState,
) -> Result<(), PresidioJsonContentError> {
    if depth > MAX_PRESIDIO_JSON_DEPTH {
        return Err(PresidioJsonContentError::TooDeep);
    }
    match value {
        serde_json::Value::String(text) if inspect_strings => {
            if state.values.len() >= MAX_PRESIDIO_JSON_VALUES {
                return Err(PresidioJsonContentError::TooManyValues);
            }
            state.total_text_bytes = state
                .total_text_bytes
                .checked_add(text.len())
                .ok_or(PresidioJsonContentError::TextBudgetExhausted)?;
            if state.total_text_bytes > MAX_PRESIDIO_JSON_TEXT_BYTES {
                return Err(PresidioJsonContentError::TextBudgetExhausted);
            }
            state.values.push(PresidioJsonString {
                path: path.to_string(),
                text: text.clone(),
            });
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_json_content_at(
                    item,
                    inspect_strings,
                    &format!("{path}[{index}]"),
                    depth + 1,
                    state,
                )?;
            }
        }
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                if unsupported_modality_field(key) && !value.is_null() {
                    state.unsupported_modality = true;
                }
                let schema_field = inspectable_json_string_field(key);
                let inspect_child = inspect_strings || schema_field;
                let child_path = if schema_field {
                    format!("{path}.{key}")
                } else if inspect_strings {
                    format!("{path}.*")
                } else {
                    path.to_string()
                };
                collect_json_content_at(value, inspect_child, &child_path, depth + 1, state)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn inspectable_json_string_field(key: &str) -> bool {
    matches!(
        key,
        "arguments" | "content" | "input" | "instructions" | "output" | "prompt" | "text"
    )
}

fn unsupported_modality_field(key: &str) -> bool {
    matches!(
        key,
        "audio"
            | "file"
            | "image"
            | "image_url"
            | "input_audio"
            | "input_file"
            | "input_image"
            | "video"
    )
}

pub(super) fn replace_json_string_values<'a>(
    value: &mut serde_json::Value,
    inspect_strings: bool,
    values: &mut impl Iterator<Item = &'a str>,
) {
    match value {
        serde_json::Value::String(text) if inspect_strings => {
            if let Some(redacted) = values.next() {
                *text = redacted.to_string();
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                replace_json_string_values(item, inspect_strings, values);
            }
        }
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                replace_json_string_values(
                    value,
                    inspect_strings || inspectable_json_string_field(key),
                    values,
                );
            }
        }
        _ => {}
    }
}

pub(super) fn presidio_json_value_separator(values: &[PresidioJsonString]) -> String {
    let mut separator = "\u{e000}PRODEX_PRESIDIO_VALUE\u{e001}".to_string();
    while values.iter().any(|value| value.text.contains(&separator)) {
        separator.push('\u{e002}');
    }
    separator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walker_inspects_supported_nested_fields_without_retaining_argument_keys() {
        let value = serde_json::json!({
            "model": "example-model",
            "tools": [{
                "arguments": {
                    "customer_email": "user@example.com",
                    "nested": ["token-value"]
                }
            }]
        });

        let content = collect_json_content(&value).unwrap();

        assert_eq!(content.coverage, InspectionCoverage::Full);
        assert_eq!(content.values.len(), 2);
        assert!(content.values.iter().all(|value| value.path.contains(".*")));
        assert!(
            content
                .values
                .iter()
                .all(|value| !value.path.contains("customer_email"))
        );
    }

    #[test]
    fn walker_reports_unsupported_modalities_and_depth_limits() {
        let content = collect_json_content(&serde_json::json!({
            "input": "inspect me",
            "input_image": {"url": "https://example.com/synthetic.png"}
        }))
        .unwrap();
        assert_eq!(content.coverage, InspectionCoverage::Partial);

        let mut value = serde_json::json!("deep");
        for _ in 0..=MAX_PRESIDIO_JSON_DEPTH {
            value = serde_json::json!({"input": value});
        }
        assert_eq!(
            collect_json_content(&value).unwrap_err(),
            PresidioJsonContentError::TooDeep
        );
    }

    #[test]
    fn walker_bounds_value_count_and_total_text() {
        let values = vec![serde_json::json!({"text": "bounded"}); MAX_PRESIDIO_JSON_VALUES + 1];
        assert_eq!(
            collect_json_content(&serde_json::Value::Array(values)).unwrap_err(),
            PresidioJsonContentError::TooManyValues
        );

        let oversized = "x".repeat(MAX_PRESIDIO_JSON_TEXT_BYTES + 1);
        assert_eq!(
            collect_json_content(&serde_json::json!({"input": oversized})).unwrap_err(),
            PresidioJsonContentError::TextBudgetExhausted
        );
    }
}
