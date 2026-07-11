//! Gemini semantic compact continuation summary helpers.

use super::GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SUMMARY_BYTES;
use super::text::{
    gemini_provider_core_local_compact_text_from_content, gemini_provider_core_truncate_utf8,
    gemini_provider_core_truncate_utf8_edges,
};

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
        .filter(|text| !text.trim().is_empty())
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
        .filter(|text| !text.trim().is_empty())
        .map(|text| {
            gemini_provider_core_truncate_utf8_edges(
                text,
                GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_LATEST_TOOL_MAX_BYTES,
            )
        });

    let mut summary = String::new();
    if let Some(active_user) = active_user {
        summary.push_str("Active user request that must still be completed:\n");
        summary.push_str(active_user.trim());
        summary.push_str("\n\n");
    }
    if let Some(latest_tool) = latest_tool {
        summary.push_str("Latest tool result after the active request:\n");
        summary.push_str(latest_tool.trim());
        summary.push_str("\n\n");
    }
    summary.push_str("Semantic continuation summary:\n");
    summary.push_str(semantic_summary.trim());
    summary.push_str(
        "\n\nContinue the active user request. Do not merely acknowledge repository, optimizer, or environment instructions.",
    );
    gemini_provider_core_truncate_utf8(
        summary,
        GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SUMMARY_BYTES,
    )
}
