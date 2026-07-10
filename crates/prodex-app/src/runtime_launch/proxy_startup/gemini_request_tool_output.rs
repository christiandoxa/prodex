use super::gemini_request::{
    RUNTIME_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD, RUNTIME_GEMINI_TOOL_OUTPUT_PREVIEW_CHARS,
};
use std::env;
use std::fs;
use std::path::PathBuf;

pub(super) fn runtime_gemini_mask_tool_response_for_history(
    tool_name: &str,
    call_id: &str,
    response: serde_json::Value,
) -> serde_json::Value {
    runtime_gemini_mask_tool_response_for_history_with_threshold(
        tool_name,
        call_id,
        response,
        runtime_gemini_tool_output_mask_threshold(),
    )
}

fn runtime_gemini_mask_tool_response_for_history_with_threshold(
    tool_name: &str,
    call_id: &str,
    response: serde_json::Value,
    threshold: usize,
) -> serde_json::Value {
    if threshold == 0 {
        return response;
    }
    let output = prodex_provider_core::gemini_provider_core_tool_response_output_string(&response);
    if output.len() <= threshold {
        return response;
    }
    let saved_path = runtime_gemini_save_masked_tool_output(tool_name, call_id, &output);
    let saved_path = saved_path.as_ref().map(|path| path.display().to_string());
    prodex_provider_core::gemini_provider_core_mask_tool_response_for_history(
        response,
        threshold,
        RUNTIME_GEMINI_TOOL_OUTPUT_PREVIEW_CHARS,
        saved_path.as_deref(),
    )
}

fn runtime_gemini_tool_output_mask_threshold() -> usize {
    env::var("PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(RUNTIME_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD)
}

fn runtime_gemini_save_masked_tool_output(
    tool_name: &str,
    call_id: &str,
    output: &str,
) -> Option<PathBuf> {
    let directory = env::var_os("PRODEX_GEMINI_TOOL_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("prodex-gemini-tool-outputs"));
    fs::create_dir_all(&directory).ok()?;
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
    fs::write(&path, output).ok()?;
    Some(path)
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
