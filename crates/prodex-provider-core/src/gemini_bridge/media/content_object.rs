//! Gemini media content-object conversion helpers.

use super::{
    gemini_provider_core_media_part_from_data, gemini_provider_core_media_part_from_uri_or_data_url,
};

pub fn gemini_provider_core_image_url_value(value: &serde_json::Value) -> Option<&str> {
    value.as_str().or_else(|| {
        value
            .get("url")
            .or_else(|| value.get("image_url"))
            .and_then(serde_json::Value::as_str)
    })
}

pub fn gemini_provider_core_media_part_from_content_object(
    object: &serde_json::Map<String, serde_json::Value>,
    mut local_path_part: impl FnMut(&str, Option<&str>) -> Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let kind = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match kind {
        "input_image" | "image_url" => {
            let image_url = object
                .get("image_url")
                .and_then(gemini_provider_core_image_url_value)
                .or_else(|| object.get("url").and_then(serde_json::Value::as_str))?;
            gemini_provider_core_media_part_from_uri_or_data_url(image_url, None)
        }
        "input_file" | "file" | "media" | "input_audio" | "input_video" => {
            gemini_provider_core_media_part_from_generic_content_object(
                object,
                &mut local_path_part,
            )
        }
        _ => None,
    }
}

fn gemini_provider_core_media_part_from_generic_content_object(
    object: &serde_json::Map<String, serde_json::Value>,
    local_path_part: &mut impl FnMut(&str, Option<&str>) -> Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let mime_type = object
        .get("mime_type")
        .or_else(|| object.get("mimeType"))
        .or_else(|| object.get("media_type"))
        .or_else(|| object.get("mediaType"))
        .and_then(serde_json::Value::as_str);
    if let Some(data) = object
        .get("data")
        .or_else(|| object.get("base64"))
        .or_else(|| object.get("file_data"))
        .or_else(|| object.get("fileData"))
        .and_then(serde_json::Value::as_str)
        && let Some(part) = gemini_provider_core_media_part_from_data(data, mime_type)
    {
        return Some(part);
    }
    let uri = object
        .get("file_url")
        .or_else(|| object.get("fileUrl"))
        .or_else(|| object.get("file_uri"))
        .or_else(|| object.get("fileUri"))
        .or_else(|| object.get("url"))
        .or_else(|| object.get("uri"))
        .and_then(serde_json::Value::as_str);
    if let Some(uri) = uri {
        return gemini_provider_core_media_part_from_uri_or_data_url(uri, mime_type);
    }
    let path = object
        .get("path")
        .or_else(|| object.get("file_path"))
        .or_else(|| object.get("filePath"))
        .and_then(serde_json::Value::as_str)?;
    local_path_part(path, mime_type)
}
