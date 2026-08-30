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
