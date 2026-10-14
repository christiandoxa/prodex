#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::rich::{
    gemini_response_conformance_preserves_response_ids, gemini_response_normalize_response_envelope,
};

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

#[test]
fn response_envelope_normalization_preserves_and_replaces_json_shapes() {
    let direct = r#"{"candidates":[]}"#;
    assert_eq!(
        gemini_response_normalize_response_envelope(direct).unwrap(),
        direct.as_bytes(),
    );

    let nested = r#"{"response":{"candidates":[]},"traceId":42}"#;
    assert_eq!(
        gemini_response_normalize_response_envelope(nested).unwrap(),
        br#"{"candidates":[]}"#,
    );

    let wrapped = r#"{"response":{"responseId":"stale","candidates":[]},"traceId":"trace-1"}"#;
    assert_eq!(
        gemini_response_normalize_response_envelope(wrapped).unwrap(),
        br#"{"candidates":[],"responseId":"trace-1"}"#,
    );

    let empty_trace = r#"{"response":{"candidates":[]},"traceId":""}"#;
    assert_eq!(
        gemini_response_normalize_response_envelope(empty_trace).unwrap(),
        br#"{"candidates":[],"responseId":""}"#,
    );

    let non_object = r#"{"response":[1,2],"traceId":"trace-1"}"#;
    assert_eq!(
        gemini_response_normalize_response_envelope(non_object).unwrap(),
        br#"[1,2]"#,
    );
}

#[test]
fn response_envelope_normalization_uses_raw_kernel_for_large_inputs() {
    let padding = "x".repeat(4 * 1024 * 1024 + 1);
    let raw = format!("{{\"response\":{{\"padding\":\"{padding}\"}},\"traceId\":\"trace-large\"}}");
    let normalized = gemini_response_normalize_response_envelope(&raw).unwrap();
    assert!(normalized.starts_with(br#"{"padding":"#));
    assert!(
        normalized
            .windows(br#""responseId":"trace-large""#.len())
            .any(|window| window == br#""responseId":"trace-large""#)
    );
    assert_eq!(
        normalized.len(),
        padding.len() + br#"{"padding":"","responseId":"trace-large"}"#.len()
    );
}
