//! Gemini assistant tool-intent guardrails.

use prodex_mojo_core::gemini_guardrails::{GeminiToolIntent, gemini_tool_intent};

pub fn gemini_provider_core_tool_intent_without_call(text: &str) -> Option<&'static str> {
    match gemini_tool_intent(text)
        .expect("Mojo Gemini tool-intent guardrail should accept Rust strings")
    {
        Some(GeminiToolIntent::ExecCommand) => Some("exec_command"),
        Some(GeminiToolIntent::WriteStdin) => Some("write_stdin"),
        Some(GeminiToolIntent::ApplyPatch) => Some("apply_patch"),
        Some(GeminiToolIntent::SqzGrep) => Some("sqz_grep"),
        Some(GeminiToolIntent::SqzReadFile) => Some("sqz_read_file"),
        Some(GeminiToolIntent::SqzListDir) => Some("sqz_list_dir"),
        Some(GeminiToolIntent::ReadMcpResource) => Some("read_mcp_resource"),
        Some(GeminiToolIntent::ListMcpResources) => Some("list_mcp_resources"),
        Some(GeminiToolIntent::ToolSearch) => Some("tool_search"),
        Some(GeminiToolIntent::Rg) => Some("rg"),
        Some(GeminiToolIntent::Grep) => Some("grep"),
        None => None,
    }
}
