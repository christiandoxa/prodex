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
fn gemini_sse_reader_forces_command_output_only_final_text() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_exact\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"The command output is `forced-ok`.forced-ok\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Run printf forced-ok and answer with only the command output."
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Chunk ID: test\nOutput:\nforced-ok\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        10,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(
        output.contains("\"text\":\"forced-ok\""),
        "unexpected output: {output}"
    );
    assert!(!output.contains("The command output is"));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_forces_command_output_before_patch_diff_suffix() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_exact_patch_diff\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"marker-ok\\ndiff --git a/file b/file\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Create file with apply_patch, run cat file, and answer with only the command output."
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Chunk ID: test\nOutput:\nmarker-ok\n\ndiff --git a/file b/file\nnew file mode 100644\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        12,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(
        output.contains("\"text\":\"marker-ok\""),
        "unexpected output: {output}"
    );
    assert!(!output.contains("diff --git"));
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
fn gemini_sse_reader_only_uses_latest_user_exact_output_contract() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_latest_user\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"The earlier command printed old-output.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Run printf old-output and answer with only the command output."
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Chunk ID: old\nOutput:\nold-output\n"
        }),
        serde_json::json!({
            "role": "user",
            "content": "Explain what the earlier command did."
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        13,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("The earlier command printed old-output."));
    assert!(!output.contains("\"text\":\"old-output\""));
}

#[test]
fn gemini_sse_reader_only_uses_latest_tool_for_exact_output() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_latest_tool\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"exec_command\",\"args\":{\"cmd\":\"cat file.txt\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Create file.txt, run cat, and answer with only the command output."
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Chunk ID: old\nOutput:\nwrong-old-output\n"
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Exit code: 0\nOutput:\nSuccess. Updated the following files:\nA file.txt\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        14,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"type\":\"function_call\""));
    assert!(output.contains("\"name\":\"exec_command\""));
    assert!(!output.contains("wrong-old-output"));
}

#[test]
fn gemini_sse_reader_uses_structured_latest_tool_output_for_exact_output() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_structured_output\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"The command succeeded.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": [{"type": "input_text", "text": "Answer with only the command output."}]
        }),
        serde_json::json!({
            "role": "tool",
            "content": [{"type": "output_text", "text": "structured-ok"}]
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        15,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"text\":\"structured-ok\""));
    assert!(!output.contains("The command succeeded."));
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

    assert!(output.contains("\"call_id\":\"call_gemini_9_0\""));
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
