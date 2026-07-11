//! Gemini pre-commit stream classification helpers.

mod output;

use self::output::{
    gemini_provider_core_precommit_apply_parts, gemini_provider_core_precommit_has_grounding,
};
use super::{
    gemini_provider_core_finish_reason, gemini_provider_core_finish_reason_retryable_invalid,
    gemini_provider_core_normalized_response_value, gemini_provider_core_prompt_feedback_failure,
};

#[derive(Default)]
pub struct GeminiProviderCorePrecommitProbe {
    pub visible_output: bool,
    pub reasoning_output: bool,
    pub internal_instruction_corpus: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeminiProviderCorePrecommitDecision {
    Continue,
    Commit,
    RetryableInvalid(String),
}

pub fn gemini_provider_core_precommit_decision_for_data_lines(
    data_lines: &[String],
    probe: &mut GeminiProviderCorePrecommitProbe,
) -> GeminiProviderCorePrecommitDecision {
    let data = data_lines.join("\n");
    let trimmed = data.trim();
    if trimmed == "[DONE]" {
        return if probe.visible_output || probe.reasoning_output {
            GeminiProviderCorePrecommitDecision::Commit
        } else {
            GeminiProviderCorePrecommitDecision::RetryableInvalid(
                "gemini_empty_response".to_string(),
            )
        };
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return gemini_provider_core_precommit_decision_for_value(&value, probe);
    }
    let mut parsed_any = false;
    for line in data_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        parsed_any = true;
        match gemini_provider_core_precommit_decision_for_value(&value, probe) {
            GeminiProviderCorePrecommitDecision::Continue => {}
            decision => return decision,
        }
    }
    if parsed_any {
        GeminiProviderCorePrecommitDecision::Continue
    } else {
        GeminiProviderCorePrecommitDecision::Commit
    }
}

fn gemini_provider_core_precommit_decision_for_value(
    value: &serde_json::Value,
    probe: &mut GeminiProviderCorePrecommitProbe,
) -> GeminiProviderCorePrecommitDecision {
    let value = gemini_provider_core_normalized_response_value(value);
    let value = value.as_ref();
    if value.get("error").is_some() || gemini_provider_core_prompt_feedback_failure(value).is_some()
    {
        return GeminiProviderCorePrecommitDecision::Commit;
    }
    if gemini_provider_core_precommit_has_grounding(value) {
        probe.visible_output = true;
        return GeminiProviderCorePrecommitDecision::Commit;
    }
    gemini_provider_core_precommit_apply_parts(value, probe);
    if probe.visible_output {
        return GeminiProviderCorePrecommitDecision::Commit;
    }
    let Some(reason) = gemini_provider_core_finish_reason(value) else {
        return GeminiProviderCorePrecommitDecision::Continue;
    };
    if gemini_provider_core_finish_reason_retryable_invalid(&reason) {
        return GeminiProviderCorePrecommitDecision::RetryableInvalid(reason);
    }
    match reason.as_str() {
        "STOP" if !probe.reasoning_output => GeminiProviderCorePrecommitDecision::RetryableInvalid(
            "gemini_empty_response".to_string(),
        ),
        _ => GeminiProviderCorePrecommitDecision::Commit,
    }
}
