//! Gemini media MIME and data-URL helpers.

use prodex_mojo_core::rich::{ascii_casefold_ends_with, ascii_casefold_equal_exact};

pub fn gemini_provider_core_data_url_parts(image_url: &str) -> Option<(&str, &str)> {
    let rest = image_url.strip_prefix("data:")?;
    let (metadata, data) = rest.split_once(',')?;
    if !metadata.split(';').any(|segment| {
        ascii_casefold_equal_exact(segment, "base64")
            .expect("Mojo Gemini data-URL encoding comparison failed")
    }) {
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
    let uri = uri.split(['?', '#']).next().unwrap_or(uri);
    let ends_with = |suffix| {
        ascii_casefold_ends_with(uri, suffix).expect("Mojo Gemini MIME suffix comparison failed")
    };
    if ends_with(".png") {
        "image/png"
    } else if ends_with(".jpg") || ends_with(".jpeg") {
        "image/jpeg"
    } else if ends_with(".webp") {
        "image/webp"
    } else if ends_with(".gif") {
        "image/gif"
    } else if ends_with(".pdf") {
        "application/pdf"
    } else if ends_with(".mp3") || ends_with(".mpeg") {
        "audio/mpeg"
    } else if ends_with(".wav") {
        "audio/wav"
    } else if ends_with(".mp4") {
        "video/mp4"
    } else if ends_with(".mov") {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_media_mime_casefold_uses_mojo_relations() {
        assert_eq!(
            gemini_provider_core_data_url_parts("data:image/png;BASE64,AAAA"),
            Some(("image/png", "AAAA"))
        );
        assert_eq!(
            gemini_provider_core_mime_type_for_uri("https://example.test/image.PNG?x=1"),
            "image/png"
        );
        assert_eq!(
            gemini_provider_core_mime_type_for_uri("https://example.test/audio.MPEG#fragment"),
            "audio/mpeg"
        );
        assert_eq!(
            gemini_provider_core_mime_type_for_uri("https://example.test/file.unknown"),
            "application/octet-stream"
        );
    }
}
