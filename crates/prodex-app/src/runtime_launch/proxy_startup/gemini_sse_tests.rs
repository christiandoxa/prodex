use super::{
    RuntimeDeepSeekConversationStore, RuntimeGeminiBindingRecorder, RuntimeGeminiGenerateSseReader,
};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::sync::{Arc, Mutex};

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
}

#[test]
fn gemini_sse_reader_maps_text_and_function_call_to_responses_events() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_1\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]}}]}\n\n",
        "data: {\"responseId\":\"resp_1\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"shell\",\"args\":{\"cmd\":\"ls\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
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

    assert!(output.contains("event: response.created"));
    assert!(output.contains("\"type\":\"response.output_text.delta\""));
    assert!(output.contains("\"delta\":\"hi\""));
    assert!(output.contains("\"type\":\"response.output_item.added\""));
    assert!(output.contains("\"type\":\"response.function_call_arguments.delta\""));
    assert!(output.contains("\"arguments\":\"{\\\"cmd\\\":\\\"rtk ls\\\"}\""));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_maps_apply_patch_to_custom_tool_call() {
    let patch = "*** Begin Patch\\n*** Add File: note.txt\\n+hello\\n*** End Patch";
    let stream = format!(
        "data: {{\"responseId\":\"resp_patch\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{{\"content\":{{\"parts\":[{{\"functionCall\":{{\"id\":\"call_patch\",\"name\":\"apply_patch\",\"args\":{{\"input\":\"{patch}\"}}}}}}]}},\"finishReason\":\"STOP\"}}]}}\n\n\
data: [DONE]\n\n"
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

    assert!(output.contains("\"type\":\"custom_tool_call\""));
    assert!(output.contains("\"name\":\"apply_patch\""));
    assert!(output.contains("\"call_id\":\"call_patch\""));
    assert!(!output.contains("\"type\":\"function_call\",\"call_id\":\"call_patch\""));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_maps_grounding_metadata_to_web_search_call() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_grounded\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"grounded\"}]},\"groundingMetadata\":{\"webSearchQueries\":[\"prodex gemini\"],\"groundingChunks\":[{\"web\":{\"uri\":\"https://example.com/gemini\",\"title\":\"Gemini Source\"}}]}}]}\n\n",
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

    assert!(output.contains("\"type\":\"web_search_call\""));
    assert!(output.contains("\"id\":\"ws_resp_grounded\""));
    assert!(output.contains("\"queries\":[\"prodex gemini\"]"));
    assert!(output.contains("\"url\":\"https://example.com/gemini\""));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_maps_flat_mcp_call_to_namespace_function_call() {
    let conversations = conversation_store();
    let stream = concat!(
        "data: {\"responseId\":\"resp_mcp\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"id\":\"call_sqz\",\"name\":\"mcp__prodex_sqz__compress\",\"args\":{\"text\":\"hello\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        9,
        Vec::new(),
        conversations.clone(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"namespace\":\"mcp__prodex_sqz\""));
    assert!(output.contains("\"name\":\"compress\""));
    assert!(output.contains("\"arguments\":\"{\\\"text\\\":\\\"hello\\\"}\""));
    let store = conversations.lock().unwrap();
    let assistant = store
        .get("resp_mcp")
        .and_then(|messages| messages.last())
        .expect("conversation should retain provider tool name");
    assert_eq!(
        assistant["tool_calls"][0]["function"]["name"],
        "mcp__prodex_sqz__compress"
    );
}

#[test]
fn gemini_sse_reader_maps_tool_search_function_to_tool_search_call() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_search\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"id\":\"call_search\",\"name\":\"tool_search\",\"args\":{\"query\":\"sqz tools\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
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

    assert!(output.contains("\"type\":\"tool_search_call\""));
    assert!(output.contains("\"execution\":\"client\""));
    assert!(output.contains("\"arguments\":{\"query\":\"sqz tools\"}"));
    assert!(!output.contains("\"type\":\"response.function_call_arguments.delta\""));
}

#[test]
fn gemini_sse_reader_records_response_and_tool_call_bindings() {
    let captured = Arc::new(Mutex::new(None::<(String, Vec<String>)>));
    let captured_for_recorder = Arc::clone(&captured);
    let recorder: RuntimeGeminiBindingRecorder = Arc::new(move |response_id, call_ids| {
        *captured_for_recorder.lock().unwrap() = Some((response_id, call_ids));
    });
    let stream = concat!(
        "data: {\"responseId\":\"resp_1\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"id\":\"call_1\",\"name\":\"shell\",\"args\":{\"cmd\":\"ls\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        9,
        Vec::new(),
        conversation_store(),
        Some(recorder),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    let (response_id, call_ids) = captured.lock().unwrap().clone().unwrap();
    assert_eq!(response_id, "resp_1");
    assert_eq!(call_ids, vec!["call_1"]);
}

#[test]
fn gemini_sse_reader_preserves_function_call_thought_signature_in_history() {
    let conversations = conversation_store();
    let stream = concat!(
        "data: {\"responseId\":\"resp_sig\",\"candidates\":[{\"content\":{\"parts\":[{\"thoughtSignature\":\"sig-stream-1\",\"functionCall\":{\"id\":\"call_sig\",\"name\":\"shell\",\"args\":{\"cmd\":\"ls\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        9,
        vec![serde_json::json!({"role": "user", "content": "list files"})],
        conversations.clone(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    let store = conversations.lock().unwrap();
    let history = store
        .get("resp_sig")
        .expect("conversation should be stored");
    let assistant = history.last().expect("assistant message should be stored");

    assert_eq!(
        assistant["tool_calls"][0]["gemini_thought_signature"],
        "sig-stream-1"
    );
    assert_eq!(assistant["tool_calls"][0]["id"], "call_sig");
}

#[test]
fn gemini_sse_reader_accumulates_multiline_json_data() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_1\",\"candidates\":[{\"content\":{\"parts\":[\n",
        "data: {\"text\":\"hi\"}\n",
        "data: ]},\"finishReason\":\"STOP\"}]}\n\n",
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

    assert!(output.contains("\"delta\":\"hi\""));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_maps_embedded_error_to_failed_event() {
    let stream = "data: {\"error\":{\"code\":429,\"status\":\"RESOURCE_EXHAUSTED\",\"message\":\"quota busy\"}}\n\n";
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        9,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.failed"));
    assert!(output.contains("RESOURCE_EXHAUSTED"));
    assert!(!output.contains("event: response.completed"));
}

struct ErrorAfterDataReader {
    data: std::io::Cursor<Vec<u8>>,
    errored: bool,
}

impl Read for ErrorAfterDataReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.data.read(buf)?;
        if read > 0 {
            return Ok(read);
        }
        if !self.errored {
            self.errored = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "error decoding response body",
            ));
        }
        Ok(0)
    }
}

#[test]
fn gemini_sse_reader_maps_upstream_decode_error_to_failed_event() {
    let mut reader = RuntimeGeminiGenerateSseReader::new(
            ErrorAfterDataReader {
                data: std::io::Cursor::new(
                    b"data: {\"responseId\":\"resp_1\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]}}]}\n\n"
                        .to_vec(),
                ),
                errored: false,
            },
            9,
            Vec::new(),
            conversation_store(),
            None,
        );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"delta\":\"hi\""));
    assert!(output.contains("event: response.failed"));
    assert!(output.contains("provider_stream_error"));
    assert!(!output.contains("event: response.completed"));
}
