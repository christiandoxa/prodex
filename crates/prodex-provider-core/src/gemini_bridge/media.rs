//! Gemini media and content-part bridge helpers.

mod content_object;
mod mime;

pub use self::content_object::{
    gemini_provider_core_image_url_value, gemini_provider_core_media_part_from_content_object,
};
pub use self::mime::{
    gemini_provider_core_data_url_parts, gemini_provider_core_mime_type_for_uri,
    gemini_provider_core_mime_type_is_text,
};

pub fn gemini_provider_core_media_part_from_data(
    data: &str,
    mime_type: Option<&str>,
) -> Option<serde_json::Value> {
    let data = data.trim();
    if data.is_empty() {
        return None;
    }
    if let Some((data_url_mime_type, data)) = gemini_provider_core_data_url_parts(data) {
        return Some(serde_json::json!({
            "inlineData": {
                "mimeType": data_url_mime_type,
                "data": data,
            }
        }));
    }
    Some(serde_json::json!({
        "inlineData": {
            "mimeType": mime_type.unwrap_or("application/octet-stream"),
            "data": data,
        }
    }))
}

pub fn gemini_provider_core_media_part_from_uri_or_data_url(
    uri_or_data: &str,
    mime_type: Option<&str>,
) -> Option<serde_json::Value> {
    let uri_or_data = uri_or_data.trim();
    if uri_or_data.is_empty() {
        return None;
    }
    if let Some((data_url_mime_type, data)) = gemini_provider_core_data_url_parts(uri_or_data) {
        return Some(serde_json::json!({
            "inlineData": {
                "mimeType": data_url_mime_type,
                "data": data,
            }
        }));
    }
    Some(serde_json::json!({
        "fileData": {
            "fileUri": uri_or_data,
            "mimeType": mime_type.unwrap_or_else(|| gemini_provider_core_mime_type_for_uri(uri_or_data)),
        }
    }))
}

pub fn gemini_provider_core_collect_media_parts(
    value: &serde_json::Value,
    parts: &mut Vec<serde_json::Value>,
    local_path_part: &mut impl FnMut(&str, Option<&str>) -> Option<serde_json::Value>,
) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                gemini_provider_core_collect_media_parts(value, parts, local_path_part);
            }
        }
        serde_json::Value::Object(object) => {
            if let Some(part) =
                gemini_provider_core_media_part_from_content_object(object, &mut *local_path_part)
            {
                parts.push(part);
            }
            if let Some(content) = object.get("content") {
                gemini_provider_core_collect_media_parts(content, parts, local_path_part);
            }
        }
        _ => {}
    }
}

pub fn gemini_provider_core_append_media_parts_to_last_user_content(
    contents: &mut Vec<serde_json::Value>,
    media_parts: Vec<serde_json::Value>,
) {
    if let Some(content) = contents
        .iter_mut()
        .rev()
        .find(|content| content.get("role").and_then(serde_json::Value::as_str) == Some("user"))
        && let Some(parts) = content
            .get_mut("parts")
            .and_then(serde_json::Value::as_array_mut)
    {
        parts.extend(media_parts);
        return;
    }
    contents.push(gemini_provider_core_content("user", media_parts));
}

pub fn gemini_provider_core_text_part(text: impl Into<String>) -> serde_json::Value {
    serde_json::json!({ "text": text.into() })
}

pub fn gemini_provider_core_local_context_text_part(
    source: impl std::fmt::Display,
    text: &str,
) -> serde_json::Value {
    gemini_provider_core_text_part(format!("Content from @{source}:\n{text}"))
}

pub fn gemini_provider_core_content(
    role: &str,
    parts: Vec<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "role": role,
        "parts": parts,
    })
}
