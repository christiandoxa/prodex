use super::super::deepseek_sse_reader::RuntimeDeepSeekChatSseReader;
use super::{RuntimeDeepSeekConversationStore, RuntimeDeepSeekSseState};
use std::collections::BTreeSet;
use std::io::{self, Read};

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    RuntimeDeepSeekConversationStore::default()
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
        None,
        conversations.clone(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"reasoning_content\":\"Need package metadata.\""));
    assert!(output.contains("event: response.completed"));
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
        None,
        conversations.clone(),
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
        None,
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
fn deepseek_sse_missing_response_id_fallback_uses_request_id_uuidv7() {
    let stream = concat!(
        "data: {\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"done\"},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    let created = output
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .expect("created event should be valid JSON");
    let id = created["response"]["id"]
        .as_str()
        .and_then(|id| id.strip_prefix("resp_deepseek_"))
        .expect("fallback DeepSeek SSE response id should be prodex scoped");

    assert_eq!(
        id.parse::<prodex_domain::RequestId>()
            .unwrap()
            .as_uuid()
            .get_version_num(),
        7
    );
    assert_ne!(created["response"]["id"].as_str(), Some("resp_deepseek_7"));
}

#[test]
fn deepseek_sse_missing_tool_call_id_fallback_uses_call_id_uuidv7() {
    let conversations = conversation_store();
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_no_call_id\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"shell\",\"arguments\":\"{\\\"cmd\\\":\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_no_call_id\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"cargo test -q\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversations.clone(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    let generated_call_ids = output
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .flat_map(|event| {
            let mut ids = Vec::new();
            if let Some(id) = event["item"]["call_id"].as_str() {
                ids.push(id.to_string());
            }
            if let Some(id) = event["call_id"].as_str() {
                ids.push(id.to_string());
            }
            if let Some(id) = event["response"]["output"]
                .as_array()
                .and_then(|output| output.first())
                .and_then(|item| item["call_id"].as_str())
            {
                ids.push(id.to_string());
            }
            ids
        })
        .filter(|id| id.starts_with("call_deepseek_"))
        .collect::<BTreeSet<_>>();
    let call_id = generated_call_ids
        .iter()
        .next()
        .expect("fallback DeepSeek SSE call id should be present");
    let uuid = call_id
        .strip_prefix("call_deepseek_")
        .expect("fallback call id should be prodex scoped")
        .parse::<prodex_domain::CallId>()
        .unwrap();

    assert_eq!(generated_call_ids.len(), 1);
    assert_eq!(uuid.as_uuid().get_version_num(), 7);
    assert!(!output.contains("\"call_id\":\"call_deepseek_7_0\""));
    let stored = conversations.lock().unwrap();
    assert_eq!(
        stored["chatcmpl_no_call_id"][0]["tool_calls"][0]["id"],
        call_id.as_str()
    );
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
        None,
        conversations.clone(),
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
fn deepseek_sse_reader_maps_flat_mcp_call_to_namespace_function_call() {
    let conversations = conversation_store();
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_mcp\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_sqz\",\"function\":{\"name\":\"mcp__prodex_sqz__compress\",\"arguments\":\"{\\\"text\\\":\\\"hello\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversations.clone(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"namespace\":\"mcp__prodex_sqz\""));
    assert!(output.contains("\"name\":\"compress\""));
    assert!(output.contains("\"arguments\":\"{\\\"text\\\":\\\"hello\\\"}\""));
    let store = conversations.lock().unwrap();
    let assistant = store
        .get("chatcmpl_mcp")
        .and_then(|messages| messages.last())
        .expect("conversation should retain provider tool name");
    assert_eq!(
        assistant["tool_calls"][0]["function"]["name"],
        "mcp__prodex_sqz__compress"
    );
}

#[test]
fn deepseek_sse_reader_maps_tool_search_function_to_tool_search_call() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_search\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_search\",\"function\":{\"name\":\"tool_search\",\"arguments\":\"{\\\"query\\\":\\\"sqz tools\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"type\":\"tool_search_call\""));
    assert!(output.contains("\"execution\":\"client\""));
    assert!(output.contains("\"arguments\":{\"query\":\"sqz tools\"}"));
    assert!(!output.contains("\"type\":\"response.function_call_arguments.delta\""));
}

#[test]
fn deepseek_sse_reader_fails_malformed_tool_call_arguments() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_bad_args\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_shell\",\"function\":{\"name\":\"shell\",\"arguments\":\"{\\\"cmd\\\":\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.failed"));
    assert!(output.contains("invalid_tool_call_arguments"));
    assert!(output.contains("malformed JSON arguments"));
    assert!(!output.contains("event: response.completed"));
}

#[test]
fn deepseek_sse_reader_fails_tool_call_without_function_object() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_bad_tool\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_missing_function\"}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.failed"));
    assert!(output.contains("invalid_tool_call_arguments"));
    assert!(output.contains("without a function object"));
    assert!(!output.contains("event: response.completed"));
}

#[test]
fn deepseek_sse_reader_fails_tool_call_without_function_name() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_bad_name\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_missing_name\",\"function\":{\"arguments\":\"{}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.failed"));
    assert!(output.contains("invalid_tool_call_arguments"));
    assert!(output.contains("without a function name"));
    assert!(!output.contains("event: response.completed"));
}

#[test]
fn deepseek_sse_reader_maps_apply_patch_to_custom_tool_call() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_patch\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_patch\",\"function\":{\"name\":\"apply_patch\",\"arguments\":\"{\\\"input\\\":\\\"*** Begin Patch\\\\n*** Add File: note.txt\\\\n\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_patch\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"+hello\\\\n*** End Patch\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"type\":\"custom_tool_call\""));
    assert!(output.contains("\"name\":\"apply_patch\""));
    assert!(output.contains("*** Begin Patch\\n*** Add File: note.txt\\n+hello\\n*** End Patch"));
    assert!(!output.contains("\"type\":\"function_call\""));
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
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"delta\":\"done\""));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn deepseek_sse_reader_preserves_final_usage_cache_details() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_usage\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"done\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_usage\",\"choices\":[],\"usage\":{\"prompt_tokens\":20,\"completion_tokens\":5,\"total_tokens\":25,\"completion_tokens_details\":{\"reasoning_tokens\":3},\"prompt_cache_hit_tokens\":12,\"prompt_cache_miss_tokens\":8}}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"input_tokens_details\":{\"cached_tokens\":12}"));
    assert!(output.contains("\"output_tokens_details\":{\"reasoning_tokens\":3}"));
    assert!(output.contains("\"prompt_cache_miss_tokens\":8"));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn deepseek_sse_reader_preserves_logprobs_metadata() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_logprobs\",\"model\":\"deepseek-v4-pro\",\"created\":1782740700,\"system_fingerprint\":\"fp_deepseek_stream\",\"choices\":[{\"delta\":{\"content\":\"done\",\"refusal\":\"I cannot\",\"annotations\":[{\"type\":\"url_citation\",\"url\":\"https://example.com/ref\"}]},\"logprobs\":{\"content\":[{\"token\":\"done\",\"logprob\":-0.2,\"bytes\":[100,111,110,101],\"top_logprobs\":[]}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_logprobs\",\"choices\":[{\"delta\":{\"refusal\":\" help with that.\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_logprobs\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"deepseek\""));
    assert!(output.contains("\"logprobs\":{\"content\""));
    assert!(output.contains("\"token\":\"done\""));
    assert!(output.contains("\"finish_reason\":\"stop\""));
    assert!(output.contains("\"refusal\":\"I cannot help with that.\""));
    assert!(output.contains("\"annotations\":[{\"type\":\"url_citation\""));
    assert!(output.contains("\"url\":\"https://example.com/ref\""));
    assert!(output.contains("\"created_at\":1782740700"));
    assert!(output.contains("\"system_fingerprint\":\"fp_deepseek_stream\""));
}

#[test]
fn deepseek_sse_reader_merges_request_degradation_metadata() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_json\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"{\\\"answer\\\":\\\"ok\\\"}\"},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        57,
        Vec::new(),
        Some(serde_json::json!({
            "deepseek": {
                "degraded_response_format": {
                    "from": "json_schema",
                    "to": "json_object"
                }
            }
        })),
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"degraded_response_format\""));
    assert!(output.contains("\"from\":\"json_schema\""));
    assert!(output.contains("\"finish_reason\":\"stop\""));
}

#[test]
fn deepseek_sse_reader_maps_embedded_error_to_failed_event() {
    let stream = "data: {\"error\":{\"type\":\"rate_limit_error\",\"message\":\"busy\"}}\n\n";
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
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
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"delta\":\"hi\""));
    assert!(output.contains("event: response.failed"));
    assert!(output.contains("provider_stream_error"));
    assert!(!output.contains("event: response.completed"));
}
