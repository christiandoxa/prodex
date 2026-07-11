//! Gemini compact transcript snippet extraction.

mod tool;

use self::tool::gemini_provider_core_local_compact_tool_snippet;
use super::{
    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
    gemini_provider_core_local_compact_text_from_content, gemini_provider_core_truncate_utf8,
};

pub(super) fn gemini_provider_core_local_compact_snippet(
    item: &serde_json::Value,
) -> Option<String> {
    let object = item.as_object()?;
    let item_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("item");
    let snippet = match item_type {
        "message" => {
            let role = object
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let text = object
                .get("content")
                .and_then(gemini_provider_core_local_compact_text_from_content)
                .or_else(|| {
                    object
                        .get("text")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default();
            if text.trim().is_empty() {
                format!("{role} message with no text content")
            } else {
                format!(
                    "{role} message: {}",
                    gemini_provider_core_truncate_utf8(
                        text,
                        GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
                    )
                )
            }
        }
        "function_call"
        | "custom_tool_call"
        | "function_call_output"
        | "custom_tool_call_output"
        | "local_shell_call"
        | "web_search_call" => gemini_provider_core_local_compact_tool_snippet(item_type, object),
        "reasoning" => {
            let summary = object
                .get("summary")
                .and_then(gemini_provider_core_local_compact_text_from_content)
                .unwrap_or_default();
            if summary.trim().is_empty() {
                return None;
            }
            format!(
                "reasoning summary: {}",
                gemini_provider_core_truncate_utf8(
                    summary,
                    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
                )
            )
        }
        _ => {
            let text =
                gemini_provider_core_local_compact_text_from_content(item).unwrap_or_default();
            if text.trim().is_empty() {
                return None;
            }
            format!(
                "{item_type}: {}",
                gemini_provider_core_truncate_utf8(
                    text,
                    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
                )
            )
        }
    };

    Some(snippet)
}
