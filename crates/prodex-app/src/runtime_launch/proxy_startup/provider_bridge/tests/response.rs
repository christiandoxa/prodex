use super::super::{RuntimeProviderBridgeKind, runtime_provider_response_conformance_result};
use prodex_provider_core::ProviderTransformLoss;

#[test]
fn response_conformance_result_uses_provider_core_translator_for_deepseek_and_gemini() {
    let deepseek = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        200,
        br#"{"id":"chatcmpl_1","model":"deepseek-chat","choices":[{"message":{"role":"assistant","content":"hi"}}],"usage":{"prompt_tokens":11,"completion_tokens":7,"total_tokens":18}}"#,
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    let deepseek_body: serde_json::Value =
        serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(deepseek_body["object"], "response");

    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"hello"}]}}],"usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":4,"totalTokenCount":7}}"#,
    )
    .unwrap();
    assert!(matches!(gemini.loss, ProviderTransformLoss::Lossless));
    let gemini_body: serde_json::Value =
        serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(gemini_body["object"], "response");
}

#[test]
fn deepseek_response_conformance_preserves_cache_and_tool_metadata() {
    let deepseek = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        200,
        br#"{"id":"chatcmpl_cache_1","model":"deepseek-chat","created":1700000000,"choices":[{"message":{"role":"assistant","content":"cached hello","reasoning_content":"think","tool_calls":[{"id":"call_1","function":{"name":"functions.exec_command","arguments":"{\"cmd\":\"echo hi\"}"},"extra_content":{"google":{"thought_signature":"sig_1"}}}]},"finish_reason":"tool_calls","logprobs":{"tokens":[]}}],"system_fingerprint":"fp_1","usage":{"prompt_tokens":11,"completion_tokens":7,"total_tokens":18,"prompt_cache_hit_tokens":5,"prompt_cache_miss_tokens":6,"completion_tokens_details":{"reasoning_tokens":2}}}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["created_at"], 1700000000);
    assert_eq!(body["output"][1]["namespace"], "functions");
    assert_eq!(body["output"][1]["name"], "exec_command");
    assert_eq!(body["output"][1]["arguments"], "{\"cmd\":\"rtk echo hi\"}");
    assert_eq!(body["output"][1]["gemini_thought_signature"], "sig_1");
    assert_eq!(
        body["usage"]["metadata"]["deepseek"]["prompt_cache_hit_tokens"],
        5
    );
    assert_eq!(body["metadata"]["deepseek"]["finish_reason"], "tool_calls");
}

#[test]
fn gemini_response_conformance_normalizes_wrapped_response_metadata() {
    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"traceId":"resp_wrapped_1","response":{"modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"wrapped hi"}]}}],"usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":4,"totalTokenCount":7,"cachedContentTokenCount":1,"thoughtsTokenCount":2,"toolUsePromptTokenCount":1}}}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["id"], "resp_wrapped_1");
    assert_eq!(body["usage"]["input_tokens_details"]["cached_tokens"], 1);
    assert_eq!(
        body["usage"]["output_tokens_details"]["reasoning_tokens"],
        2
    );
    assert_eq!(
        body["metadata"]["gemini"]["usageMetadata"]["toolUsePromptTokenCount"],
        1
    );
}

#[test]
fn gemini_response_conformance_maps_prompt_feedback_and_incomplete_status() {
    let blocked = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_blocked","modelVersion":"gemini-2.5-pro","promptFeedback":{"blockReason":"SAFETY"}}"#,
    )
    .unwrap();
    let blocked_body: serde_json::Value =
        serde_json::from_slice(blocked.body.as_ref().unwrap()).unwrap();
    assert_eq!(blocked_body["status"], "failed");
    assert_eq!(blocked_body["error"]["code"], "gemini_prompt_blocked");

    let incomplete = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_truncated","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"partial"}]},"finishReason":"MAX_TOKENS"}]}"#,
    )
    .unwrap();
    let incomplete_body: serde_json::Value =
        serde_json::from_slice(incomplete.body.as_ref().unwrap()).unwrap();
    assert_eq!(incomplete_body["status"], "incomplete");
    assert_eq!(
        incomplete_body["incomplete_details"]["reason"],
        "max_output_tokens"
    );
}

#[test]
fn gemini_response_conformance_maps_safe_function_call_only_response() {
    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_sqz_1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"thoughtSignature":"sig-response-1","functionCall":{"id":"call_sqz_1","name":"mcp__prodex_sqz__compress","args":{"text":"large content"}}}]}}]}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["output"][0]["type"], "function_call");
    assert_eq!(body["output"][0]["namespace"], "mcp__prodex_sqz");
    assert_eq!(body["output"][0]["name"], "compress");
    assert_eq!(
        body["output"][0]["gemini_thought_signature"],
        "sig-response-1"
    );
}

#[test]
fn gemini_response_conformance_maps_tool_search_and_apply_patch_function_calls() {
    let tool_search = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_search_1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"functionCall":{"id":"call_search_1","name":"tool_search","args":{"query":"provider translator"}}}]}}]}"#,
    )
    .unwrap();
    let tool_search_body: serde_json::Value =
        serde_json::from_slice(tool_search.body.as_ref().unwrap()).unwrap();
    assert_eq!(tool_search_body["output"][0]["type"], "tool_search_call");
    assert_eq!(tool_search_body["output"][0]["call_id"], "call_search_1");
    assert_eq!(
        tool_search_body["output"][0]["arguments"]["query"],
        "provider translator"
    );

    let apply_patch = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_patch_1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"functionCall":{"id":"call_patch_1","name":"apply_patch","args":{"input":"*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch"}}}]}}]}"#,
    )
    .unwrap();
    let apply_patch_body: serde_json::Value =
        serde_json::from_slice(apply_patch.body.as_ref().unwrap()).unwrap();
    assert_eq!(apply_patch_body["output"][0]["type"], "custom_tool_call");
    assert_eq!(apply_patch_body["output"][0]["name"], "apply_patch");
    assert_eq!(
        apply_patch_body["output"][0]["input"],
        "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch"
    );
}

#[test]
fn gemini_response_conformance_ignores_thought_text_in_function_call_only_response() {
    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_reason_call","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"internal summary","thought":true},{"thoughtSignature":"sig-response-1","functionCall":{"id":"call_sqz_1","name":"mcp__prodex_sqz__compress","args":{"text":"large content"}}}]}}]}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["output"].as_array().unwrap().len(), 1);
    assert_eq!(body["output"][0]["type"], "function_call");
    assert_eq!(
        body["output"][0]["gemini_thought_signature"],
        "sig-response-1"
    );
}

#[test]
fn gemini_response_conformance_maps_grounding_and_citation_outputs() {
    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_grounded","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"grounded answer"}]},"finishReason":"STOP","citationMetadata":{"citations":[{"uri":"https://citation.example","title":"Citation"}]},"groundingMetadata":{"groundingChunks":[{"web":{"uri":"https://ground.example","title":"Ground"}}]},"urlContextMetadata":{"urlMetadata":[{"retrievedUrl":"https://context.example","urlRetrievalStatus":"URL_RETRIEVAL_STATUS_SUCCESS"}]}}]}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    let output = body["output"].as_array().unwrap();
    assert!(output.iter().any(|item| item["type"] == "web_search_call"));
    assert!(output.iter().any(|item| {
        item["type"] == "message"
            && item["content"][0]["text"]
                .as_str()
                .is_some_and(|text| text == "Citations:\n(Citation) https://citation.example")
    }));
}

#[test]
fn gemini_response_conformance_maps_media_and_special_parts() {
    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_media","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"executableCode":{"language":"PYTHON","code":"print(2 + 2)"}},{"codeExecutionResult":{"outcome":"OUTCOME_OK","output":"4"}},{"videoMetadata":{"startOffset":"1s","endOffset":"3s"}},{"inlineData":{"mimeType":"image/png","data":"aW1hZ2U="}},{"fileData":{"fileUri":"https://files.example/doc.pdf","mimeType":"application/pdf"}}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":20,"candidatesTokenCount":8,"totalTokenCount":32}}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    let output = body["output"].as_array().unwrap();
    let message = output
        .iter()
        .find(|item| item["type"] == "message")
        .unwrap();
    let content = message["content"].as_array().unwrap();

    assert!(content.iter().any(|item| {
        item["text"]
            .as_str()
            .is_some_and(|text| text.contains("Gemini executable code"))
    }));
    assert!(content.iter().any(|item| {
        item["text"]
            .as_str()
            .is_some_and(|text| text.contains("Gemini code execution result"))
    }));
    assert!(content.iter().any(|item| {
        item["text"]
            .as_str()
            .is_some_and(|text| text.contains("Gemini video metadata"))
    }));
    assert!(content.iter().any(|item| item["type"] == "input_image"));
    assert!(content.iter().any(|item| {
        item["text"]
            .as_str()
            .is_some_and(|text| text.contains("application/pdf"))
    }));
    assert!(
        output
            .iter()
            .any(|item| item["type"] == "image_generation_call")
    );
}

#[test]
fn response_conformance_result_skips_non_success_status() {
    assert!(
        runtime_provider_response_conformance_result(
            RuntimeProviderBridgeKind::DeepSeek,
            429,
            br#"{"error":{"message":"too many requests"}}"#,
        )
        .is_none()
    );
}
