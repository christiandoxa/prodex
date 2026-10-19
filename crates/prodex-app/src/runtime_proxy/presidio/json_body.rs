//! Shared compiled selection for bounded Presidio JSON inspection and masking.

use prodex_domain::{FindingKind, InspectionCoverage};
use prodex_mojo_core::json::{JsonKind, redaction_json_content_plan};
use serde::Deserialize;
use std::error::Error;
use std::fmt;

#[cfg(test)]
pub(super) const MAX_PRESIDIO_JSON_DEPTH: usize = 32;
#[cfg(test)]
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
    slots: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PresidioJsonContentError {
    TooDeep,
    TooManyValues,
    TextBudgetExhausted,
    InvalidPlan,
    ReplacementMismatch,
}

impl fmt::Display for PresidioJsonContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooDeep | Self::TooManyValues | Self::TextBudgetExhausted => {
                "request content exceeds inspection limits"
            }
            Self::InvalidPlan => "compiled Mojo JSON inspection returned an invalid plan",
            Self::ReplacementMismatch => "JSON inspection replacement count or target changed",
        })
    }
}

impl Error for PresidioJsonContentError {}

#[derive(Deserialize)]
struct JsonContentSlot {
    node: usize,
    path: String,
    sensitive_kind: Option<FindingKind>,
}

#[derive(Deserialize)]
struct JsonContentPlan {
    error: u8,
    coverage: InspectionCoverage,
    slots: Vec<JsonContentSlot>,
}

pub(super) fn collect_json_content(
    value: &serde_json::Value,
) -> Result<PresidioJsonContent, PresidioJsonContentError> {
    let invalid = PresidioJsonContentError::InvalidPlan;
    let tree = crate::json_boundary::app_json_tree_without_wire(value).map_err(|_| invalid)?;
    let output = redaction_json_content_plan(&tree.nodes).map_err(|_| invalid)?;
    let plan: JsonContentPlan = serde_json::from_slice(&output).map_err(|_| invalid)?;
    match plan.error {
        0 => {}
        1 => return Err(PresidioJsonContentError::TooDeep),
        2 => return Err(PresidioJsonContentError::TooManyValues),
        3 => return Err(PresidioJsonContentError::TextBudgetExhausted),
        _ => return Err(invalid),
    }
    let mut slots = Vec::with_capacity(plan.slots.len());
    let mut values = Vec::with_capacity(plan.slots.len());
    for slot in plan.slots {
        let node = tree.nodes.get(slot.node).ok_or(invalid)?;
        if !matches!(node.kind, JsonKind::String)
            || slots.last().is_some_and(|last| *last >= slot.node)
        {
            return Err(invalid);
        }
        slots.push(slot.node);
        values.push(PresidioJsonString {
            path: slot.path,
            text: node.text.to_string(),
            sensitive_kind: slot.sensitive_kind,
        });
    }
    Ok(PresidioJsonContent {
        values,
        coverage: plan.coverage,
        slots,
    })
}

pub(super) fn replace_json_string_values<'a>(
    value: &mut serde_json::Value,
    content: &PresidioJsonContent,
    values: &mut impl Iterator<Item = &'a str>,
) -> Result<(), PresidioJsonContentError> {
    let replacements = values.take(content.slots.len() + 1).collect::<Vec<_>>();
    if replacements.len() != content.slots.len() {
        return Err(PresidioJsonContentError::ReplacementMismatch);
    }
    let mut slots = content.slots.iter().copied().zip(replacements).peekable();
    apply_json_string_slots(value, &mut 0, &mut slots)?;
    if slots.next().is_some() {
        return Err(PresidioJsonContentError::ReplacementMismatch);
    }
    Ok(())
}

// Serde owns mutable JSON storage; this walk only applies Mojo's node indices.
fn apply_json_string_slots<'a>(
    value: &mut serde_json::Value,
    index: &mut usize,
    slots: &mut std::iter::Peekable<impl Iterator<Item = (usize, &'a str)>>,
) -> Result<(), PresidioJsonContentError> {
    if slots.peek().is_some_and(|(slot, _)| slot == index) {
        let serde_json::Value::String(text) = value else {
            return Err(PresidioJsonContentError::ReplacementMismatch);
        };
        if let Some((_, replacement)) = slots.next() {
            *text = replacement.to_string();
        }
    }
    *index += 1;
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                apply_json_string_slots(item, index, slots)?;
            }
        }
        serde_json::Value::Object(fields) => {
            for item in fields.values_mut() {
                apply_json_string_slots(item, index, slots)?;
            }
        }
        _ => {}
    }
    Ok(())
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
    fn compiled_inspection_accepts_exact_depth_value_and_text_limits() {
        let mut deep = serde_json::json!("bounded");
        for _ in 0..MAX_PRESIDIO_JSON_DEPTH {
            deep = serde_json::json!({"input": deep});
        }
        assert_eq!(collect_json_content(&deep).unwrap().values.len(), 1);
        let many = serde_json::json!({"input": vec!["bounded"; MAX_PRESIDIO_JSON_VALUES]});
        assert_eq!(
            collect_json_content(&many).unwrap().values.len(),
            MAX_PRESIDIO_JSON_VALUES
        );
        let text = serde_json::json!({"input": "x".repeat(MAX_PRESIDIO_JSON_TEXT_BYTES)});
        assert_eq!(
            collect_json_content(&text).unwrap().values[0].text.len(),
            MAX_PRESIDIO_JSON_TEXT_BYTES
        );
    }

    #[test]
    fn compiled_replacement_rejects_count_changes_before_mutation() {
        let original = serde_json::json!({"input": ["first", "second"]});
        let content = collect_json_content(&original).unwrap();
        for replacements in [vec!["masked"], vec!["masked", "masked", "extra"]] {
            let mut request = original.clone();
            assert_eq!(
                replace_json_string_values(&mut request, &content, &mut replacements.into_iter()),
                Err(PresidioJsonContentError::ReplacementMismatch)
            );
            assert_eq!(request, original);
        }
    }

    #[test]
    fn replacement_targets_match_collected_slots_through_opaque_root_objects() {
        let mut request = serde_json::json!({
            "a": {"tools": {"content": "not inspected"}},
            "input": "user@example.com",
        });
        let content = collect_json_content(&request).unwrap();
        assert_eq!(
            content
                .values
                .iter()
                .map(|value| value.text.as_str())
                .collect::<Vec<_>>(),
            ["user@example.com"]
        );
        replace_json_string_values(&mut request, &content, &mut ["<redacted>"].into_iter())
            .unwrap();
        assert_eq!(
            request,
            serde_json::json!({
                "a": {"tools": {"content": "not inspected"}},
                "input": "<redacted>",
            })
        );
    }

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
    fn walker_downgrades_known_content_with_opaque_fields() {
        let value = serde_json::json!({
            "model": "example-model",
            "input": "inspect me",
            "opaque": {"secret": "must not be treated as covered"}
        });

        let content = collect_json_content(&value).unwrap();

        assert_eq!(content.coverage, InspectionCoverage::Partial);
        assert_eq!(
            content
                .values
                .iter()
                .map(|value| value.text.as_str())
                .collect::<Vec<_>>(),
            ["inspect me"]
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

        replace_json_string_values(&mut request, &content, &mut ["<redacted>"].into_iter())
            .unwrap();
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
        replace_json_string_values(&mut request, &content, &mut redacted.into_iter()).unwrap();
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
