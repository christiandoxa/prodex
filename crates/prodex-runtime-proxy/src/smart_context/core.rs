use super::*;
use std::borrow::Cow;

pub(super) const SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX: &str = "psc:";
pub(super) const SMART_CONTEXT_MODEL_SCAN_MAX_BYTES: usize = 4 * 1024;
pub(super) const SMART_CONTEXT_MODEL_NAME_MAX_BYTES: usize = 128;

pub fn smart_context_structural_minify_json_body(body: &[u8]) -> Cow<'_, [u8]> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return Cow::Borrowed(body);
    };
    smart_context_structural_minify_json_value_body(body, &value)
}

pub fn smart_context_structural_minify_json_value_body<'a>(
    original_body: &'a [u8],
    value: &serde_json::Value,
) -> Cow<'a, [u8]> {
    match serde_json::to_vec(value) {
        Ok(body) if body != original_body => Cow::Owned(body),
        _ => Cow::Borrowed(original_body),
    }
}

pub fn smart_context_model_name_from_body(body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }
    if body.len() <= SMART_CONTEXT_MODEL_SCAN_MAX_BYTES
        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
    {
        return smart_context_model_name_from_value(&value);
    }
    let scan_len = body.len().min(SMART_CONTEXT_MODEL_SCAN_MAX_BYTES);
    let scan = std::str::from_utf8(&body[..scan_len]).ok()?;
    smart_context_model_name_from_json_prefix(scan)
}

pub fn smart_context_normalized_model_name(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty()
        || value.len() > SMART_CONTEXT_MODEL_NAME_MAX_BYTES
        || value.chars().any(char::is_control)
    {
        return None;
    }
    Some(value.to_string())
}

pub(super) fn smart_context_model_name_from_value(value: &serde_json::Value) -> Option<String> {
    smart_context_normalized_model_name(value.get("model")?.as_str())
}

pub(super) fn smart_context_model_name_from_json_prefix(text: &str) -> Option<String> {
    let (_, after_key) = text.split_once("\"model\"")?;
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    let mut chars = after_colon.strip_prefix('"')?.chars();
    let mut model = String::new();
    let mut escaped = false;
    for ch in chars.by_ref() {
        if escaped {
            model.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return smart_context_normalized_model_name(Some(&model));
        } else if ch.is_control() {
            return None;
        } else {
            model.push(ch);
        }
        if model.len() > SMART_CONTEXT_MODEL_NAME_MAX_BYTES {
            return None;
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextExactnessDecision {
    Allow,
    RequireExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextExactnessReason {
    ExplicitExactMode,
    PreviousResponseAffinity,
    TurnStateAffinity,
    SessionAffinity,
    ToolOutputWithoutArtifact,
    RehydrateRequired,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartContextExactnessInput {
    pub exact_mode: bool,
    pub previous_response_id: Option<String>,
    pub turn_state: Option<String>,
    pub session_id: Option<String>,
    pub tool_output_without_artifact: bool,
    pub missing_rehydrate_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextExactnessGuard {
    pub decision: SmartContextExactnessDecision,
    pub reasons: Vec<SmartContextExactnessReason>,
}

pub fn smart_context_exactness_guard(
    input: SmartContextExactnessInput,
) -> SmartContextExactnessGuard {
    let mut reasons = Vec::new();
    if input.exact_mode {
        reasons.push(SmartContextExactnessReason::ExplicitExactMode);
    }
    if input.previous_response_id.as_deref().is_some_and(non_empty) {
        reasons.push(SmartContextExactnessReason::PreviousResponseAffinity);
    }
    if input.turn_state.as_deref().is_some_and(non_empty) {
        reasons.push(SmartContextExactnessReason::TurnStateAffinity);
    }
    if input.session_id.as_deref().is_some_and(non_empty) {
        reasons.push(SmartContextExactnessReason::SessionAffinity);
    }
    if input.tool_output_without_artifact {
        reasons.push(SmartContextExactnessReason::ToolOutputWithoutArtifact);
    }
    if input
        .missing_rehydrate_refs
        .iter()
        .any(|value| non_empty(value))
    {
        reasons.push(SmartContextExactnessReason::RehydrateRequired);
    }

    SmartContextExactnessGuard {
        decision: if reasons.is_empty() {
            SmartContextExactnessDecision::Allow
        } else {
            SmartContextExactnessDecision::RequireExact
        },
        reasons,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextArtifactRef {
    pub id: String,
    pub byte_len: usize,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextToolOutput {
    pub call_id: String,
    pub text: String,
    pub artifact: Option<SmartContextArtifactRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextCondensedToolOutput {
    Inline {
        call_id: String,
        text: String,
        content_hash: String,
    },
    ArtifactBacked {
        call_id: String,
        artifact: SmartContextArtifactRef,
        content_hash: String,
        summary: String,
    },
}
