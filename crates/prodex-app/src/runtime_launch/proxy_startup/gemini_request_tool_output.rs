use super::gemini_request::RUNTIME_GEMINI_TOOL_OUTPUT_PREVIEW_CHARS;
use crate::RuntimeGeminiConfig;
use std::env;
use std::path::{Path, PathBuf};

pub(super) fn runtime_gemini_mask_tool_response_for_history(
    tool_name: &str,
    call_id: &str,
    response: serde_json::Value,
    config: &RuntimeGeminiConfig,
) -> serde_json::Value {
    runtime_gemini_mask_tool_response_for_history_with_threshold(
        tool_name,
        call_id,
        response,
        config.tool_output_mask_threshold,
        config.tool_output_dir.as_deref(),
    )
}

fn runtime_gemini_mask_tool_response_for_history_with_threshold(
    tool_name: &str,
    call_id: &str,
    response: serde_json::Value,
    threshold: usize,
    output_dir: Option<&std::path::Path>,
) -> serde_json::Value {
    if threshold == 0 {
        return response;
    }
    let output = prodex_provider_core::gemini_provider_core_tool_response_output_string(&response);
    if output.len() <= threshold {
        return response;
    }
    let saved_path =
        runtime_gemini_save_masked_tool_output(tool_name, call_id, &output, output_dir);
    let saved_path = saved_path.as_ref().map(|path| path.display().to_string());
    prodex_provider_core::gemini_provider_core_mask_tool_response_for_history(
        response,
        threshold,
        RUNTIME_GEMINI_TOOL_OUTPUT_PREVIEW_CHARS,
        saved_path.as_deref(),
    )
}

fn runtime_gemini_save_masked_tool_output(
    tool_name: &str,
    call_id: &str,
    output: &str,
    configured_directory: Option<&std::path::Path>,
) -> Option<PathBuf> {
    let directory = configured_directory
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("prodex-gemini-tool-outputs"));
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    let tool = runtime_gemini_sanitize_file_component(tool_name);
    let call = runtime_gemini_sanitize_file_component(call_id);
    let path = directory.join(format!(
        "{millis}-{}-{}-{}.txt",
        std::process::id(),
        tool,
        call
    ));
    runtime_gemini_write_masked_tool_output(&path, output).ok()?;
    Some(path)
}

fn runtime_gemini_write_masked_tool_output(path: &Path, output: &str) -> std::io::Result<()> {
    secret_store::write_private_file_atomic(path, output.as_bytes())
}

fn runtime_gemini_sanitize_file_component(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                Some(ch)
            } else if ch.is_whitespace() || ch == '.' || ch == '/' || ch == '\\' {
                Some('_')
            } else {
                None
            }
        })
        .take(64)
        .collect::<String>();
    if sanitized.is_empty() {
        sanitized.push_str("unknown");
    }
    sanitized
}

#[cfg(all(test, unix))]
#[path = "../../../tests/src/runtime_launch/proxy_startup/gemini_request_tool_output.rs"]
mod tests;
