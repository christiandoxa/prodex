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
    fn prodex_mojo_gemini_media_append_user_v1(
        abi_version: i64,
        rows_address: u64,
        row_count: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_mojo_gemini_media_request_selection_v1(
        abi_version: i64,
        operation: i64,
        slots_address: u64,
        slot_count: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_mojo_gemini_data_url_parts_v1(
        abi: i64,
        address: u64,
        length: i64,
        output: u64,
    ) -> i64;
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

/// One borrowed Serde message observation for the Mojo-owned media append target.
#[derive(Debug, Clone, Copy)]
pub struct GeminiMediaUserRow<'a> {
    pub role: Option<&'a str>,
    pub parts_is_array: bool,
}

/// Select the latest user message if and only if that very message has an
/// array of parts. Rust does not search backward for an earlier usable user.
pub fn gemini_media_append_user_target(
    messages: &[GeminiMediaUserRow<'_>],
) -> Result<Option<usize>, MojoError> {
    ensure_rich_abi()?;
    if messages.len() > 1_048_576 {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<u64>::with_capacity(messages.len() * 3);
    for message in messages {
        let role = message.role.unwrap_or("");
        rows.extend([
            role.as_ptr() as usize as u64,
            u64::try_from(role.len()).map_err(|_| MojoError::InvalidInput)?,
            u64::from(message.parts_is_array),
        ]);
    }
    let mut index = -7_i64;
    match unsafe {
        prodex_mojo_gemini_media_append_user_v1(
            GEMINI_RESPONSE_MEDIA_ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(messages.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut index as *mut i64) as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    if index == -1 {
        return Ok(None);
    }
    let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
    if !messages
        .get(index)
        .is_some_and(|message| message.parts_is_array)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(index))
}

/// Number of borrowed JSON field slots accepted by the request-media planner.
pub const GEMINI_MEDIA_REQUEST_SLOTS: usize = 22;

/// Serde field acquisition without any host-owned precedence or type decisions.
#[derive(Clone, Copy, Debug, Default)]
pub struct GeminiMediaField<'a> {
    pub present: bool,
    pub text: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiMediaRequestOperation {
    ImageUrl = 0,
    ContentObject = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiMediaSelectedKind {
    Data,
    Uri,
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeminiMediaSelected<'a> {
    pub kind: GeminiMediaSelectedKind,
    pub source: &'a str,
    pub mime: Option<&'a str>,
}

/// Mojo decides source/mime field priority without copying large media data.
/// The host only applies the typed plan to the original borrowed strings.
pub fn gemini_media_request_selection<'a>(
    fields: &[GeminiMediaField<'a>; GEMINI_MEDIA_REQUEST_SLOTS],
    operation: GeminiMediaRequestOperation,
) -> Result<Option<GeminiMediaSelected<'a>>, MojoError> {
    ensure_rich_abi()?;
    let mut rows = Vec::<u64>::with_capacity(GEMINI_MEDIA_REQUEST_SLOTS * 3);
    for field in fields {
        if field.text.is_some() && !field.present {
            return Err(MojoError::InvalidInput);
        }
        match (field.present, field.text) {
            (false, None) => rows.extend([0, 0, 0]),
            (true, None) => rows.extend([1, 0, 0]),
            (true, Some(text)) => rows.extend([
                2,
                text.as_ptr() as usize as u64,
                u64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            ]),
            (false, Some(_)) => return Err(MojoError::InvalidInput),
        }
    }
    let mut result = [-7_i64; 3];
    match unsafe {
        prodex_mojo_gemini_media_request_selection_v1(
            GEMINI_RESPONSE_MEDIA_ABI_VERSION,
            operation as i64,
            rows.as_ptr() as usize as u64,
            GEMINI_MEDIA_REQUEST_SLOTS as i64,
            result.as_mut_ptr() as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    decode_gemini_media_request_selection(fields, operation, result)
}

fn decode_gemini_media_request_selection<'a>(
    fields: &[GeminiMediaField<'a>; GEMINI_MEDIA_REQUEST_SLOTS],
    operation: GeminiMediaRequestOperation,
    result: [i64; 3],
) -> Result<Option<GeminiMediaSelected<'a>>, MojoError> {
    let [action, source, mime] = result;
    if action == 0 {
        return if source == -1 && mime == -1 {
            Ok(None)
        } else {
            Err(MojoError::InvalidOutput)
        };
    }
    let kind = match action {
        1 => GeminiMediaSelectedKind::Data,
        2 => GeminiMediaSelectedKind::Uri,
        3 => GeminiMediaSelectedKind::Path,
        _ => return Err(MojoError::InvalidOutput),
    };
    if operation == GeminiMediaRequestOperation::ImageUrl
        && (kind != GeminiMediaSelectedKind::Uri || mime != -1)
    {
        return Err(MojoError::InvalidOutput);
    }
    let source = usize::try_from(source).map_err(|_| MojoError::InvalidOutput)?;
    let source = fields
        .get(source)
        .and_then(|field| field.text)
        .ok_or(MojoError::InvalidOutput)?;
    let mime = if mime == -1 {
        None
    } else {
        Some(
            fields
                .get(usize::try_from(mime).map_err(|_| MojoError::InvalidOutput)?)
                .and_then(|field| field.text)
                .ok_or(MojoError::InvalidOutput)?,
        )
    };
    Ok(Some(GeminiMediaSelected { kind, source, mime }))
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

fn data_url_span(text: &str, start: i64, end: i64) -> Result<&str, MojoError> {
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
    text.get(start..end).ok_or(MojoError::InvalidOutput)
}

/// Borrow original MIME/data spans selected by Mojo; raw input is not trimmed.
pub fn gemini_media_data_url_parts(text: &str) -> Result<Option<(&str, &str)>, MojoError> {
    ensure_rich_abi()?;
    let mut fields = [-1_i64; 6];
    match unsafe {
        prodex_mojo_gemini_data_url_parts_v1(
            1,
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            fields.as_mut_ptr() as usize as u64,
        )
    } {
        0 => {}
        1 | 2 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let [
        present,
        default_mime,
        mime_start,
        mime_end,
        data_start,
        data_end,
    ] = fields;
    if present == 0 && fields == [0; 6] {
        return Ok(None);
    }
    if present != 1 {
        return Err(MojoError::InvalidOutput);
    }
    let mime = match default_mime {
        0 => data_url_span(text, mime_start, mime_end)?,
        1 if mime_start == 0 && mime_end == 0 => "application/octet-stream",
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(Some((mime, data_url_span(text, data_start, data_end)?)))
}

#[cfg(test)]
#[path = "gemini_media_data_url_tests.rs"]
mod data_url_tests;

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
mod media_append_user_tests {
    use super::*;

    #[test]
    fn compiled_gemini_media_append_targets_only_the_latest_user() {
        let rows = [
            GeminiMediaUserRow {
                role: Some("user"),
                parts_is_array: true,
            },
            GeminiMediaUserRow {
                role: Some("assistant"),
                parts_is_array: true,
            },
            GeminiMediaUserRow {
                role: Some("user"),
                parts_is_array: true,
            },
        ];
        assert_eq!(gemini_media_append_user_target(&rows), Ok(Some(2)));
        let mut rows = rows;
        rows[2].parts_is_array = false;
        assert_eq!(gemini_media_append_user_target(&rows), Ok(None));
        rows[2].role = Some("User");
        assert_eq!(gemini_media_append_user_target(&rows), Ok(Some(0)));
        rows[0].role = None;
        assert_eq!(gemini_media_append_user_target(&rows), Ok(None));
        assert_eq!(gemini_media_append_user_target(&[]), Ok(None));
        assert_eq!(
            gemini_media_append_user_target(&[GeminiMediaUserRow {
                role: Some("user"),
                parts_is_array: true,
            }]),
            Ok(Some(0))
        );
    }

    #[test]
    fn compiled_gemini_media_append_abi_rejects_shapes_before_output_writes() {
        let values = [0_u64; 6];
        let mut selected = 71_i64;
        let out = (&mut selected as *mut i64) as usize as u64;
        for (version, address, count, output, expected) in [
            (2, values.as_ptr() as usize as u64, 2, out, 4),
            (1, 0, 2, out, 1),
            (1, values.as_ptr() as usize as u64, -1, out, 1),
            (1, values.as_ptr() as usize as u64, 1_048_577, out, 1),
            (1, values.as_ptr() as usize as u64, 2, 0, 1),
        ] {
            assert_eq!(
                unsafe { prodex_mojo_gemini_media_append_user_v1(version, address, count, output) },
                expected
            );
            assert_eq!(selected, 71);
        }
        let mut invalid = values;
        invalid[2] = 2;
        assert_eq!(
            unsafe {
                prodex_mojo_gemini_media_append_user_v1(1, invalid.as_ptr() as usize as u64, 2, out)
            },
            1
        );
        assert_eq!(selected, 71);
    }
}

#[cfg(test)]
mod media_request_selection_tests {
    use super::*;

    fn slots<'a>() -> [GeminiMediaField<'a>; GEMINI_MEDIA_REQUEST_SLOTS] {
        [GeminiMediaField::default(); GEMINI_MEDIA_REQUEST_SLOTS]
    }

    #[test]
    fn compiled_gemini_request_media_image_selection_keeps_presence_barriers() {
        let mut fields = slots();
        fields[0] = GeminiMediaField {
            present: true,
            text: Some("direct"),
        };
        fields[1] = GeminiMediaField {
            present: true,
            text: Some("outer"),
        };
        fields[2] = GeminiMediaField {
            present: true,
            text: Some("image"),
        };
        assert_eq!(
            gemini_media_request_selection(&fields, GeminiMediaRequestOperation::ImageUrl),
            Ok(Some(GeminiMediaSelected {
                kind: GeminiMediaSelectedKind::Uri,
                source: "direct",
                mime: None,
            })),
        );
        fields[0] = GeminiMediaField::default();
        assert_eq!(
            gemini_media_request_selection(&fields, GeminiMediaRequestOperation::ImageUrl)
                .unwrap()
                .unwrap()
                .source,
            "outer",
        );
        fields[1] = GeminiMediaField {
            present: true,
            text: None,
        };
        assert_eq!(
            gemini_media_request_selection(&fields, GeminiMediaRequestOperation::ImageUrl),
            Ok(None),
        );
        fields[1] = GeminiMediaField::default();
        assert_eq!(
            gemini_media_request_selection(&fields, GeminiMediaRequestOperation::ImageUrl)
                .unwrap()
                .unwrap()
                .source,
            "image",
        );
    }

    #[test]
    fn compiled_gemini_request_media_selects_nonblank_data_then_uri_then_path() {
        let mut fields = slots();
        fields[3] = GeminiMediaField {
            present: true,
            text: Some("input_file"),
        };
        fields[6] = GeminiMediaField {
            present: true,
            text: None,
        };
        fields[7] = GeminiMediaField {
            present: true,
            text: Some("application/json"),
        };
        fields[10] = GeminiMediaField {
            present: true,
            text: Some(" \u{2003} "),
        };
        fields[11] = GeminiMediaField {
            present: true,
            text: Some("masked data"),
        };
        fields[14] = GeminiMediaField {
            present: true,
            text: Some(""),
        };
        fields[19] = GeminiMediaField {
            present: true,
            text: Some("local"),
        };
        assert_eq!(
            gemini_media_request_selection(&fields, GeminiMediaRequestOperation::ContentObject),
            Ok(Some(GeminiMediaSelected {
                kind: GeminiMediaSelectedKind::Uri,
                source: "",
                mime: None,
            })),
        );
        fields[14] = GeminiMediaField {
            present: true,
            text: None,
        };
        fields[15] = GeminiMediaField {
            present: true,
            text: Some("second URI"),
        };
        assert_eq!(
            gemini_media_request_selection(&fields, GeminiMediaRequestOperation::ContentObject),
            Ok(Some(GeminiMediaSelected {
                kind: GeminiMediaSelectedKind::Path,
                source: "local",
                mime: None,
            })),
        );
        fields[10] = GeminiMediaField {
            present: true,
            text: Some("nonblank 雪"),
        };
        fields[6] = GeminiMediaField {
            present: true,
            text: Some("image/png"),
        };
        assert_eq!(
            gemini_media_request_selection(&fields, GeminiMediaRequestOperation::ContentObject),
            Ok(Some(GeminiMediaSelected {
                kind: GeminiMediaSelectedKind::Data,
                source: "nonblank 雪",
                mime: Some("image/png"),
            })),
        );
    }

    #[test]
    fn compiled_gemini_request_media_abi_rejects_invalid_rows_before_output_writes() {
        let rows = [0_u64; GEMINI_MEDIA_REQUEST_SLOTS * 3];
        let mut result = [73_i64; 3];
        for (abi, mode, count, address, expected) in [
            (2, 0, 22, rows.as_ptr() as usize as u64, 4),
            (1, 2, 22, rows.as_ptr() as usize as u64, 1),
            (1, 0, 21, rows.as_ptr() as usize as u64, 1),
            (1, 0, 22, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_mojo_gemini_media_request_selection_v1(
                        abi,
                        mode,
                        address,
                        count,
                        result.as_mut_ptr() as usize as u64,
                    )
                },
                expected
            );
            assert_eq!(result, [73; 3]);
        }
        let mut invalid = rows;
        invalid[0] = 3;
        assert_eq!(
            unsafe {
                prodex_mojo_gemini_media_request_selection_v1(
                    1,
                    0,
                    invalid.as_ptr() as usize as u64,
                    22,
                    result.as_mut_ptr() as usize as u64,
                )
            },
            1
        );
        assert_eq!(result, [73; 3]);
        let mut invalid = rows;
        invalid[3] = 2;
        invalid[4] = 0;
        invalid[5] = 1;
        assert_eq!(
            unsafe {
                prodex_mojo_gemini_media_request_selection_v1(
                    1,
                    0,
                    invalid.as_ptr() as usize as u64,
                    22,
                    result.as_mut_ptr() as usize as u64,
                )
            },
            1
        );
        assert_eq!(result, [73; 3]);
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
