//! Gemini request-media host acquisition for compiled Mojo field precedence.
//! The host borrows JSON strings and applies the selected conversion/callback.

use super::{
    gemini_provider_core_media_part_from_data, gemini_provider_core_media_part_from_uri_or_data_url,
};
use prodex_mojo_core::rich::{
    GEMINI_MEDIA_REQUEST_SLOTS, GeminiMediaField, GeminiMediaRequestOperation,
    GeminiMediaSelectedKind, gemini_media_request_selection,
};
use serde_json::{Map, Value};

fn acquired_field(value: Option<&Value>) -> GeminiMediaField<'_> {
    GeminiMediaField {
        present: value.is_some(),
        text: value.and_then(Value::as_str),
    }
}

fn acquired_object_fields<'a>(
    object: &'a Map<String, Value>,
) -> [GeminiMediaField<'a>; GEMINI_MEDIA_REQUEST_SLOTS] {
    let mut fields = [GeminiMediaField::default(); GEMINI_MEDIA_REQUEST_SLOTS];
    for (slot, key) in [
        (1, "url"),
        (2, "image_url"),
        (3, "type"),
        (6, "mime_type"),
        (7, "mimeType"),
        (8, "media_type"),
        (9, "mediaType"),
        (10, "data"),
        (11, "base64"),
        (12, "file_data"),
        (13, "fileData"),
        (14, "file_url"),
        (15, "fileUrl"),
        (16, "file_uri"),
        (17, "fileUri"),
        (18, "uri"),
        (19, "path"),
        (20, "file_path"),
        (21, "filePath"),
    ] {
        fields[slot] = acquired_field(object.get(key));
    }
    let nested = object.get("image_url");
    fields[4] = acquired_field(nested.and_then(|value| value.get("url")));
    fields[5] = acquired_field(nested.and_then(|value| value.get("image_url")));
    fields
}

pub fn gemini_provider_core_image_url_value(value: &Value) -> Option<&str> {
    let mut fields = [GeminiMediaField::default(); GEMINI_MEDIA_REQUEST_SLOTS];
    fields[0] = acquired_field(Some(value));
    fields[1] = acquired_field(value.get("url"));
    fields[2] = acquired_field(value.get("image_url"));
    gemini_media_request_selection(&fields, GeminiMediaRequestOperation::ImageUrl)
        .expect("compiled Mojo Gemini image URL selection returned invalid output")
        .map(|selected| selected.source)
}

pub fn gemini_provider_core_media_part_from_content_object(
    object: &Map<String, Value>,
    mut local_path_part: impl FnMut(&str, Option<&str>) -> Option<Value>,
) -> Option<Value> {
    let fields = acquired_object_fields(object);
    let selected =
        gemini_media_request_selection(&fields, GeminiMediaRequestOperation::ContentObject)
            .expect(
                "compiled Mojo Gemini media content-object selection returned invalid output",
            )?;
    match selected.kind {
        GeminiMediaSelectedKind::Data => {
            gemini_provider_core_media_part_from_data(selected.source, selected.mime)
        }
        GeminiMediaSelectedKind::Uri => {
            gemini_provider_core_media_part_from_uri_or_data_url(selected.source, selected.mime)
        }
        GeminiMediaSelectedKind::Path => local_path_part(selected.source, selected.mime),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compiled_image_url_selection_honors_nested_presence_and_empty_values() {
        for (value, expected) in [
            (
                json!("https://example.test/main.png"),
                Some("https://example.test/main.png"),
            ),
            (json!(""), Some("")),
            (
                json!({"url": "direct", "image_url": "secondary"}),
                Some("direct"),
            ),
            (json!({"url": null, "image_url": "secondary"}), None),
            (json!({"image_url": "secondary"}), Some("secondary")),
            (json!({"image_url": 9}), None),
            (json!({"image_url": {"url": "nested"}}), None),
            (json!({"image_url": {"image_url": "deep"}}), None),
            (json!([]), None),
            (json!(null), None),
        ] {
            assert_eq!(
                gemini_provider_core_image_url_value(&value),
                expected,
                "{value}"
            );
        }
    }

    #[test]
    fn compiled_media_object_selection_preserves_nested_image_and_data_precedence() {
        for (object, expected) in [
            (
                json!({"type":"input_image","image_url":{"url":"https://example.test/image.png"}}),
                Some(
                    json!({"fileData":{"fileUri":"https://example.test/image.png","mimeType":"image/png"}}),
                ),
            ),
            (
                json!({"type":"image_url","image_url":{"url":null,"image_url":"blocked"}, "url":"https://example.test/outer.png"}),
                Some(
                    json!({"fileData":{"fileUri":"https://example.test/outer.png","mimeType":"image/png"}}),
                ),
            ),
            (
                json!({"type":"input_image","image_url":"  ", "url":"ignored"}),
                None,
            ),
            (
                json!({"type":"input_file","data":"  ", "file_url":"https://example.test/audio.mp3"}),
                Some(
                    json!({"fileData":{"fileUri":"https://example.test/audio.mp3","mimeType":"audio/mpeg"}}),
                ),
            ),
            (
                json!({"type":"input_file","data":"雪", "mime_type":"text/plain", "file_url":"ignored"}),
                Some(json!({"inlineData":{"mimeType":"text/plain","data":"雪"}})),
            ),
            (
                json!({"type":"media","mime_type":null,"mimeType":"text/plain","data":"foo"}),
                Some(json!({"inlineData":{"mimeType":"application/octet-stream","data":"foo"}})),
            ),
            (
                json!({"type":"input_audio","uri":"", "path":"not reached"}),
                None,
            ),
            (
                json!({"type":"unknown","uri":"https://example.test/photo.png"}),
                None,
            ),
        ] {
            let mut paths = 0;
            let actual = gemini_provider_core_media_part_from_content_object(
                object.as_object().unwrap(),
                |_, _| {
                    paths += 1;
                    None
                },
            );
            assert_eq!(actual, expected, "{object}");
            assert_eq!(paths, 0, "{object}");
        }
    }

    #[test]
    fn compiled_media_object_path_callback_receives_borrowed_path_and_mime() {
        let input = json!({
            "type":"input_video",
            "path":" \\u{2003} local.mp4 ",
            "mime_type":"video/mp4",
            "file_path":"ignored",
        });
        let mut calls = 0;
        let result = gemini_provider_core_media_part_from_content_object(
            input.as_object().unwrap(),
            |path, mime| {
                calls += 1;
                assert_eq!(path, " \\u{2003} local.mp4 ");
                assert_eq!(mime, Some("video/mp4"));
                Some(json!({"local":path}))
            },
        );
        assert_eq!(calls, 1);
        assert_eq!(result, Some(json!({"local":" \\u{2003} local.mp4 "})));
        assert_eq!(
            gemini_provider_core_media_part_from_content_object(
                json!({"type":"input_file", "path":null, "file_path":"blocked"})
                    .as_object()
                    .unwrap(),
                |_, _| panic!("null first-present path must not run callback"),
            ),
            None
        );
    }
}
