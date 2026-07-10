use super::gemini_request::{
    RUNTIME_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD, RUNTIME_GEMINI_TOOL_OUTPUT_PREVIEW_CHARS,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn runtime_gemini_mask_tool_response_for_history(
    tool_name: &str,
    call_id: &str,
    response: serde_json::Value,
    persist_output: bool,
) -> serde_json::Value {
    runtime_gemini_mask_tool_response_for_history_with_threshold(
        tool_name,
        call_id,
        response,
        runtime_gemini_tool_output_mask_threshold(),
        persist_output,
    )
}

fn runtime_gemini_mask_tool_response_for_history_with_threshold(
    tool_name: &str,
    call_id: &str,
    response: serde_json::Value,
    threshold: usize,
    persist_output: bool,
) -> serde_json::Value {
    if threshold == 0 {
        return response;
    }
    let output = runtime_gemini_tool_response_output_string(&response);
    if output.len() <= threshold {
        return response;
    }
    let saved_path = persist_output
        .then(|| runtime_gemini_save_masked_tool_output(tool_name, call_id, &output))
        .flatten();
    let mask = runtime_gemini_masked_tool_output_text(&output, saved_path.as_deref());
    if let serde_json::Value::Object(mut object) = response {
        object.insert("output".to_string(), serde_json::Value::String(mask));
        object.insert("_prodex_masked".to_string(), serde_json::Value::Bool(true));
        return serde_json::Value::Object(object);
    }
    serde_json::json!({
        "output": mask,
        "_prodex_masked": true,
    })
}

fn runtime_gemini_tool_output_mask_threshold() -> usize {
    env::var("PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(RUNTIME_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD)
}

fn runtime_gemini_tool_response_output_string(response: &serde_json::Value) -> String {
    response
        .get("output")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            response
                .get("content")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            serde_json::to_string_pretty(response).unwrap_or_else(|_| response.to_string())
        })
}

fn runtime_gemini_masked_tool_output_text(output: &str, saved_path: Option<&Path>) -> String {
    let path = saved_path
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    format!(
        "[tool_output_masked]\nOriginal Gemini tool output was {} chars / {} lines and was omitted from model history.\nFull output saved to: {}\nPreview:\n{}",
        output.chars().count(),
        output.lines().count(),
        path,
        runtime_gemini_tool_output_preview(output),
    )
}

fn runtime_gemini_tool_output_preview(output: &str) -> String {
    let preview_chars = RUNTIME_GEMINI_TOOL_OUTPUT_PREVIEW_CHARS;
    let char_count = output.chars().count();
    if char_count <= preview_chars {
        return output.to_string();
    }
    let head_len = preview_chars / 2;
    let tail_len = preview_chars.saturating_sub(head_len);
    let head = output.chars().take(head_len).collect::<String>();
    let tail = output
        .chars()
        .rev()
        .take(tail_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{head}\n...\n{tail}")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_gateway_tool_output_is_not_persisted() {
        let masked = runtime_gemini_mask_tool_response_for_history_with_threshold(
            "shell",
            "call-1",
            serde_json::json!({"output": "sensitive tenant output"}),
            1,
            false,
        );

        assert_eq!(masked["_prodex_masked"], true);
        assert!(masked["output"].as_str().unwrap().contains("unavailable"));
    }
}
