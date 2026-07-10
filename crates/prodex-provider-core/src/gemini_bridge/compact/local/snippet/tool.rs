//! Gemini compact tool transcript snippet extraction.

use super::super::{
    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
    gemini_provider_core_local_compact_text_from_content, gemini_provider_core_truncate_utf8,
};

pub(super) fn gemini_provider_core_local_compact_tool_snippet(
    item_type: &str,
    object: &serde_json::Map<String, serde_json::Value>,
) -> String {
    match item_type {
        "function_call" => {
            let name = object
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("function");
            let call_id = object
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let arguments = object
                .get("arguments")
                .and_then(gemini_provider_core_local_compact_text_from_content)
                .unwrap_or_default();
            format!(
                "tool call {name} ({call_id}): {}",
                gemini_provider_core_truncate_utf8(
                    arguments,
                    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
                )
            )
        }
        "custom_tool_call" => {
            let name = object
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("custom_tool");
            let call_id = object
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let input = object
                .get("input")
                .and_then(gemini_provider_core_local_compact_text_from_content)
                .unwrap_or_default();
            format!(
                "custom tool call {name} ({call_id}): {}",
                gemini_provider_core_truncate_utf8(
                    input,
                    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
                )
            )
        }
        "function_call_output" | "custom_tool_call_output" => {
            let call_id = object
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let output = object
                .get("output")
                .and_then(gemini_provider_core_local_compact_text_from_content)
                .unwrap_or_default();
            format!(
                "tool output {call_id}: {}",
                gemini_provider_core_truncate_utf8(
                    output,
                    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
                )
            )
        }
        "local_shell_call" => {
            let call_id = object
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let action = object
                .get("action")
                .and_then(gemini_provider_core_local_compact_text_from_content)
                .unwrap_or_default();
            format!(
                "local shell call {call_id}: {}",
                gemini_provider_core_truncate_utf8(
                    action,
                    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
                )
            )
        }
        "web_search_call" => {
            let action = object
                .get("action")
                .and_then(gemini_provider_core_local_compact_text_from_content)
                .unwrap_or_default();
            format!(
                "web search: {}",
                gemini_provider_core_truncate_utf8(
                    action,
                    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
                )
            )
        }
        _ => unreachable!("unknown Gemini compact tool snippet type"),
    }
}
