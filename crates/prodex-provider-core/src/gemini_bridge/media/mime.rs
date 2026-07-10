//! Gemini media MIME and data-URL helpers.

pub fn gemini_provider_core_data_url_parts(image_url: &str) -> Option<(&str, &str)> {
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

pub fn gemini_provider_core_mime_type_for_uri(uri: &str) -> &'static str {
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

pub fn gemini_provider_core_mime_type_is_text(mime_type: &str) -> bool {
    mime_type.starts_with("text/")
        || matches!(
            mime_type,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/typescript"
                | "application/x-sh"
                | "application/octet-stream"
        )
}
