//! Gemini semantic compact continuation summary helpers.

use super::GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SUMMARY_BYTES;
use super::text::{
    gemini_provider_core_local_compact_text_from_content, gemini_provider_core_truncate_utf8_edges,
};
use prodex_mojo_core::rich::format_gemini_semantic_continuation_summary;

const GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_ACTIVE_USER_MAX_BYTES: usize = 2 * 1024;
const GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_LATEST_TOOL_MAX_BYTES: usize = 1024;

pub fn gemini_provider_core_semantic_compact_continuation_summary(
    semantic_summary: &str,
    compact_request_body: &[u8],
) -> String {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(compact_request_body) else {
        return semantic_summary.trim().to_string();
    };
    let input = value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let active_user_index = input.iter().rposition(|item| {
        item.get("type").and_then(serde_json::Value::as_str) == Some("message")
            && item.get("role").and_then(serde_json::Value::as_str) == Some("user")
    });
    let active_user = active_user_index
        .and_then(|index| {
            input[index]
                .get("content")
                .and_then(gemini_provider_core_local_compact_text_from_content)
        })
        .map(|text| {
            gemini_provider_core_truncate_utf8_edges(
                text,
                GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_ACTIVE_USER_MAX_BYTES,
            )
        });
    let latest_tool = active_user_index
        .and_then(|index| {
            input[(index + 1)..].iter().rev().find(|item| {
                matches!(
                    item.get("type").and_then(serde_json::Value::as_str),
                    Some(
                        "function_call_output"
                            | "custom_tool_call_output"
                            | "local_shell_call_output"
                    )
                )
            })
        })
        .and_then(|item| {
            item.get("output")
                .or_else(|| item.get("content"))
                .and_then(gemini_provider_core_local_compact_text_from_content)
        })
        .map(|text| {
            gemini_provider_core_truncate_utf8_edges(
                text,
                GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_LATEST_TOOL_MAX_BYTES,
            )
        });

    format_gemini_semantic_continuation_summary(
        semantic_summary,
        active_user.as_deref(),
        latest_tool.as_deref(),
        GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SUMMARY_BYTES,
    )
    .expect("Mojo Gemini semantic continuation formatter returned invalid output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_continuation_formatting_is_mojo_owned_at_provider_boundary() {
        let body = serde_json::to_vec(&serde_json::json!({
            "input": [
                {"type":"message","role":"user","content":"older"},
                {"type":"message","role":"user","content":"  finish this  "},
                {"type":"function_call_output","output":"  tool result  "}
            ]
        }))
        .unwrap();
        let summary =
            gemini_provider_core_semantic_compact_continuation_summary("  semantic state  ", &body);
        assert_eq!(
            summary,
            "Active user request that must still be completed:\nfinish this\n\nLatest tool result after the active request:\ntool result\n\nSemantic continuation summary:\nsemantic state\n\nContinue the active user request. Do not merely acknowledge repository, optimizer, or environment instructions."
        );
    }
}
