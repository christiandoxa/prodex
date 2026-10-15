//! Gemini tool-call/tool-response sequence repair through compiled Mojo.
mod order;
pub(super) use self::order::gemini_provider_core_refine_tool_response_order;
use prodex_mojo_core::json::GeminiContentOperation;

pub(super) fn gemini_provider_core_pair_tool_calls(contents: &mut Vec<serde_json::Value>) {
    super::mojo::apply(contents, GeminiContentOperation::PairMissingResults);
}

pub(super) fn gemini_provider_core_repair_orphan_tool_responses(
    contents: &mut Vec<serde_json::Value>,
) {
    super::mojo::apply(contents, GeminiContentOperation::RepairOrphanResponses);
}

#[cfg(test)]
mod tests;
