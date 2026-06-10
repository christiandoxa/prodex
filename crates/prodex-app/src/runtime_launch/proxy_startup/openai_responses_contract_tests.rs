use super::deepseek_rewrite::{RuntimeDeepSeekChatSseReader, RuntimeDeepSeekConversationStore};
use super::gemini_sse::RuntimeGeminiGenerateSseReader;
use super::local_rewrite_copilot::{
    RuntimeCopilotBindingRecorder, RuntimeCopilotResponsesSseBindingReader,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderWireFormat, runtime_provider_openai_contract,
};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::{Arc, Mutex};

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
}

fn assert_openai_responses_sse_contract(provider: &str, output: &str) {
    let mut data_events = 0usize;
    let mut terminal_events = 0usize;

    for line in output.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(data)
            .unwrap_or_else(|err| panic!("{provider} emitted invalid SSE JSON: {err}: {data}"));
        assert_no_provider_native_markers(provider, &value);
        let event_type = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{provider} SSE data lacks OpenAI Responses type: {value}"));
        assert!(
            event_type.starts_with("response."),
            "{provider} emitted non-Responses event type {event_type}: {value}"
        );
        if matches!(
            event_type,
            "response.completed" | "response.failed" | "response.incomplete"
        ) {
            terminal_events += 1;
        }
        data_events += 1;
    }

    assert!(data_events > 0, "{provider} emitted no SSE data events");
    assert!(
        terminal_events > 0,
        "{provider} emitted no terminal OpenAI Responses event: {output}"
    );
}

fn assert_no_provider_native_markers(provider: &str, value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for marker in [
                "choices",
                "candidates",
                "responseId",
                "finishReason",
                "promptFeedback",
                "usageMetadata",
                "content_block",
            ] {
                assert!(
                    !map.contains_key(marker),
                    "{provider} leaked provider-native marker {marker}: {value}"
                );
            }
            for (key, nested) in map {
                if key == "metadata" {
                    continue;
                }
                assert_no_provider_native_markers(provider, nested);
            }
        }
        serde_json::Value::Array(items) => {
            for nested in items {
                assert_no_provider_native_markers(provider, nested);
            }
        }
        _ => {}
    }
}

#[test]
fn chat_compatible_adapters_stream_openai_responses_events() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_1\",\"model\":\"provider-model\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        31,
        Vec::new(),
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert_openai_responses_sse_contract("chat-compatible adapter", &output);
}

#[test]
fn gemini_adapter_streams_openai_responses_events() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_1\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]}}]}\n\n",
        "data: {\"responseId\":\"resp_1\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"shell\",\"args\":{\"cmd\":\"ls\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        32,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert_openai_responses_sse_contract("gemini adapter", &output);
}

#[test]
fn copilot_adapter_streams_openai_responses_events_and_records_bindings() {
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured_for_recorder = Arc::clone(&captured);
    let recorder: RuntimeCopilotBindingRecorder = Arc::new(move |response_id| {
        captured_for_recorder.lock().unwrap().push(response_id);
    });
    let chat_stream = concat!(
        "data: {\"id\":\"chatcmpl_copilot_1\",\"model\":\"gpt-5.1-codex\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_copilot_1\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let chat_reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(chat_stream.as_bytes()),
        33,
        Vec::new(),
        conversation_store(),
    );
    let mut reader = RuntimeCopilotResponsesSseBindingReader::new(chat_reader, Some(recorder));
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert_openai_responses_sse_contract("copilot adapter", &output);
    assert_eq!(captured.lock().unwrap().as_slice(), ["chatcmpl_copilot_1"]);
}

#[test]
fn anthropic_adapter_declares_openai_responses_client_contract() {
    let contract = runtime_provider_openai_contract(RuntimeProviderBridgeKind::Anthropic);

    assert_eq!(
        contract.client_request_format,
        RuntimeProviderWireFormat::OpenAiResponses
    );
    assert_eq!(
        contract.upstream_request_format,
        RuntimeProviderWireFormat::OpenAiChatCompletions
    );
    assert_eq!(
        contract.response_format,
        RuntimeProviderWireFormat::OpenAiResponses
    );
    assert_eq!(contract.canonical_client_endpoint, "/v1/responses");
    assert!(contract.supports_streaming);
}
