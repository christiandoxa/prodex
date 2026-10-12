use super::{MojoError, ensure_rich_abi};
use crate::json::{JsonKernel, JsonNode, transform_json};

/// Deterministic Gemini response-part projections emitted by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiResponseMediaOperation {
    Content = 0,
    SpecialText = 1,
    ImageGeneration = 2,
}

const GEMINI_RESPONSE_MEDIA_MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;

unsafe extern "C" {
    fn prodex_mojo_gemini_media_classification_v1(
        abi_version: i64,
        operation: i64,
        address: u64,
        length: i64,
    ) -> i64;
    fn prodex_mojo_gemini_response_media_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Runs one bounded Gemini response-part projection in compiled Mojo.
pub fn gemini_response_media(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: GeminiResponseMediaOperation,
) -> Result<Option<Vec<u8>>, MojoError> {
    ensure_rich_abi()?;
    if raw.len() > GEMINI_RESPONSE_MEDIA_MAX_INPUT_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let kernel: JsonKernel = prodex_mojo_gemini_response_media_v1;
    transform_json(nodes, raw, operation as i64, false, kernel)
}

// A typed adapter is intentionally the only Rust surface for Mojo-owned MIME
// decisions. No Rust MIME suffix or text policy is recomputed on errors.
fn gemini_media_classification(operation: i64, value: &str) -> Result<i64, MojoError> {
    ensure_rich_abi()?;
    let length = i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?;
    let result = unsafe {
        prodex_mojo_gemini_media_classification_v1(
            GEMINI_RESPONSE_MEDIA_ABI_VERSION,
            operation,
            value.as_ptr() as usize as u64,
            length,
        )
    };
    match result {
        -3 => Err(MojoError::AbiMismatch),
        -2 => Err(MojoError::InvalidInput),
        output => Ok(output),
    }
}

const GEMINI_RESPONSE_MEDIA_ABI_VERSION: i64 = 1;

pub fn gemini_response_media_mime_for_uri(uri: &str) -> Result<&'static str, MojoError> {
    Ok(match gemini_media_classification(0, uri)? {
        0 => "application/octet-stream",
        1 => "image/png",
        2 => "image/jpeg",
        3 => "image/webp",
        4 => "image/gif",
        5 => "application/pdf",
        6 => "audio/mpeg",
        7 => "audio/wav",
        8 => "video/mp4",
        9 => "video/quicktime",
        _ => return Err(MojoError::InvalidOutput),
    })
}

pub fn gemini_response_media_mime_is_text(mime: &str) -> Result<bool, MojoError> {
    match gemini_media_classification(1, mime)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod media_classification_tests {
    use super::*;

    #[test]
    fn required_mojo_mime_classification_preserves_uri_and_content_boundaries() {
        for (uri, expected) in [
            ("https://example.com/picture.PNG?x=.jpg", "image/png"),
            (
                "https://example.com/path/video.MOV#fragment.mpeg",
                "video/quicktime",
            ),
            ("https://example.com/picture.JpEg", "image/jpeg"),
            ("https://example.com/file.MPEG", "audio/mpeg"),
            (
                "https://example.com/file.unknown",
                "application/octet-stream",
            ),
            (
                "https://example.com/path?asset=photo.png",
                "application/octet-stream",
            ),
            ("https://example.com/.png", "image/png"),
            ("", "application/octet-stream"),
        ] {
            assert_eq!(
                gemini_response_media_mime_for_uri(uri),
                Ok(expected),
                "{uri:?}"
            );
        }
        for (mime, expected) in [
            ("text/plain", true),
            ("text/", true),
            ("application/json", true),
            ("application/xml", true),
            ("application/javascript", true),
            ("application/typescript", true),
            ("application/x-sh", true),
            ("application/octet-stream", true),
            ("application/JSON", false),
            ("image/png", false),
            ("text", false),
            ("", false),
        ] {
            assert_eq!(
                gemini_response_media_mime_is_text(mime),
                Ok(expected),
                "{mime:?}"
            );
        }
    }
}
