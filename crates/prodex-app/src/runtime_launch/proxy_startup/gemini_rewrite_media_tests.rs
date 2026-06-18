#![cfg(test)]

use super::gemini_rewrite_test_support::conversation_store;
use super::{
    runtime_deepseek_store_conversation,
    runtime_gemini_chat_assistant_messages_from_generate_value,
    runtime_gemini_generate_request_body, runtime_gemini_responses_value_from_generate_value,
};

#[test]
fn gemini_response_translation_preserves_native_code_media_cache_and_metadata() {
    let response = serde_json::json!({
        "responseId": "resp_native",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [
                {"executableCode": {"language": "PYTHON", "code": "print(2 + 2)"}},
                {"codeExecutionResult": {"outcome": "OUTCOME_OK", "output": "4"}},
                {"videoMetadata": {"startOffset": "1s", "endOffset": "3s"}},
                {"inlineData": {"mimeType": "image/png", "data": "aW1hZ2U="}}
            ]},
            "finishReason": "STOP",
            "safetyRatings": [{"category": "HARM_CATEGORY_DANGEROUS_CONTENT", "probability": "NEGLIGIBLE"}],
            "avgLogprobs": -0.25,
            "logprobsResult": {"topCandidates": [{"candidates": [{"token": "hello", "logProbability": -0.1}]}]},
            "citationMetadata": {"citations": [{"uri": "https://citation.example", "title": "Citation"}]},
            "urlContextMetadata": {"urlMetadata": [{"retrievedUrl": "https://context.example", "urlRetrievalStatus": "URL_RETRIEVAL_STATUS_SUCCESS"}]}
        }],
        "usageMetadata": {
            "promptTokenCount": 20,
            "candidatesTokenCount": 8,
            "totalTokenCount": 32,
            "cachedContentTokenCount": 12,
            "thoughtsTokenCount": 4,
            "toolUsePromptTokenCount": 3
        }
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 7);
    let output = translated["output"].as_array().unwrap();
    let message = output
        .iter()
        .find(|item| item["type"] == "message")
        .unwrap();
    let message_text = message["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let image = output
        .iter()
        .find(|item| item["type"] == "image_generation_call")
        .unwrap();
    let web_search = output
        .iter()
        .find(|item| item["type"] == "web_search_call")
        .unwrap();
    let sources = web_search["action"]["sources"].as_array().unwrap();

    assert!(message_text.contains("Gemini executable code"));
    assert!(message_text.contains("Gemini code execution result"));
    assert!(message_text.contains("Gemini video metadata"));
    assert_eq!(image["result"], "aW1hZ2U=");
    assert!(
        sources
            .iter()
            .any(|source| source["url"] == "https://citation.example")
    );
    assert!(
        sources
            .iter()
            .any(|source| source["url"] == "https://context.example")
    );
    assert_eq!(web_search["action"]["type"], "open_page");
    assert_eq!(web_search["action"]["url"], "https://citation.example");
    assert_eq!(
        translated["usage"]["input_tokens_details"]["cached_tokens"],
        12
    );
    assert_eq!(
        translated["usage"]["output_tokens_details"]["reasoning_tokens"],
        4
    );
    assert_eq!(
        translated["usage"]["input_tokens_details"]["tool_tokens"],
        3
    );
    assert_eq!(
        translated["metadata"]["gemini"]["safetyRatings"][0]["probability"],
        "NEGLIGIBLE"
    );
    assert_eq!(
        translated["metadata"]["gemini"]["urlContextMetadata"]["urlMetadata"][0]["urlRetrievalStatus"],
        "URL_RETRIEVAL_STATUS_SUCCESS"
    );
    assert_eq!(
        translated["metadata"]["gemini"]["logprobsResult"]["topCandidates"][0]["candidates"][0]["token"],
        "hello"
    );
    assert!(output.iter().any(|item| {
        item["type"] == "message"
            && item["content"][0]["text"]
                .as_str()
                .is_some_and(|text| text == "Citations:\n(Citation) https://citation.example")
    }));
    let assistant = runtime_gemini_chat_assistant_messages_from_generate_value(&response, 7);
    assert_eq!(
        assistant[0]["gemini_native_parts"][0]["videoMetadata"]["startOffset"],
        "1s"
    );
}

#[test]
fn gemini_response_keeps_non_image_media_in_followup_without_visible_base64_dump() {
    let store = conversation_store();
    let response = serde_json::json!({
        "responseId": "resp_audio",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{
                "inlineData": {"mimeType": "audio/wav", "data": "UklGRg=="}
            }]},
            "finishReason": "STOP"
        }]
    });
    let translated = runtime_gemini_responses_value_from_generate_value(&response, 7);
    let visible_text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();
    let assistant = runtime_gemini_chat_assistant_messages_from_generate_value(&response, 7);

    assert!(visible_text.contains("audio/wav"));
    assert!(!visible_text.contains("UklGRg=="));
    assert_eq!(
        assistant[0]["gemini_native_parts"][0]["inlineData"]["data"],
        "UklGRg=="
    );

    runtime_deepseek_store_conversation(
        &store,
        "resp_audio",
        vec![serde_json::json!({"role": "user", "content": "make audio"})],
        assistant,
    );
    let followup = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "previous_response_id": "resp_audio",
        "input": "Describe the audio"
    }))
    .unwrap();
    let translated =
        runtime_gemini_generate_request_body(&followup, &store, false, None, None).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let model_parts = value["contents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|content| content["role"] == "model")
        .unwrap()["parts"]
        .as_array()
        .unwrap();

    assert!(model_parts.iter().any(|part| {
        part["inlineData"]["mimeType"] == "audio/wav" && part["inlineData"]["data"] == "UklGRg=="
    }));
}
