use super::{
    RuntimeDeepSeekConversationStore, RuntimeGeminiBindingRecorder, RuntimeGeminiGenerateSseReader,
};
use std::collections::BTreeSet;
use std::io::Read;
use std::sync::{Arc, Mutex};

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    RuntimeDeepSeekConversationStore::default()
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
    let item_ids = output
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|event| event["item"]["id"].as_str().map(str::to_string))
        .filter(|id| id.starts_with("msg_gemini_"))
        .collect::<BTreeSet<_>>();
    let item_id = item_ids
        .iter()
        .next()
        .expect("message item id should be present");
    let uuid = item_id
        .strip_prefix("msg_gemini_")
        .expect("message item id should be prodex scoped")
        .parse::<prodex_domain::RequestId>()
        .unwrap();
    assert_eq!(item_ids.len(), 1);
    assert_eq!(uuid.as_uuid().get_version_num(), 7);
    assert!(!output.contains("\"id\":\"msg_gemini_9\""));
}

#[test]
fn evaluated_gemini_sse_restores_native_shell_alias_in_every_tool_call_event() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_alias\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"run_shell_command\",\"args\":{\"cmd\":\"pwd\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let config = crate::RuntimeConfig::compatibility_current();
    let mut reader = RuntimeGeminiGenerateSseReader::new_with_config(
        std::io::Cursor::new(stream.as_bytes()),
        10,
        Vec::new(),
        conversation_store(),
        None,
        None,
        prodex_provider_core::EffectiveHarnessMode::Evaluated,
        Some("gemini-3.1-pro-preview".to_string()),
        config.gemini,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"name\":\"exec_command\""));
    assert!(!output.contains("\"name\":\"run_shell_command\""));
}

#[test]
fn gemini_sse_missing_response_id_fallback_uses_request_id_uuidv7() {
    let stream = concat!(
        "data: {\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]},\"finishReason\":\"STOP\"}]}\n\n",
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

    let created = output
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .expect("created event should be valid JSON");
    let id = created["response"]["id"]
        .as_str()
        .and_then(|id| id.strip_prefix("resp_gemini_"))
        .expect("fallback Gemini SSE response id should be prodex scoped");

    assert_eq!(
        id.parse::<prodex_domain::RequestId>()
            .unwrap()
            .as_uuid()
            .get_version_num(),
        7
    );
    assert_ne!(created["response"]["id"].as_str(), Some("resp_gemini_9"));
}

#[test]
fn gemini_sse_reader_fails_tool_intent_text_without_function_call() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_tool_intent\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"I already did this.\\n\\nNow, I'll use `sqz_grep` to find the request builder.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        16,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.failed"));
    assert!(output.contains("\"code\":\"gemini_tool_intent_without_call\""));
    assert!(output.contains("sqz_grep"));
    assert!(!output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_fails_wait_poll_narration_without_wait_tool() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_wait\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"`cargo install` is still running. I will poll it.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        161,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.failed"));
    assert!(output.contains("\"code\":\"gemini_non_actionable_wait_or_poll\""));
    assert!(!output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_fails_unverified_success_after_tool_error() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_unverified\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Semua optional tools berhasil diupdate ke versi terbaru.\\n\\nBlocker/Unresolved: None.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let messages = vec![
        serde_json::json!({"role": "user", "content": "update optional tools"}),
        serde_json::json!({
            "role": "tool",
            "output": "Chunk ID: bad\nProcess exited with code 127\nOutput:\n/bin/bash: pip: No such file or directory\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        162,
        messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.failed"));
    assert!(output.contains("\"code\":\"gemini_unverified_success_claim\""));
    assert!(!output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_allows_success_after_explicit_version_verification() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_verified\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Semua optional tools berhasil diupdate ke versi terbaru.\\n\\nBlocker/Unresolved: None.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let messages = vec![
        serde_json::json!({"role": "user", "content": "update optional tools"}),
        serde_json::json!({
            "role": "tool",
            "output": "Chunk ID: ok\nProcess exited with code 0\nOutput:\nrtk 0.42.3\nsqz 1.2.0\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        163,
        messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.completed"));
    assert!(!output.contains("gemini_unverified_success_claim"));
}

#[test]
fn gemini_sse_reader_handles_long_text_stream_without_layout_events_drifting() {
    let mut stream = String::new();
    for index in 0..256 {
        let chunk = serde_json::json!({
            "responseId": "resp_long",
            "modelVersion": "gemini-2.5-pro",
            "candidates": [{
                "content": {"parts": [{"text": format!("chunk-{index};")}]}
            }]
        });
        stream.push_str("data: ");
        stream.push_str(&chunk.to_string());
        stream.push_str("\n\n");
    }
    let final_chunk = serde_json::json!({
        "responseId": "resp_long",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{"text": "done"}]},
            "finishReason": "STOP"
        }]
    });
    stream.push_str("data: ");
    stream.push_str(&final_chunk.to_string());
    stream.push_str("\n\ndata: [DONE]\n\n");

    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        10,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert_eq!(
        output
            .matches("\"type\":\"response.output_item.added\"")
            .count(),
        1
    );
    assert_eq!(
        output
            .matches("\"type\":\"response.output_text.delta\"")
            .count(),
        257
    );
    assert!(output.contains("chunk-0;"));
    assert!(output.contains("chunk-255;"));
    assert!(output.contains("\"text\":\"chunk-0;"));
    assert!(output.contains("done"));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_does_not_force_apply_patch_output_as_command_output() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_patch_then_shell\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"exec_command\",\"args\":{\"cmd\":\"cat file.txt\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Create file.txt using apply_patch. Then cat it and answer with only the command output."
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Exit code: 0\nOutput:\nSuccess. Updated the following files:\nA file.txt\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        11,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"type\":\"function_call\""));
    assert!(output.contains("\"name\":\"exec_command\""));
    assert!(!output.contains("\"text\":\"Success. Updated the following files:"));
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
fn gemini_sse_reader_merges_repeated_function_call_id_with_latest_args() {
    let partial_patch = "*** Begin Patch\n*** Add File: note.txt\n+hel";
    let final_patch = "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch";
    let first = serde_json::json!({
        "responseId": "resp_patch",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "id": "call_patch",
                        "name": "apply_patch",
                        "args": {"input": partial_patch}
                    }
                }]
            }
        }]
    });
    let second = serde_json::json!({
        "responseId": "resp_patch",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "id": "call_patch",
                        "name": "apply_patch",
                        "args": {"input": final_patch}
                    }
                }]
            },
            "finishReason": "STOP"
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

    assert_eq!(output.matches("\"type\":\"custom_tool_call\"").count(), 2);
    assert!(output.contains("*** Begin Patch\\n*** Add File: note.txt\\n+hello\\n*** End Patch"));
    assert!(!output.contains("*** Begin Patch\\n*** Add File: note.txt\\n+hel\\n*** Begin Patch"));
}

#[test]
fn gemini_sse_reader_merges_repeated_function_call_without_id() {
    let first = serde_json::json!({
        "responseId": "resp_shell",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "name": "shell",
                        "args": {"cmd": "l"}
                    }
                }]
            }
        }]
    });
    let second = serde_json::json!({
        "responseId": "resp_shell",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "name": "shell",
                        "args": {"cmd": "ls"}
                    }
                }]
            },
            "finishReason": "STOP"
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

    let call_ids = output
        .split("\"call_id\":\"")
        .skip(1)
        .filter_map(|part| part.split('"').next())
        .filter(|id| id.starts_with("call_gemini_"))
        .collect::<Vec<_>>();
    assert!(!call_ids.is_empty());
    for call_id in &call_ids {
        let uuid = call_id
            .strip_prefix("call_gemini_")
            .expect("fallback Gemini SSE call id should be prodex scoped");
        assert_eq!(
            uuid.parse::<prodex_domain::CallId>()
                .unwrap()
                .as_uuid()
                .get_version_num(),
            7
        );
    }
    assert!(call_ids.iter().all(|id| *id == call_ids[0]));
    assert!(!output.contains("\"call_id\":\"call_gemini_9_0\""));
    assert!(!output.contains("\"call_id\":\"call_gemini_9_1\""));
    assert!(output.contains("\"arguments\":\"{\\\"cmd\\\":\\\"rtk ls\\\"}\""));
}

#[test]
fn gemini_sse_reader_normalizes_unified_diff_apply_patch() {
    let unified_diff = "\
--- a/crates/prodex-cli/src/presidio.rs
+++ b/crates/prodex-cli/src/presidio.rs
@@ -1,4 +1,4 @@
-use clap::{Args, Subcommand, ValueEnum};
+use clap::{Args, Subcommand, ValueEnum, parser::ValueSource};
 use std::path::PathBuf;";
    let patch_json = serde_json::to_string(unified_diff).unwrap();
    let stream = format!(
        "data: {{\"responseId\":\"resp_patch\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{{\"content\":{{\"parts\":[{{\"functionCall\":{{\"id\":\"call_patch\",\"name\":\"apply_patch\",\"args\":{{\"input\":{patch_json}}}}}}}]}},\"finishReason\":\"STOP\"}}]}}\n\n\
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
    assert!(
        output.contains("*** Begin Patch\\n*** Update File: crates/prodex-cli/src/presidio.rs")
    );
    assert!(output.contains("\\n@@\\n-use clap::{Args, Subcommand, ValueEnum};"));
    assert!(!output.contains("--- a/crates/prodex-cli/src/presidio.rs"));
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
fn gemini_sse_reader_maps_thought_parts_to_reasoning_events() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_thought\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"plan step\",\"thought\":true},{\"text\":\"answer\"}]},\"finishReason\":\"STOP\"}]}\n\n",
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

    assert!(output.contains("\"type\":\"response.reasoning_summary_part.added\""));
    assert!(output.contains("\"type\":\"response.reasoning_summary_text.delta\""));
    assert!(output.contains("\"delta\":\"plan step\""));
    assert!(output.contains("\"type\":\"response.output_text.delta\""));
    assert!(output.contains("\"delta\":\"answer\""));
    assert!(output.contains("event: response.completed"));
}
