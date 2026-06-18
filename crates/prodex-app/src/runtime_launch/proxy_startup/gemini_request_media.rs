pub(in super::super) fn runtime_gemini_media_part_from_data(
    data: &str,
    mime_type: Option<&str>,
) -> Option<serde_json::Value> {
    let data = data.trim();
    if data.is_empty() {
        return None;
    }
    if let Some((data_url_mime_type, data)) = runtime_gemini_data_url_parts(data) {
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

pub(in super::super) fn runtime_gemini_media_part_from_uri_or_data_url(
    uri_or_data: &str,
    mime_type: Option<&str>,
) -> Option<serde_json::Value> {
    let uri_or_data = uri_or_data.trim();
    if uri_or_data.is_empty() {
        return None;
    }
    if let Some((data_url_mime_type, data)) = runtime_gemini_data_url_parts(uri_or_data) {
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
            "mimeType": mime_type.unwrap_or_else(|| runtime_gemini_mime_type_for_uri(uri_or_data)),
        }
    }))
}

pub(in super::super) fn runtime_gemini_data_url_parts(image_url: &str) -> Option<(&str, &str)> {
    let rest = image_url.strip_prefix("data:")?;
    let (metadata, data) = rest.split_once(',')?;
    if !metadata
        .split(';')
        .any(|segment| segment.eq_ignore_ascii_case("base64"))
    {
        return None;
    }
    let mime_type = metadata
        .split(';')
        .next()
        .filter(|mime_type| !mime_type.trim().is_empty())
        .unwrap_or("application/octet-stream");
    Some((mime_type, data))
}

pub(in super::super) fn runtime_gemini_mime_type_for_uri(uri: &str) -> &'static str {
    let uri = uri
        .split(['?', '#'])
        .next()
        .unwrap_or(uri)
        .to_ascii_lowercase();
    if uri.ends_with(".png") {
        "image/png"
    } else if uri.ends_with(".jpg") || uri.ends_with(".jpeg") {
        "image/jpeg"
    } else if uri.ends_with(".webp") {
        "image/webp"
    } else if uri.ends_with(".gif") {
        "image/gif"
    } else if uri.ends_with(".pdf") {
        "application/pdf"
    } else if uri.ends_with(".mp3") || uri.ends_with(".mpeg") {
        "audio/mpeg"
    } else if uri.ends_with(".wav") {
        "audio/wav"
    } else if uri.ends_with(".mp4") {
        "video/mp4"
    } else if uri.ends_with(".mov") {
        "video/quicktime"
    } else {
        "application/octet-stream"
    }
}
