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
    let rows = contents
        .iter()
        .map(|content| prodex_mojo_core::rich::GeminiMediaUserRow {
            role: content.get("role").and_then(serde_json::Value::as_str),
            parts_is_array: content
                .get("parts")
                .is_some_and(serde_json::Value::is_array),
        })
        .collect::<Vec<_>>();
    if let Some(index) = prodex_mojo_core::rich::gemini_media_append_user_target(&rows)
        .expect("compiled Mojo Gemini media append selection returned invalid output")
    {
        contents[index]
            .get_mut("parts")
            .and_then(serde_json::Value::as_array_mut)
            .expect("compiled Mojo selected a valid user parts array")
            .extend(media_parts);
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

#[cfg(test)]
mod append_media_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compiled_media_append_attaches_to_latest_user_with_valid_parts() {
        let mut contents = vec![
            json!({"role":"user","parts":[{"text":"early"}]}),
            json!({"role":"assistant","parts":[{"text":"answer"}]}),
            json!({"role":"user","parts":[{"text":"last"}]}),
        ];
        gemini_provider_core_append_media_parts_to_last_user_content(
            &mut contents,
            vec![json!({"inlineData":{"mimeType":"image/png","data":"abc"}})],
        );
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[0]["parts"], json!([{"text":"early"}]));
        assert_eq!(
            contents[2]["parts"],
            json!([
                {"text":"last"},
                {"inlineData":{"mimeType":"image/png","data":"abc"}}
            ])
        );
    }

    #[test]
    fn compiled_media_append_does_not_fall_back_to_earlier_user_after_malformed_last() {
        let mut contents = vec![
            json!({"role":"user","parts":[{"text":"preserve"}]}),
            json!({"role":"assistant","parts":[]}),
            json!({"role":"user","parts":"invalid"}),
        ];
        gemini_provider_core_append_media_parts_to_last_user_content(
            &mut contents,
            vec![json!({"fileData":{"fileUri":"gs://media"}})],
        );
        assert_eq!(contents.len(), 4);
        assert_eq!(contents[0]["parts"], json!([{"text":"preserve"}]));
        assert_eq!(contents[2]["parts"], json!("invalid"));
        assert_eq!(
            contents[3],
            json!({
                "role":"user",
                "parts":[{"fileData":{"fileUri":"gs://media"}}]
            })
        );
    }

    #[test]
    fn compiled_media_append_handles_missing_user_wrong_role_case_and_empty_parts() {
        let mut contents = vec![
            json!({"role":"User","parts":[]}),
            json!({"role":"assistant","parts":[]}),
        ];
        gemini_provider_core_append_media_parts_to_last_user_content(&mut contents, vec![]);
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[2], json!({"role":"user","parts":[]}));

        let mut empty = Vec::new();
        gemini_provider_core_append_media_parts_to_last_user_content(
            &mut empty,
            vec![json!({"text":"media"})],
        );
        assert_eq!(
            empty,
            vec![json!({"role":"user","parts":[{"text":"media"}]})]
        );

        let mut last = vec![
            json!({"role":"user","parts":[]}),
            json!({"role":"user","parts":null}),
        ];
        gemini_provider_core_append_media_parts_to_last_user_content(&mut last, vec![]);
        assert_eq!(last.len(), 3);
        assert_eq!(last[0]["parts"], json!([]));
        assert_eq!(last[1]["parts"], json!(null));
    }
}
