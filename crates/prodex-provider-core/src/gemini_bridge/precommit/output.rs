//! Gemini pre-commit output detection helpers.

use super::GeminiProviderCorePrecommitProbe;
use crate::gemini_bridge::{
    gemini_provider_core_media_content_item_from_part,
    gemini_provider_core_text_echoes_internal_instruction,
    gemini_provider_core_text_from_special_part, gemini_provider_core_visible_text_from_part,
};

pub(super) fn gemini_provider_core_precommit_apply_parts(
    value: &serde_json::Value,
    probe: &mut GeminiProviderCorePrecommitProbe,
) {
    let Some(parts) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    for part in parts {
        if part.get("functionCall").is_some()
            || gemini_provider_core_media_content_item_from_part(part).is_some()
            || gemini_provider_core_text_from_special_part(part).is_some()
        {
            probe.visible_output = true;
            return;
        }
        let Some(text) = part.get("text").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        if part
            .get("thought")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            probe.reasoning_output = true;
        } else if gemini_provider_core_visible_text_from_part(part).is_none()
            || gemini_provider_core_text_echoes_internal_instruction(
                text,
                &probe.internal_instruction_corpus,
            )
        {
            continue;
        } else {
            probe.visible_output = true;
            return;
        }
    }
}

pub(super) fn gemini_provider_core_precommit_has_grounding(value: &serde_json::Value) -> bool {
    value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("groundingMetadata"))
        .is_some_and(|metadata| {
            metadata
                .get("webSearchQueries")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|queries| !queries.is_empty())
                || metadata
                    .get("groundingChunks")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|chunks| !chunks.is_empty())
        })
}
