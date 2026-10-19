use super::*;
use crate::{
    gemini_provider_core_media_part_from_data, gemini_provider_core_media_part_from_uri_or_data_url,
};
use serde_json::json;

#[test]
fn gemini_data_url_grammar_reaches_real_media_callers_without_changing_json() {
    for (input, mime, data) in [
        ("data:image/png;BASE64,AA==", "image/png", "AA=="),
        ("data:base64,AA==", "base64", "AA=="),
        ("data:;base64,", "application/octet-stream", ""),
        (
            "data: \u{2003};BaSe64,x,y\0🧭",
            "application/octet-stream",
            "x,y\0🧭",
        ),
        (
            "data:画像/png;charset=utf-8;base64,AA==",
            "画像/png",
            "AA==",
        ),
    ] {
        assert_eq!(
            gemini_provider_core_data_url_parts(input),
            Some((mime, data))
        );
        let expected = Some(json!({"inlineData": {"mimeType": mime, "data": data}}));
        assert_eq!(
            gemini_provider_core_media_part_from_data(input, Some("ignored/mime")),
            expected
        );
        assert_eq!(
            gemini_provider_core_media_part_from_uri_or_data_url(input, None),
            expected
        );
    }
}

#[test]
fn gemini_data_url_callers_keep_outer_trimming_and_opaque_nonbase64_data() {
    let input = " \u{2003}data:image/png;base64,AA== \u{3000}";
    assert_eq!(gemini_provider_core_data_url_parts(input), None);
    assert_eq!(
        gemini_provider_core_media_part_from_data(input, None),
        Some(json!({"inlineData":{"mimeType":"image/png","data":"AA=="}}))
    );
    let input = "data:text/plain;charset=utf-8,opaque";
    assert_eq!(gemini_provider_core_data_url_parts(input), None);
    assert_eq!(
        gemini_provider_core_media_part_from_data(input, Some("custom/mime")),
        Some(json!({"inlineData":{"mimeType":"custom/mime","data":input}}))
    );
    assert_eq!(
        gemini_provider_core_media_part_from_uri_or_data_url(input, None),
        Some(json!({"fileData":{"mimeType":"application/octet-stream","fileUri":input}}))
    );
}
