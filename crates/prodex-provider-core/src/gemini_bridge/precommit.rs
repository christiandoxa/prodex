//! Gemini pre-commit stream classification helpers.

use super::gemini_provider_core_normalized_response_value;
use crate::mojo_json::Document;
use prodex_mojo_core::rich::{GeminiPrecommitDecision, GeminiPrecommitFrame, GeminiPrecommitState};

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
    let decision = prodex_mojo_core::rich::gemini_precommit_frame_decision(
        GeminiPrecommitFrame {
            data: &data,
            parsed_any,
        },
        GeminiPrecommitState {
            visible_output: probe.visible_output,
            reasoning_output: probe.reasoning_output,
        },
    )
    .expect("Mojo Gemini pre-commit frame policy returned invalid output");
    gemini_provider_core_precommit_decision(decision)
}

fn gemini_provider_core_precommit_decision_for_value(
    value: &serde_json::Value,
    probe: &mut GeminiProviderCorePrecommitProbe,
) -> GeminiProviderCorePrecommitDecision {
    let value = gemini_provider_core_normalized_response_value(value);
    let mut document = Document::default();
    document.root(value.as_ref());
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let plan = prodex_mojo_core::rich::gemini_precommit_plan(
        &document.nodes,
        raw,
        &probe.internal_instruction_corpus,
        GeminiPrecommitState {
            visible_output: probe.visible_output,
            reasoning_output: probe.reasoning_output,
        },
    )
    .expect("Mojo Gemini pre-commit policy returned invalid output");
    probe.visible_output = plan.state.visible_output;
    probe.reasoning_output = plan.state.reasoning_output;
    gemini_provider_core_precommit_decision(plan.decision)
}

fn gemini_provider_core_precommit_decision(
    decision: GeminiPrecommitDecision<'_>,
) -> GeminiProviderCorePrecommitDecision {
    match decision {
        GeminiPrecommitDecision::Continue => GeminiProviderCorePrecommitDecision::Continue,
        GeminiPrecommitDecision::Commit => GeminiProviderCorePrecommitDecision::Commit,
        GeminiPrecommitDecision::RetryableInvalid(reason) => {
            GeminiProviderCorePrecommitDecision::RetryableInvalid(reason.to_string())
        }
    }
}

#[cfg(test)]
mod tests;
