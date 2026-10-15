//! Gemini content sequence hardening.

mod mojo;
mod parts;
mod tool_pairs;

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
    mojo::apply(
        contents,
        prodex_mojo_core::json::GeminiContentOperation::EnforceRoleShape,
    );
}

#[cfg(test)]
mod tests;
