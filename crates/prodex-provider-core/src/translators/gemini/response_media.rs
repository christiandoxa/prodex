use serde_json::{Value, json};

pub(crate) fn gemini_media_content_item_from_part(part: &Value) -> Option<Value> {
    if let Some(inline_data) = part.get("inlineData").or_else(|| part.get("inline_data")) {
        let mime_type = inline_data
            .get("mimeType")
            .or_else(|| inline_data.get("mime_type"))
            .and_then(Value::as_str)
            .unwrap_or("application/octet-stream");
        let data = inline_data.get("data").and_then(Value::as_str)?;
        if mime_type.starts_with("image/") {
            return Some(json!({
                "type": "input_image",
                "image_url": format!("data:{mime_type};base64,{data}"),
            }));
        }
        return Some(json!({
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
            .and_then(Value::as_str)?;
        let mime_type = file_data
            .get("mimeType")
            .or_else(|| file_data.get("mime_type"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| gemini_mime_type_for_uri(file_uri));
        if mime_type.starts_with("image/") {
            return Some(json!({
                "type": "input_image",
                "image_url": file_uri,
            }));
        }
        return Some(json!({
            "type": "output_text",
            "text": format!("Gemini returned {mime_type} media: {file_uri}"),
        }));
    }
    let text = part.get("text").and_then(Value::as_str)?;
    let (mime_type, data) = gemini_data_url_parts(text)?;
    if mime_type.starts_with("image/") {
        Some(json!({
            "type": "input_image",
            "image_url": format!("data:{mime_type};base64,{data}"),
        }))
    } else {
        Some(json!({
            "type": "output_text",
            "text": format!(
                "Gemini returned inline {mime_type} media ({} base64 characters).",
                data.len()
            ),
        }))
    }
}

pub(crate) fn gemini_text_from_special_part(part: &Value) -> Option<String> {
    if let Some(executable_code) = part.get("executableCode") {
        let language = executable_code
            .get("language")
            .and_then(Value::as_str)
            .unwrap_or("text");
        let code = executable_code
            .get("code")
            .and_then(Value::as_str)
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
            .and_then(Value::as_str)
            .unwrap_or("OUTCOME_UNSPECIFIED");
        let output = result
            .get("output")
            .and_then(Value::as_str)
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

pub(crate) fn gemini_image_generation_call_item_from_part(
    response_id: &str,
    index: usize,
    part: &Value,
) -> Option<Value> {
    let inline_data = part.get("inlineData").or_else(|| part.get("inline_data"))?;
    let mime_type = inline_data
        .get("mimeType")
        .or_else(|| inline_data.get("mime_type"))
        .and_then(Value::as_str)
        .unwrap_or("application/octet-stream");
    if !mime_type.starts_with("image/") {
        return None;
    }
    let data = inline_data.get("data").and_then(Value::as_str)?;
    Some(json!({
        "type": "image_generation_call",
        "id": format!("ig_{response_id}_{index}"),
        "status": "completed",
        "result": data,
    }))
}

fn gemini_data_url_parts(text: &str) -> Option<(&str, &str)> {
    let data = text.strip_prefix("data:")?;
    let (meta, body) = data.split_once(',')?;
    let meta = meta.strip_suffix(";base64")?;
    Some((meta, body))
}

fn gemini_mime_type_for_uri(uri: &str) -> &str {
    match uri
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
