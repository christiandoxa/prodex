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
