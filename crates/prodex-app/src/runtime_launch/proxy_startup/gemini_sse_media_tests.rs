use super::{RuntimeDeepSeekConversationStore, RuntimeGeminiGenerateSseReader};
use std::io::Read;

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    RuntimeDeepSeekConversationStore::default()
}

#[test]
fn gemini_sse_reader_preserves_media_parts_as_message_content() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_media\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"inlineData\":{\"mimeType\":\"image/png\",\"data\":\"aW1hZ2U=\"}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        9,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.output_item.done"));
    assert!(output.contains("\"type\":\"input_image\""));
    assert!(output.contains("data:image/png;base64,aW1hZ2U="));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_maps_native_code_image_cache_and_metadata() {
    let chunk = serde_json::json!({
        "responseId": "resp_native",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [
                {"executableCode": {"language": "PYTHON", "code": "print(4)"}},
                {"codeExecutionResult": {"outcome": "OUTCOME_OK", "output": "4"}},
                {"videoMetadata": {"startOffset": "1s"}},
                {"inlineData": {"mimeType": "image/png", "data": "aW1hZ2U="}}
            ]},
            "finishReason": "STOP",
            "safetyRatings": [{"category": "HARM_CATEGORY_DANGEROUS_CONTENT", "probability": "NEGLIGIBLE"}],
            "logprobsResult": {"chosenCandidates": [{"token": "done"}]},
            "citationMetadata": {"citations": [{"uri": "https://citation.example"}]}
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
    let stream = format!("data: {chunk}\n\ndata: [DONE]\n\n");
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        9,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.metadata"));
    assert!(output.contains("Gemini executable code"));
    assert!(output.contains("Gemini code execution result"));
    assert!(output.contains("Gemini video metadata"));
    assert!(output.contains("\"type\":\"image_generation_call\""));
    assert!(output.contains("\"cached_tokens\":12"));
    assert!(output.contains("\"tool_tokens\":3"));
    assert!(output.contains("\"reasoning_tokens\":4"));
    assert!(output.contains("\"logprobsResult\""));
    assert!(output.contains("Citations:\\nhttps://citation.example"));
    assert!(output.contains("https://citation.example"));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_emits_enriched_metadata_from_later_chunks() {
    let first = serde_json::json!({
        "responseId": "resp_metadata",
        "usageMetadata": {"promptTokenCount": 10}
    });
    let second = serde_json::json!({
        "responseId": "resp_metadata",
        "candidates": [{
            "content": {"parts": [{"text": "done"}]},
            "finishReason": "STOP",
            "safetyRatings": [{
                "category": "HARM_CATEGORY_DANGEROUS_CONTENT",
                "probability": "NEGLIGIBLE"
            }]
        }]
    });
    let stream = format!("data: {first}\n\ndata: {second}\n\ndata: [DONE]\n\n");
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        9,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert_eq!(output.matches("event: response.metadata").count(), 2);
    assert!(output.contains("HARM_CATEGORY_DANGEROUS_CONTENT"));
    assert!(output.contains("event: response.completed"));
}
