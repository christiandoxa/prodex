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

#[test]
fn gemini_sse_reader_maps_prompt_feedback_block_to_failed_event() {
    let stream = "data: {\"responseId\":\"resp_blocked\",\"modelVersion\":\"gemini-2.5-pro\",\"promptFeedback\":{\"blockReason\":\"SAFETY\"}}\n\n";
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
    assert!(output.contains("\"id\":\"resp_blocked\""));
    assert!(output.contains("gemini_prompt_blocked"));
    assert!(!output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_maps_empty_stop_to_failed_event() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_empty\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[]},\"finishReason\":\"STOP\"}]}\n\n",
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
    assert!(output.contains("event: response.failed"));
    assert!(output.contains("gemini_empty_response"));
    assert!(output.contains("finishReason=STOP"));
    assert!(!output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_maps_malformed_finish_to_failed_event() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_malformed\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[]},\"finishReason\":\"MALFORMED_FUNCTION_CALL\"}]}\n\n",
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

    assert!(output.contains("event: response.failed"));
    assert!(output.contains("gemini_malformed_function_call"));
    assert!(!output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_completes_reasoning_only_stop() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_reasoning_only\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"internal summary\",\"thought\":true}]},\"finishReason\":\"STOP\"}]}\n\n",
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

    assert!(output.contains("\"type\":\"response.reasoning_summary_text.delta\""));
    assert!(output.contains("event: response.completed"));
    assert!(!output.contains("gemini_empty_response"));
}

#[test]
fn gemini_sse_reader_maps_max_tokens_to_incomplete_event() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_truncated\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"partial\"}]},\"finishReason\":\"MAX_TOKENS\"}]}\n\n",
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

    assert!(output.contains("\"type\":\"response.output_text.delta\""));
    assert!(output.contains("event: response.incomplete"));
    assert!(output.contains("\"reason\":\"max_output_tokens\""));
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
        None::<RuntimeGeminiBindingRecorder>,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"delta\":\"hi\""));
    assert!(output.contains("event: response.failed"));
    assert!(output.contains("provider_stream_error"));
    assert!(!output.contains("event: response.completed"));
}
