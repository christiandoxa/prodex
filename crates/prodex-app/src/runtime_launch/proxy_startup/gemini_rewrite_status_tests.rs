use super::{
    runtime_gemini_blocked_tool_call_message, runtime_gemini_responses_value_from_generate_value,
};
use prodex_provider_core::gemini_provider_core_simple_response;

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

#[test]
fn gemini_provider_core_simple_response_accepts_prompt_feedback_block() {
    let response = serde_json::json!({
        "responseId": "resp_blocked",
        "modelVersion": "gemini-2.5-pro",
        "promptFeedback": {
            "blockReason": "SAFETY"
        }
    });

    assert!(gemini_provider_core_simple_response(
        &response,
        |name, args| { runtime_gemini_blocked_tool_call_message(name, args) }
    ));
}

#[test]
fn gemini_provider_core_simple_response_accepts_max_tokens_plain_text_candidate() {
    let response = serde_json::json!({
        "responseId": "resp_truncated",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{"text": "partial"}]},
            "finishReason": "MAX_TOKENS",
            "finishMessage": "token budget hit",
            "safetyRatings": []
        }]
    });

    assert!(gemini_provider_core_simple_response(
        &response,
        |name, args| { runtime_gemini_blocked_tool_call_message(name, args) }
    ));
}

#[test]
fn gemini_provider_core_simple_response_accepts_safe_function_call_only_candidate() {
    let response = serde_json::json!({
        "responseId": "resp_call",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{
                "thoughtSignature": "sig-response-1",
                "functionCall": {
                    "id": "call_sqz_1",
                    "name": "mcp__prodex_sqz__compress",
                    "args": {"text": "large content"}
                }
            }]}
        }]
    });

    assert!(gemini_provider_core_simple_response(
        &response,
        |name, args| { runtime_gemini_blocked_tool_call_message(name, args) }
    ));
}

#[test]
fn gemini_provider_core_simple_response_accepts_apply_patch_function_call_candidate() {
    let response = serde_json::json!({
        "responseId": "resp_patch",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{
                "functionCall": {
                    "id": "call_patch_1",
                    "name": "apply_patch",
                    "args": {"input": "*** Begin Patch\n*** End Patch"}
                }
            }]}
        }]
    });

    assert!(gemini_provider_core_simple_response(
        &response,
        |name, args| { runtime_gemini_blocked_tool_call_message(name, args) }
    ));
}

#[test]
fn gemini_provider_core_simple_response_accepts_tool_search_function_call_candidate() {
    let response = serde_json::json!({
        "responseId": "resp_search",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{
                "functionCall": {
                    "id": "call_search_1",
                    "name": "tool_search",
                    "args": {"query": "provider translator"}
                }
            }]}
        }]
    });

    assert!(gemini_provider_core_simple_response(
        &response,
        |name, args| { runtime_gemini_blocked_tool_call_message(name, args) }
    ));
}

#[test]
fn gemini_provider_core_simple_response_accepts_thought_plus_safe_function_call_candidate() {
    let response = serde_json::json!({
        "responseId": "resp_reason_call",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [
                {"text": "internal summary", "thought": true},
                {
                    "thoughtSignature": "sig-response-1",
                    "functionCall": {
                        "id": "call_sqz_1",
                        "name": "mcp__prodex_sqz__compress",
                        "args": {"text": "large content"}
                    }
                }
            ]}
        }]
    });

    assert!(gemini_provider_core_simple_response(
        &response,
        |name, args| { runtime_gemini_blocked_tool_call_message(name, args) }
    ));
}

#[test]
fn gemini_provider_core_simple_response_accepts_grounding_and_citation_metadata() {
    let response = serde_json::json!({
        "responseId": "resp_grounded",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{"text": "grounded answer"}]},
            "finishReason": "STOP",
            "citationMetadata": {
                "citations": [{"uri": "https://citation.example", "title": "Citation"}]
            },
            "groundingMetadata": {
                "groundingChunks": [{
                    "web": {"uri": "https://ground.example", "title": "Ground"}
                }]
            },
            "urlContextMetadata": {
                "urlMetadata": [{
                    "retrievedUrl": "https://context.example",
                    "urlRetrievalStatus": "URL_RETRIEVAL_STATUS_SUCCESS"
                }]
            }
        }]
    });

    assert!(gemini_provider_core_simple_response(
        &response,
        |name, args| { runtime_gemini_blocked_tool_call_message(name, args) }
    ));
}

#[test]
fn gemini_provider_core_simple_response_accepts_media_and_special_parts() {
    let response = serde_json::json!({
        "responseId": "resp_media",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [
                {"executableCode": {"language": "PYTHON", "code": "print(2 + 2)"}},
                {"codeExecutionResult": {"outcome": "OUTCOME_OK", "output": "4"}},
                {"videoMetadata": {"startOffset": "1s", "endOffset": "3s"}},
                {"inlineData": {"mimeType": "image/png", "data": "aW1hZ2U="}},
                {"fileData": {"fileUri": "https://files.example/doc.pdf", "mimeType": "application/pdf"}}
            ]},
            "finishReason": "STOP"
        }]
    });

    assert!(gemini_provider_core_simple_response(
        &response,
        |name, args| { runtime_gemini_blocked_tool_call_message(name, args) }
    ));
}

#[test]
fn gemini_provider_core_simple_response_rejects_mixed_visible_text_and_function_call() {
    let response = serde_json::json!({
        "responseId": "resp_mixed",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [
                {"text": "visible answer"},
                {"functionCall": {
                    "id": "call_sqz_1",
                    "name": "mcp__prodex_sqz__compress",
                    "args": {"text": "large content"}
                }}
            ]}
        }]
    });

    assert!(!gemini_provider_core_simple_response(
        &response,
        |name, args| { runtime_gemini_blocked_tool_call_message(name, args) }
    ));
}
