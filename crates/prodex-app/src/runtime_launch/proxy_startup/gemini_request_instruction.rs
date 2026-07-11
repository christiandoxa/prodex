use super::super::gemini_request_policy::RuntimeGeminiPolicyCompat;
use super::{
    PRODEX_GEMINI_CODEX_PARITY_INSTRUCTION, PRODEX_GEMINI_TOOL_DISCIPLINE_INSTRUCTION,
    runtime_gemini_hierarchical_memory,
};
use crate::RuntimeGeminiConfig;
use prodex_provider_core::gemini_provider_core_system_instruction_from_chat;

pub(super) fn runtime_gemini_system_instruction(
    chat: &serde_json::Value,
    original: &serde_json::Value,
    allow_local_file_access: bool,
    config: &RuntimeGeminiConfig,
) -> Option<serde_json::Value> {
    let memory = allow_local_file_access
        .then(|| runtime_gemini_hierarchical_memory(original, config))
        .flatten();
    let policy = allow_local_file_access
        .then(|| RuntimeGeminiPolicyCompat::from_request_and_files(original, config).summary())
        .flatten();
    gemini_provider_core_system_instruction_from_chat(
        chat,
        original,
        PRODEX_GEMINI_CODEX_PARITY_INSTRUCTION,
        PRODEX_GEMINI_TOOL_DISCIPLINE_INSTRUCTION,
        memory.as_deref(),
        policy.as_deref(),
    )
}
