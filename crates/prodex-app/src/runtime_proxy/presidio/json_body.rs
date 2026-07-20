//! Bounded schema-aware JSON content walking for Presidio inspection.

use prodex_domain::{FindingKind, InspectionCoverage};
use std::error::Error;
use std::fmt;

pub(super) const MAX_PRESIDIO_JSON_DEPTH: usize = 32;
pub(super) const MAX_PRESIDIO_JSON_VALUES: usize = 256;
pub(super) const MAX_PRESIDIO_JSON_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PresidioJsonString {
    pub path: String,
    pub text: String,
    pub sensitive_kind: Option<FindingKind>,
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
    collect_json_content_at(
        value,
        PresidioJsonInspectMode::SchemaOnly,
        None,
        "$",
        0,
        &mut state,
    )?;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PresidioJsonInspectMode {
    SchemaOnly,
    DirectStrings,
    AllStrings,
}

fn collect_json_content_at(
    value: &serde_json::Value,
    inspect_mode: PresidioJsonInspectMode,
    sensitive_kind: Option<FindingKind>,
    path: &str,
    depth: usize,
    state: &mut PresidioJsonWalkState,
) -> Result<(), PresidioJsonContentError> {
    if depth > MAX_PRESIDIO_JSON_DEPTH {
        return Err(PresidioJsonContentError::TooDeep);
    }
    match value {
        serde_json::Value::String(text) if inspect_mode != PresidioJsonInspectMode::SchemaOnly => {
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
                sensitive_kind,
            });
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_json_content_at(
                    item,
                    inspect_mode,
                    sensitive_kind,
                    &format!("{path}[{index}]"),
                    depth + 1,
                    state,
                )?;
            }
        }
        serde_json::Value::Object(fields) => {
            let skip_tools = path == "$" || json_object_declares_tools(fields);
            let inspect_all_strings = inspect_mode == PresidioJsonInspectMode::AllStrings;
            for (key, value) in fields {
                if skip_tools && key == "tools" {
                    continue;
                }
                if unsupported_modality_field(key) && !value.is_null() {
                    state.unsupported_modality = true;
                    continue;
                }
                let field_mode = json_string_inspection_mode(key);
                let child_mode = if inspect_all_strings {
                    PresidioJsonInspectMode::AllStrings
                } else {
                    field_mode
                };
                let child_sensitive_kind = sensitive_json_key_kind(key).or(sensitive_kind);
                let child_path = if field_mode != PresidioJsonInspectMode::SchemaOnly {
                    format!("{path}.{key}")
                } else if inspect_all_strings {
                    format!("{path}.*")
                } else {
                    path.to_string()
                };
                collect_json_content_at(
                    value,
                    child_mode,
                    child_sensitive_kind,
                    &child_path,
                    depth + 1,
                    state,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn sensitive_json_key_kind(key: &str) -> Option<FindingKind> {
    if !redaction::redaction_key_looks_sensitive(key) {
        return None;
    }
    let normalized = key
        .bytes()
        .filter(u8::is_ascii_alphanumeric)
        .map(|byte| byte.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let normalized = std::str::from_utf8(&normalized).ok()?;
    Some(if normalized.contains("privatekey") {
        FindingKind::PrivateKey
    } else if normalized.contains("apikey") {
        FindingKind::ApiKey
    } else if normalized.contains("token") || normalized == "authorization" {
        FindingKind::AccessToken
    } else {
        FindingKind::Password
    })
}

fn json_string_inspection_mode(key: &str) -> PresidioJsonInspectMode {
    match key {
        "arguments" | "output" => PresidioJsonInspectMode::AllStrings,
        "content" | "input" | "instructions" | "prompt" | "text" => {
            PresidioJsonInspectMode::DirectStrings
        }
        _ => PresidioJsonInspectMode::SchemaOnly,
    }
}

fn json_object_declares_tools(fields: &serde_json::Map<String, serde_json::Value>) -> bool {
    fields.get("role").and_then(serde_json::Value::as_str) == Some("developer")
}

fn unsupported_modality_field(key: &str) -> bool {
    matches!(
        key,
        "audio"
            | "audio_url"
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
    replace_json_string_values_at(
        value,
        if inspect_strings {
            PresidioJsonInspectMode::AllStrings
        } else {
            PresidioJsonInspectMode::SchemaOnly
        },
        values,
        true,
    );
}

fn replace_json_string_values_at<'a>(
    value: &mut serde_json::Value,
    inspect_mode: PresidioJsonInspectMode,
    values: &mut impl Iterator<Item = &'a str>,
    root: bool,
) {
    match value {
        serde_json::Value::String(text) if inspect_mode != PresidioJsonInspectMode::SchemaOnly => {
            if let Some(redacted) = values.next() {
                *text = redacted.to_string();
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                replace_json_string_values_at(item, inspect_mode, values, false);
            }
        }
        serde_json::Value::Object(fields) => {
            let skip_tools = root || json_object_declares_tools(fields);
            let inspect_all_strings = inspect_mode == PresidioJsonInspectMode::AllStrings;
            for (key, value) in fields {
                if skip_tools && key == "tools" {
                    continue;
                }
                if unsupported_modality_field(key) {
                    continue;
                }
                let field_mode = json_string_inspection_mode(key);
                replace_json_string_values_at(
                    value,
                    if inspect_all_strings {
                        PresidioJsonInspectMode::AllStrings
                    } else {
                        field_mode
                    },
                    values,
                    false,
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
            "input": [{
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
        for value in [
            serde_json::json!({
                "input": "inspect me",
                "input_image": {"url": "https://example.com/synthetic.png"}
            }),
            serde_json::json!({"input": "inspect me", "input_audio": "synthetic-audio"}),
            serde_json::json!({"input": "inspect me", "input_file": "synthetic-file"}),
        ] {
            let content = collect_json_content(&value).unwrap();
            assert_eq!(content.coverage, InspectionCoverage::Partial);
        }

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
    fn walker_preserves_codex_audio_payload_and_protocol_metadata() {
        let audio_url = "data:audio/wav;base64,AAAA";
        let mut request = serde_json::json!({
            "input": [{
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "user@example.com"},
                    {"type": "input_audio", "audio_url": audio_url},
                ],
            }],
        });

        let content = collect_json_content(&request).unwrap();
        assert_eq!(content.coverage, InspectionCoverage::Partial);
        assert_eq!(
            content
                .values
                .iter()
                .map(|value| value.text.as_str())
                .collect::<Vec<_>>(),
            ["user@example.com"]
        );

        replace_json_string_values(&mut request, false, &mut ["<redacted>"].into_iter());
        assert_eq!(request["input"][0]["type"], "message");
        assert_eq!(request["input"][0]["role"], "user");
        assert_eq!(request["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(request["input"][0]["content"][0]["text"], "<redacted>");
        assert_eq!(request["input"][0]["content"][1]["type"], "input_audio");
        assert_eq!(request["input"][0]["content"][1]["audio_url"], audio_url);
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

    #[test]
    fn walker_skips_tool_schemas_but_not_user_content() {
        let tools = (0..=MAX_PRESIDIO_JSON_VALUES)
            .map(|index| {
                serde_json::json!({
                    "parameters": {"properties": {"input": {"description": format!("schema {index}")}}}
                })
            })
            .collect::<Vec<_>>();
        let mut request = serde_json::json!({
            "input": [
                {"role": "developer", "tools": tools.clone()},
                {"role": "user", "content": {"tools": {"content": "user@example.com"}}}
            ],
            "tools": tools,
        });
        let content = collect_json_content(&request).unwrap();

        assert!(
            content
                .values
                .iter()
                .any(|value| value.text == "user@example.com")
        );
        assert!(
            content
                .values
                .iter()
                .all(|value| !value.text.starts_with("schema "))
        );

        let redacted = content
            .values
            .iter()
            .map(|value| {
                if value.text == "user@example.com" {
                    "<redacted>"
                } else {
                    value.text.as_str()
                }
            })
            .collect::<Vec<_>>();
        replace_json_string_values(&mut request, false, &mut redacted.into_iter());
        assert_eq!(
            request["input"][1]["content"]["tools"]["content"],
            "<redacted>"
        );
        assert_eq!(
            request["input"][0]["tools"][0]["parameters"]["properties"]["input"]["description"],
            "schema 0"
        );
        assert_eq!(
            request["tools"][0]["parameters"]["properties"]["input"]["description"],
            "schema 0"
        );
    }
}
