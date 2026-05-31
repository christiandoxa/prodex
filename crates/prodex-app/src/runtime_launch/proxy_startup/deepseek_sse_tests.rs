use super::*;
use std::sync::{Arc, Mutex};

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
}

#[test]
fn deepseek_sse_reader_stores_reasoning_content_for_tool_call_replay() {
    let conversations = conversation_store();
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_1\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"reasoning_content\":\"Need package \"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"reasoning_content\":\"metadata.\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"content\":\"I will inspect it.\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"shell\",\"arguments\":\"{\\\"cmd\\\":\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"cat package.json\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        vec![serde_json::json!({
            "role": "user",
            "content": "read package metadata"
        })],
        Arc::clone(&conversations),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("reasoning_content"));
    let stored = conversations.lock().unwrap();
    let messages = stored
        .get("chatcmpl_1")
        .expect("stream should store conversation");
    assert_eq!(messages[1]["reasoning_content"], "Need package metadata.");
    assert_eq!(messages[1]["tool_calls"][0]["id"], "call_1");
}

#[test]
fn deepseek_sse_state_stores_tool_call_snapshot_before_done_event() {
    let conversations = conversation_store();
    let mut state = RuntimeDeepSeekSseState::new(
        7,
        vec![serde_json::json!({
            "role": "user",
            "content": "read recent commits"
        })],
        Arc::clone(&conversations),
    );

    state.observe_chat_chunk(&serde_json::json!({
        "id": "chatcmpl_1",
        "model": "deepseek-v4-pro",
        "choices": [{
            "delta": {
                "reasoning_content": "Need commit history.",
                "tool_calls": [{
                    "index": 0,
                    "id": "call_1",
                    "function": {
                        "name": "shell",
                        "arguments": "{\"cmd\":\"git log"
                    }
                }]
            }
        }]
    }));
    state.observe_chat_chunk(&serde_json::json!({
        "id": "chatcmpl_1",
        "choices": [{
            "delta": {
                "tool_calls": [{
                    "index": 0,
                    "function": {
                        "arguments": " --oneline -10\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    }));

    let stored = conversations.lock().unwrap();
    let messages = stored
        .get("chatcmpl_1")
        .expect("tool-call finish should store conversation before stream done");
    assert_eq!(messages[1]["reasoning_content"], "Need commit history.");
    assert_eq!(messages[1]["tool_calls"][0]["id"], "call_1");
}

#[test]
fn deepseek_sse_reader_wraps_text_delta_in_message_item() {
    let conversations = conversation_store();
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_2\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"done\"}}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        conversations,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    let item_added = output
        .find("\"type\":\"response.output_item.added\"")
        .expect("message item should be added before text delta");
    let text_delta = output
        .find("\"type\":\"response.output_text.delta\"")
        .expect("text delta should be emitted");
    let item_done = output
        .find("\"type\":\"response.output_item.done\"")
        .expect("message item should be done before completion");
    let completed = output
        .find("\"type\":\"response.completed\"")
        .expect("response should complete");

    assert!(item_added < text_delta);
    assert!(text_delta < item_done);
    assert!(item_done < completed);
    assert!(output.contains("\"type\":\"message\""));
    assert!(output.contains("\"text\":\"done\""));
}

#[test]
fn deepseek_sse_reader_wraps_noisy_shell_call_with_rtk() {
    let conversations = conversation_store();
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_3\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"shell\",\"arguments\":\"{\\\"cmd\\\":\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_3\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"cargo test -q login\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        vec![serde_json::json!({
            "role": "user",
            "content": "run focused tests"
        })],
        Arc::clone(&conversations),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains(r#""delta":"{\"cmd\":\"rtk cargo test -q login\"}""#));
    assert!(output.contains(r#""arguments":"{\"cmd\":\"rtk cargo test -q login\"}""#));
    let stored = conversations.lock().unwrap();
    let messages = stored
        .get("chatcmpl_3")
        .expect("stream should store conversation");
    assert_eq!(
        messages[1]["tool_calls"][0]["function"]["arguments"],
        r#"{"cmd":"rtk cargo test -q login"}"#
    );
}

#[test]
fn deepseek_sse_reader_accumulates_multiline_json_data() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_4\",\"choices\":[{\"delta\":{\n",
        "data: \"content\":\"done\"\n",
        "data: },\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"delta\":\"done\""));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn deepseek_sse_reader_maps_embedded_error_to_failed_event() {
    let stream = "data: {\"error\":{\"type\":\"rate_limit_error\",\"message\":\"busy\"}}\n\n";
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.failed"));
    assert!(output.contains("rate_limit_error"));
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
fn deepseek_sse_reader_maps_upstream_decode_error_to_failed_event() {
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        ErrorAfterDataReader {
            data: std::io::Cursor::new(
                b"data: {\"id\":\"chatcmpl_5\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n"
                    .to_vec(),
            ),
            errored: false,
        },
        7,
        Vec::new(),
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"delta\":\"hi\""));
    assert!(output.contains("event: response.failed"));
    assert!(output.contains("provider_stream_error"));
    assert!(!output.contains("event: response.completed"));
}
