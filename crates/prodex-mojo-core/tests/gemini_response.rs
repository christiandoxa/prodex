#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::rich::gemini_response_conformance_preserves_response_ids;

#[test]
fn response_conformance_preserves_required_ids_in_mojo() {
    let valid = r#"{"responseId":"resp_1","candidates":[{"content":{"parts":[{"text":"ok"},{"functionCall":{"id":"call_1"}}]}}]}"#;
    assert_eq!(
        gemini_response_conformance_preserves_response_ids(valid),
        Ok(true)
    );

    let missing_response_id = r#"{"candidates":[{"content":{"parts":[]}}]}"#;
    assert_eq!(
        gemini_response_conformance_preserves_response_ids(missing_response_id),
        Ok(false)
    );

    let missing_call_id =
        r#"{"responseId":"resp_1","candidates":[{"content":{"parts":[{"functionCall":{}}]}}]}"#;
    assert_eq!(
        gemini_response_conformance_preserves_response_ids(missing_call_id),
        Ok(false)
    );

    let blank_call_id = r#"{"responseId":"resp_1","candidates":[{"content":{"parts":[{"functionCall":{"id":"\u2003"}}]}}]}"#;
    assert_eq!(
        gemini_response_conformance_preserves_response_ids(blank_call_id),
        Ok(false)
    );

    let non_object_call =
        r#"{"responseId":"resp_1","candidates":[{"content":{"parts":[{"functionCall":null}]}}]}"#;
    assert_eq!(
        gemini_response_conformance_preserves_response_ids(non_object_call),
        Ok(false)
    );
}

#[test]
fn response_conformance_mutations_change_the_mojo_result() {
    let valid = r#"{"responseId":"resp_1","candidates":[{"content":{"parts":[{"functionCall":{"id":"call_1"}}]}}]}"#;
    let missing_response_id =
        r#"{"candidates":[{"content":{"parts":[{"functionCall":{"id":"call_1"}}]}}]}"#;
    let missing_call_id =
        r#"{"responseId":"resp_1","candidates":[{"content":{"parts":[{"functionCall":{}}]}}]}"#;

    assert_eq!(
        gemini_response_conformance_preserves_response_ids(valid),
        Ok(true)
    );
    assert_eq!(
        gemini_response_conformance_preserves_response_ids(missing_response_id),
        Ok(false)
    );
    assert_eq!(
        gemini_response_conformance_preserves_response_ids(missing_call_id),
        Ok(false)
    );
}
