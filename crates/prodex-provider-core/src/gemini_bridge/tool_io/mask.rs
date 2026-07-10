//! Gemini tool-output masking helpers.

pub fn gemini_provider_core_mask_tool_response_for_history(
    response: serde_json::Value,
    threshold: usize,
    preview_chars: usize,
    saved_path: Option<&str>,
) -> serde_json::Value {
    if threshold == 0 {
        return response;
    }
    let output = gemini_provider_core_tool_response_output_string(&response);
    if output.len() <= threshold {
        return response;
    }
    let mask = gemini_provider_core_masked_tool_output_text(&output, preview_chars, saved_path);
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

pub fn gemini_provider_core_tool_response_output_string(response: &serde_json::Value) -> String {
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

pub fn gemini_provider_core_tool_output_preview(output: &str, preview_chars: usize) -> String {
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

fn gemini_provider_core_masked_tool_output_text(
    output: &str,
    preview_chars: usize,
    saved_path: Option<&str>,
) -> String {
    let path = saved_path.unwrap_or("unavailable");
    format!(
        "[tool_output_masked]\nOriginal Gemini tool output was {} chars / {} lines and was omitted from model history.\nFull output saved to: {}\nPreview:\n{}",
        output.chars().count(),
        output.lines().count(),
        path,
        gemini_provider_core_tool_output_preview(output, preview_chars),
    )
}
