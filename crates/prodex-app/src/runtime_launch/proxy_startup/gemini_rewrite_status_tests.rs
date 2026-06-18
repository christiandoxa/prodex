use super::runtime_gemini_responses_value_from_generate_value;

#[test]
fn gemini_response_translation_marks_prompt_feedback_block_as_failed() {
    let response = serde_json::json!({
        "responseId": "resp_blocked",
        "modelVersion": "gemini-2.5-pro",
        "promptFeedback": {
            "blockReason": "SAFETY"
        }
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);

    assert_eq!(translated["id"], "resp_blocked");
    assert_eq!(translated["status"], "failed");
    assert_eq!(translated["error"]["code"], "gemini_prompt_blocked");
    assert_eq!(translated["output"].as_array().unwrap().len(), 0);
}

#[test]
fn gemini_response_translation_marks_malformed_finish_as_failed() {
    let response = serde_json::json!({
        "responseId": "resp_malformed",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": []},
            "finishReason": "MALFORMED_FUNCTION_CALL"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);

    assert_eq!(translated["status"], "failed");
    assert_eq!(
        translated["error"]["code"],
        "gemini_malformed_function_call"
    );
}

#[test]
fn gemini_response_translation_marks_max_tokens_as_incomplete() {
    let response = serde_json::json!({
        "responseId": "resp_truncated",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{"text": "partial"}]},
            "finishReason": "MAX_TOKENS"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);

    assert_eq!(translated["status"], "incomplete");
    assert_eq!(
        translated["incomplete_details"]["reason"],
        "max_output_tokens"
    );
    assert_eq!(translated["output"][0]["content"][0]["text"], "partial");
}

#[test]
fn gemini_response_translation_maps_safety_finish_to_invalid_prompt() {
    let response = serde_json::json!({
        "responseId": "resp_safety",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": []},
            "finishReason": "SAFETY"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);

    assert_eq!(translated["status"], "failed");
    assert_eq!(translated["error"]["code"], "invalid_prompt");
    assert!(
        translated["error"]["message"]
            .as_str()
            .unwrap()
            .contains("finishReason=SAFETY")
    );
}
