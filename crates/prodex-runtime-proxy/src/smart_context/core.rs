use super::*;

pub(super) const SMART_CONTEXT_MODEL_SCAN_MAX_BYTES: usize = 4 * 1024;
pub(super) const SMART_CONTEXT_MODEL_NAME_MAX_BYTES: usize = 128;

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
    smart_context_model_name_from_json_prefix(scan).or_else(|| {
        serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .and_then(|value| smart_context_model_name_from_value(&value))
    })
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
    let decision = prodex_mojo_core::runtime::smart_context_exactness_plan(
        input.exact_mode,
        input.previous_response_id.as_deref().is_some_and(non_empty),
        input.turn_state.as_deref().is_some_and(non_empty),
        input.session_id.as_deref().is_some_and(non_empty),
        input.tool_output_without_artifact,
    )
    .expect("Mojo Smart Context exactness planner returned invalid output");
    SmartContextExactnessGuard {
        decision: match decision.0 {
            0 => SmartContextExactnessDecision::Allow,
            1 => SmartContextExactnessDecision::RequireExact,
            _ => unreachable!("Mojo Smart Context exactness decision was validated"),
        },
        reasons: smart_context_exactness_reasons_from_bits(decision.1),
    }
}

fn smart_context_exactness_reasons_from_bits(bits: u64) -> Vec<SmartContextExactnessReason> {
    [
        (1_u64 << 0, SmartContextExactnessReason::ExplicitExactMode),
        (
            1_u64 << 1,
            SmartContextExactnessReason::PreviousResponseAffinity,
        ),
        (1_u64 << 2, SmartContextExactnessReason::TurnStateAffinity),
        (1_u64 << 3, SmartContextExactnessReason::SessionAffinity),
        (
            1_u64 << 4,
            SmartContextExactnessReason::ToolOutputWithoutArtifact,
        ),
    ]
    .into_iter()
    .filter_map(|(bit, reason)| (bits & bit != 0).then_some(reason))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactness_planner_preserves_expected_reasons_in_every_feature_mode() {
        use SmartContextExactnessDecision::{Allow, RequireExact};
        use SmartContextExactnessReason::{
            ExplicitExactMode, PreviousResponseAffinity, SessionAffinity,
            ToolOutputWithoutArtifact, TurnStateAffinity,
        };

        let cases = [
            (SmartContextExactnessInput::default(), Allow, vec![]),
            (
                SmartContextExactnessInput {
                    exact_mode: true,
                    ..Default::default()
                },
                RequireExact,
                vec![ExplicitExactMode],
            ),
            (
                SmartContextExactnessInput {
                    previous_response_id: Some("response".into()),
                    ..Default::default()
                },
                RequireExact,
                vec![PreviousResponseAffinity],
            ),
            (
                SmartContextExactnessInput {
                    turn_state: Some("turn".into()),
                    ..Default::default()
                },
                RequireExact,
                vec![TurnStateAffinity],
            ),
            (
                SmartContextExactnessInput {
                    session_id: Some("session".into()),
                    ..Default::default()
                },
                RequireExact,
                vec![SessionAffinity],
            ),
            (
                SmartContextExactnessInput {
                    tool_output_without_artifact: true,
                    ..Default::default()
                },
                RequireExact,
                vec![ToolOutputWithoutArtifact],
            ),
            (
                SmartContextExactnessInput {
                    exact_mode: true,
                    previous_response_id: Some("response".into()),
                    turn_state: Some("turn".into()),
                    session_id: Some("session".into()),
                    tool_output_without_artifact: true,
                    ..Default::default()
                },
                RequireExact,
                vec![
                    ExplicitExactMode,
                    PreviousResponseAffinity,
                    TurnStateAffinity,
                    SessionAffinity,
                    ToolOutputWithoutArtifact,
                ],
            ),
            (
                SmartContextExactnessInput {
                    previous_response_id: Some(" \t ".into()),
                    turn_state: Some("".into()),
                    session_id: Some("  ".into()),
                    missing_rehydrate_refs: vec!["ignored".into()],
                    ..Default::default()
                },
                Allow,
                vec![],
            ),
        ];

        for (input, decision, reasons) in cases {
            assert_eq!(
                smart_context_exactness_guard(input),
                SmartContextExactnessGuard { decision, reasons }
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextArtifactRef {
    pub id: String,
    pub byte_len: usize,
    pub content_hash: String,
}
