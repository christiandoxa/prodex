//! Gemini media MIME and data-URL helpers.

use prodex_mojo_core::rich::ascii_casefold_equal_exact;

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
    prodex_mojo_core::rich::gemini_response_media_mime_for_uri(uri)
        .expect("Mojo Gemini media MIME URI classification failed")
}

pub fn gemini_provider_core_mime_type_is_text(mime_type: &str) -> bool {
    prodex_mojo_core::rich::gemini_response_media_mime_is_text(mime_type)
        .expect("Mojo Gemini media MIME text classification failed")
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
        assert_eq!(
            gemini_provider_core_mime_type_for_uri("https://example.test/photo.PNG?x=.jpg"),
            "image/png"
        );
        assert_eq!(
            gemini_provider_core_mime_type_for_uri("https://example.test/video.MOV#asset=.mp3"),
            "video/quicktime"
        );
        assert_eq!(
            gemini_provider_core_mime_type_for_uri("https://example.test/no-suffix?x=.png"),
            "application/octet-stream"
        );
        for (mime, expected) in [
            ("text/plain", true),
            ("application/json", true),
            ("application/x-sh", true),
            ("application/octet-stream", true),
            ("application/JSON", false),
            ("image/png", false),
        ] {
            assert_eq!(
                gemini_provider_core_mime_type_is_text(mime),
                expected,
                "{mime:?}"
            );
        }
    }
}
