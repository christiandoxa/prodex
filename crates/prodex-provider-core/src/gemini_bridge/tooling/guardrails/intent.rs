//! Gemini assistant tool-intent guardrails.

pub fn gemini_provider_core_tool_intent_without_call(text: &str) -> Option<&'static str> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.len() < 16 {
        return None;
    }
    let future_tool_intent = [
        "i'll use",
        "i will use",
        "i'll call",
        "i will call",
        "i'll run",
        "i will run",
        "i'll search",
        "i will search",
        "i'll inspect",
        "i will inspect",
        "i'll read",
        "i will read",
        "i'm going to use",
        "i am going to use",
        "now, i'll use",
        "next, i'll use",
    ]
    .iter()
    .any(|phrase| lower.contains(phrase));
    if !future_tool_intent {
        return None;
    }
    [
        "exec_command",
        "write_stdin",
        "apply_patch",
        "sqz_grep",
        "sqz_read_file",
        "sqz_list_dir",
        "read_mcp_resource",
        "list_mcp_resources",
        "tool_search",
        "rg",
        "grep",
    ]
    .into_iter()
    .find(|tool| gemini_provider_core_contains_tool_token(&lower, tool))
}

fn gemini_provider_core_contains_tool_token(text: &str, token: &str) -> bool {
    let mut offset = 0;
    while let Some(index) = text[offset..].find(token) {
        let start = offset + index;
        let end = start + token.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        let before_boundary = before.is_none_or(gemini_provider_core_tool_token_boundary);
        let after_boundary = after.is_none_or(gemini_provider_core_tool_token_boundary);
        if before_boundary && after_boundary {
            return true;
        }
        offset = end;
    }
    false
}

fn gemini_provider_core_tool_token_boundary(ch: char) -> bool {
    !(ch.is_ascii_alphanumeric() || ch == '_')
}
