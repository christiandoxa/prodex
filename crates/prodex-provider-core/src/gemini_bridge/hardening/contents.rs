//! Gemini content sequence hardening.

mod mojo;
mod parts;
mod tool_pairs;

use self::parts::gemini_provider_core_content_role;
use self::tool_pairs::{
    gemini_provider_core_pair_tool_calls, gemini_provider_core_refine_tool_response_order,
    gemini_provider_core_repair_orphan_tool_responses,
};

pub fn gemini_provider_core_harden_contents(contents: &mut Vec<serde_json::Value>) {
    gemini_provider_core_coalesce_contents(contents);
    gemini_provider_core_pair_tool_calls(contents);
    gemini_provider_core_repair_orphan_tool_responses(contents);
    gemini_provider_core_refine_tool_response_order(contents);
    gemini_provider_core_enforce_content_role_shape(contents);
}

fn gemini_provider_core_coalesce_contents(contents: &mut Vec<serde_json::Value>) {
    mojo::apply(
        contents,
        prodex_mojo_core::json::GeminiContentOperation::Coalesce,
    );
}

fn gemini_provider_core_enforce_content_role_shape(contents: &mut Vec<serde_json::Value>) {
    if contents.is_empty() {
        return;
    }
    if gemini_provider_core_content_role(&contents[0]) == Some("model") {
        contents.insert(
            0,
            serde_json::json!({
                "role": "user",
                "parts": [{ "text": "[Continuing from previous Codex context.]" }],
            }),
        );
    }
    if contents.last().and_then(gemini_provider_core_content_role) == Some("model") {
        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{ "text": "Please continue." }],
        }));
    }
    gemini_provider_core_coalesce_contents(contents);
}

#[cfg(test)]
mod tests;
