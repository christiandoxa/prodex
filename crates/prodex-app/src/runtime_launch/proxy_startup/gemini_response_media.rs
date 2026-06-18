use super::super::gemini_request_media::{
    runtime_gemini_data_url_parts, runtime_gemini_mime_type_for_uri,
};

pub(in super::super::super) fn runtime_gemini_media_content_item_from_part(
    part: &serde_json::Value,
) -> Option<serde_json::Value> {
    if let Some(inline_data) = part.get("inlineData").or_else(|| part.get("inline_data")) {
        let mime_type = inline_data
            .get("mimeType")
            .or_else(|| inline_data.get("mime_type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("application/octet-stream");
        let data = inline_data
            .get("data")
            .and_then(serde_json::Value::as_str)?;
        if mime_type.starts_with("image/") {
            return Some(serde_json::json!({
                "type": "input_image",
                "image_url": format!("data:{mime_type};base64,{data}"),
            }));
        }
        return Some(serde_json::json!({
            "type": "output_text",
            "text": format!(
                "Gemini returned inline {mime_type} media ({} base64 characters).",
                data.len()
            ),
        }));
    }
    if let Some(file_data) = part.get("fileData").or_else(|| part.get("file_data")) {
        let file_uri = file_data
            .get("fileUri")
            .or_else(|| file_data.get("file_uri"))
            .and_then(serde_json::Value::as_str)?;
        let mime_type = file_data
            .get("mimeType")
            .or_else(|| file_data.get("mime_type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| runtime_gemini_mime_type_for_uri(file_uri));
        if mime_type.starts_with("image/") {
            return Some(serde_json::json!({
                "type": "input_image",
                "image_url": file_uri,
            }));
        }
        return Some(serde_json::json!({
            "type": "output_text",
            "text": format!("Gemini returned {mime_type} media: {file_uri}"),
        }));
    }
    let text = part.get("text").and_then(serde_json::Value::as_str)?;
    let (mime_type, data) = runtime_gemini_data_url_parts(text)?;
    if mime_type.starts_with("image/") {
        Some(serde_json::json!({
            "type": "input_image",
            "image_url": format!("data:{mime_type};base64,{data}"),
        }))
    } else {
        Some(serde_json::json!({
            "type": "output_text",
            "text": format!(
                "Gemini returned inline {mime_type} media ({} base64 characters).",
                data.len()
            ),
        }))
    }
}

pub(in super::super::super) fn runtime_gemini_text_from_special_part(
    part: &serde_json::Value,
) -> Option<String> {
    if let Some(executable_code) = part.get("executableCode") {
        let language = executable_code
            .get("language")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("text");
        let code = executable_code
            .get("code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if code.trim().is_empty() {
            return None;
        }
        return Some(format!(
            "Gemini executable code ({language}):\n```{language}\n{code}\n```"
        ));
    }
    if let Some(result) = part.get("codeExecutionResult") {
        let outcome = result
            .get("outcome")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("OUTCOME_UNSPECIFIED");
        let output = result
            .get("output")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        return Some(format!(
            "Gemini code execution result ({outcome}):\n```text\n{output}\n```"
        ));
    }
    if let Some(video_metadata) = part.get("videoMetadata") {
        let metadata = serde_json::to_string(video_metadata).unwrap_or_else(|_| "{}".to_string());
        return Some(format!("Gemini video metadata: {metadata}"));
    }
    None
}

pub(in super::super::super) fn runtime_gemini_image_generation_call_item_from_part(
    response_id: &str,
    index: usize,
    part: &serde_json::Value,
) -> Option<serde_json::Value> {
    let inline_data = part.get("inlineData").or_else(|| part.get("inline_data"))?;
    let mime_type = inline_data
        .get("mimeType")
        .or_else(|| inline_data.get("mime_type"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("application/octet-stream");
    if !mime_type.starts_with("image/") {
        return None;
    }
    let data = inline_data
        .get("data")
        .and_then(serde_json::Value::as_str)?;
    Some(serde_json::json!({
        "type": "image_generation_call",
        "id": format!("ig_{response_id}_{index}"),
        "status": "completed",
        "result": data,
    }))
}
