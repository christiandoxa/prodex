use super::super::{
    gemini_provider_core_runtime_responses_value,
    gemini_provider_core_web_search_call_from_grounding,
};

#[test]
fn gemini_provider_core_web_search_call_from_grounding_opens_retrieved_context() {
    let response = serde_json::json!({
        "candidates": [{
            "groundingMetadata": {
                "groundingChunks": [{
                    "retrievedContext": {"uri": "https://context.example"}
                }]
            }
        }]
    });
    let item = gemini_provider_core_web_search_call_from_grounding(&response, "resp_2").unwrap();
    assert_eq!(item["action"]["type"], "open_page");
    assert_eq!(item["action"]["url"], "https://context.example");
}

#[test]
fn gemini_provider_core_buffered_response_preserves_content_grounding_and_order() {
    let response = serde_json::json!({
        "responseId": "resp_grounded",
        "modelVersion": "gemini-test",
        "candidates": [{
            "content": {"parts": [
                {"text": "visible"},
                {"executableCode": {"language": "PYTHON", "code": "print(1)"}},
                {"inlineData": {"mimeType": "image/png", "data": "abc123"}},
                {"functionCall": {
                    "id": "call_tool",
                    "name": "tool",
                    "args": {"value": 1}
                }}
            ]},
            "finishReason": "STOP",
            "citationMetadata": {
                "citations": [{"title": "Source", "uri": "https://example.com/source"}]
            },
            "groundingMetadata": {
                "webSearchQueries": ["prodex gemini"],
                "groundingChunks": [{
                    "web": {"title": "Ground", "uri": "https://example.com/ground"}
                }]
            }
        }]
    });

    let value = gemini_provider_core_runtime_responses_value(
        &response,
        12,
        1234,
        "gemini-default",
        |_, _| None,
    );

    assert_eq!(value["output"][0]["type"], "message");
    assert_eq!(value["output"][0]["content"][0]["type"], "output_text");
    assert!(
        value["output"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Gemini executable code")
    );
    assert_eq!(value["output"][0]["content"][1]["type"], "input_image");
    assert_eq!(value["output"][1]["type"], "image_generation_call");
    assert_eq!(value["output"][2]["type"], "function_call");
    assert_eq!(value["output"][2]["call_id"], "call_tool");
    assert_eq!(value["output"][3]["type"], "web_search_call");
    assert_eq!(
        value["output"][4]["content"][0]["text"],
        "Citations:\n(Source) https://example.com/source"
    );
}

#[test]
fn gemini_provider_core_citation_only_response_is_not_marked_empty() {
    let response = serde_json::json!({
        "responseId": "resp_citation_only",
        "candidates": [{
            "finishReason": "STOP",
            "citationMetadata": {
                "citations": [{"uri": "https://example.com/source"}]
            }
        }]
    });

    let value = gemini_provider_core_runtime_responses_value(
        &response,
        13,
        1235,
        "gemini-default",
        |_, _| None,
    );

    assert!(value.get("status").is_none());
    assert_eq!(
        value["output"][1]["content"][0]["text"],
        "Citations:\nhttps://example.com/source"
    );
}

#[test]
fn gemini_provider_core_buffered_response_preserves_unicode_usage_and_citation_metadata() {
    let response = serde_json::json!({
        "responseId": "resp_unicode",
        "modelVersion": "gemini-test",
        "candidates": [{
            "content": {"parts": [{"text": "こんにちは 🙂"}, {"text": "tail"}]},
            "finishReason": "STOP",
            "citationMetadata": {"citations": [
                {"title": "東京", "uri": "https://example.com/tokyo"},
                {"title": "A", "uri": "https://example.com/a"},
                {"title": "A", "uri": "https://example.com/a"}
            ]}
        }],
        "usageMetadata": {
            "promptTokenCount": 11,
            "candidatesTokenCount": 2,
            "totalTokenCount": 13,
            "cachedContentTokenCount": 3,
            "thoughtsTokenCount": 1,
            "toolUsePromptTokenCount": 4
        }
    });

    let value = gemini_provider_core_runtime_responses_value(
        &response,
        14,
        1236,
        "gemini-default",
        |_, _| None,
    );

    assert_eq!(
        value,
        serde_json::json!({
            "id": "resp_unicode",
            "object": "response",
            "model": "gemini-test",
            "created_at": 1236,
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "こんにちは 🙂tail"}]
                },
                {
                    "type": "web_search_call",
                    "id": "ws_resp_unicode",
                    "status": "completed",
                    "action": {
                        "type": "open_page",
                        "url": "https://example.com/tokyo",
                        "sources": [
                            {"type": "url", "url": "https://example.com/tokyo", "title": "東京"},
                            {"type": "url", "url": "https://example.com/a", "title": "A"}
                        ]
                    }
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": "Citations:\n(A) https://example.com/a\n(東京) https://example.com/tokyo"
                    }]
                }
            ],
            "usage": {
                "input_tokens": 11,
                "input_tokens_details": {"cached_tokens": 3, "tool_tokens": 4},
                "output_tokens": 2,
                "output_tokens_details": {"reasoning_tokens": 1},
                "total_tokens": 13
            },
            "metadata": {"gemini": {
                "usageMetadata": {
                    "promptTokenCount": 11,
                    "candidatesTokenCount": 2,
                    "totalTokenCount": 13,
                    "cachedContentTokenCount": 3,
                    "thoughtsTokenCount": 1,
                    "toolUsePromptTokenCount": 4
                },
                "finishReason": "STOP",
                "citationMetadata": {"citations": [
                    {"title": "東京", "uri": "https://example.com/tokyo"},
                    {"title": "A", "uri": "https://example.com/a"},
                    {"title": "A", "uri": "https://example.com/a"}
                ]}
            }}
        })
    );
}

#[test]
fn gemini_provider_core_buffered_response_defaults_wrong_type_fields() {
    let response = serde_json::json!({
        "responseId": 8,
        "id": false,
        "modelVersion": [],
        "model": null,
        "candidates": {"content": {"parts": [{"text": "ignored"}]}},
        "usageMetadata": "wrong-type",
        "promptFeedback": ["wrong-type"]
    });

    let value = gemini_provider_core_runtime_responses_value(
        &response,
        15,
        1237,
        "gemini-default",
        |_, _| None,
    );

    assert_eq!(
        value,
        serde_json::json!({
            "id": "resp_gemini_15",
            "object": "response",
            "model": "gemini-default",
            "created_at": 1237,
            "output": [],
            "status": "failed",
            "error": {
                "code": "gemini_empty_response",
                "message": "Gemini returned no visible response content."
            },
            "usage": {
                "input_tokens": 0,
                "input_tokens_details": {"cached_tokens": 0, "tool_tokens": 0},
                "output_tokens": 0,
                "output_tokens_details": {"reasoning_tokens": 0},
                "total_tokens": 0
            },
            "metadata": {"gemini": {
                "usageMetadata": "wrong-type",
                "promptFeedback": ["wrong-type"]
            }}
        })
    );
}

#[test]
fn gemini_provider_core_buffered_response_preserves_incomplete_and_failed_statuses() {
    let incomplete = gemini_provider_core_runtime_responses_value(
        &serde_json::json!({
            "responseId": "resp_incomplete",
            "candidates": [{
                "content": {"parts": [{"text": "partial"}]},
                "finishReason": "MAX_TOKENS"
            }]
        }),
        16,
        1238,
        "gemini-default",
        |_, _| None,
    );
    assert_eq!(incomplete["status"], "incomplete");
    assert_eq!(
        incomplete["incomplete_details"],
        serde_json::json!({
            "reason": "max_output_tokens",
            "message": "Gemini stopped because it reached the maximum output token limit."
        })
    );
    assert_eq!(incomplete["output"][0]["content"][0]["text"], "partial");

    let failed = gemini_provider_core_runtime_responses_value(
        &serde_json::json!({
            "responseId": "resp_blocked",
            "promptFeedback": {"blockReason": "SAFETY"},
            "candidates": [{
                "content": {"parts": [{"text": "hidden"}]},
                "finishReason": "STOP"
            }]
        }),
        17,
        1239,
        "gemini-default",
        |_, _| None,
    );
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["error"]["code"], "gemini_prompt_blocked");
    assert_eq!(
        failed["error"]["message"],
        "Gemini blocked the prompt: SAFETY"
    );
}

#[test]
fn gemini_provider_core_buffered_response_handles_large_inputs_and_rejects_oversized_inputs() {
    let large_text = "x".repeat(5 * 1024 * 1024);
    let response = serde_json::json!({
        "responseId": "resp_large",
        "candidates": [{"content": {"parts": [{"text": large_text}]}}]
    });
    let value = gemini_provider_core_runtime_responses_value(
        &response,
        18,
        1240,
        "gemini-default",
        |_, _| None,
    );
    assert_eq!(value["output"][0]["content"][0]["text"], large_text);
    assert!(value.get("status").is_none());

    let oversized_text =
        "x".repeat(prodex_mojo_core::rich::GEMINI_BUFFERED_RESPONSE_MAX_INPUT_BYTES + 1);
    let oversized = serde_json::json!({
        "responseId": "resp_oversized",
        "candidates": [{"content": {"parts": [{"text": oversized_text}]}}]
    });
    let value = gemini_provider_core_runtime_responses_value(
        &oversized,
        19,
        1241,
        "gemini-default",
        |_, _| None,
    );
    assert_eq!(value["status"], "failed");
    assert_eq!(value["error"]["code"], "gemini_response_too_large");
    assert_eq!(value["output"], serde_json::json!([]));
}
