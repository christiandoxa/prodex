use super::{RuntimeDeepSeekConversationStore, RuntimeGeminiGenerateSseReader};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::{Arc, Mutex};

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
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
fn gemini_sse_reader_does_not_force_exploratory_output_before_required_command() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_required_command\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"exec_command\",\"args\":{\"cmd\":\"./demo-optimizer/install.sh && ./bin/demo-optimizer --version\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Update demo-optimizer.\nAfter updating, run exactly: ./bin/demo-optimizer --version\nAnswer with only the command output."
        }),
        serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "type": "function",
                "function": {
                    "name": "exec_command",
                    "arguments": "{\"cmd\":\"cat OWNER.txt\"}"
                }
            }]
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Chunk ID: owner\nOutput:\ndemo-optimizer is owned by ./demo-optimizer/install.sh\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        151,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"type\":\"function_call\""));
    assert!(output.contains("./demo-optimizer/install.sh"));
    assert!(!output.contains("\"text\":\"demo-optimizer is owned by"));
}

#[test]
fn gemini_sse_reader_drops_command_output_only_text_before_required_tool_call() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_required_command_text_before_tool\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Tool discipline for Codex parity leaked text\"},{\"functionCall\":{\"name\":\"exec_command\",\"args\":{\"cmd\":\"./bin/demo-optimizer --version\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Update demo-optimizer.\nAfter updating, run exactly: ./bin/demo-optimizer --version\nAnswer with only the command output."
        }),
        serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "type": "function",
                "function": {
                    "name": "exec_command",
                    "arguments": "{\"cmd\":\"cat OWNER.txt\"}"
                }
            }]
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Chunk ID: owner\nOutput:\ndemo-optimizer is owned by ./demo-optimizer/install.sh\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        154,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"type\":\"function_call\""));
    assert!(output.contains("./bin/demo-optimizer --version"));
    assert!(!output.contains("Tool discipline for Codex parity"));
}

#[test]
fn gemini_sse_reader_forces_required_command_output_after_matching_command() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_required_command_done\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"done\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Update demo-optimizer.\nAfter updating, run exactly: ./bin/demo-optimizer --version\nAnswer with only the command output."
        }),
        serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "type": "function",
                "function": {
                    "name": "exec_command",
                    "arguments": "{\"cmd\":\"./demo-optimizer/install.sh && ./bin/demo-optimizer --version\"}"
                }
            }]
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Chunk ID: version\nOutput:\nPRODEX_GEMINI_LIVE_OK\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        152,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"text\":\"PRODEX_GEMINI_LIVE_OK\""));
    assert!(!output.contains("\"delta\":\"done\""));
}

#[test]
fn gemini_sse_reader_forces_then_run_cat_output_after_matching_command() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_then_run_done\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"You are Codex CLI. Tool discipline for Codex parity. leaked text\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Create file gemini-patch-smoke.txt containing exactly PRODEX_GEMINI_LIVE_OK using apply_patch. Then run cat gemini-patch-smoke.txt. Answer with only the command output."
        }),
        serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "type": "function",
                "function": {
                    "name": "exec_command",
                    "arguments": "{\"cmd\":\"cat gemini-patch-smoke.txt\"}"
                }
            }]
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Chunk ID: cat\nOutput:\nPRODEX_GEMINI_LIVE_OK\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        153,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"text\":\"PRODEX_GEMINI_LIVE_OK\""));
    assert!(!output.contains("You are Codex CLI"));
    assert!(!output.contains("Tool discipline for Codex parity"));
}

#[test]
fn gemini_sse_reader_forces_verification_command_output_by_shared_marker() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_marker_command_done\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"done\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Read gemini-smoke.txt from the workspace, keep the existing file unchanged, then run this verification command:\nnode -e \"const fs=require('fs'); process.stdout.write('PRODEX_GEMINI_LIVE_OK_123');\"\nAnswer with only the command output."
        }),
        serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "type": "function",
                "function": {
                    "name": "exec_command",
                    "arguments": "{\"cmd\":\"node -e \\\"process.stdout.write('PRODEX_GEMINI_LIVE_OK_123');\\\"\"}"
                }
            }]
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Chunk ID: node\nOutput:\nPRODEX_GEMINI_LIVE_OK_123\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        155,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"text\":\"PRODEX_GEMINI_LIVE_OK_123\""));
    assert!(!output.contains("\"delta\":\"done\""));
}

#[test]
fn gemini_sse_reader_forces_verification_output_when_command_metadata_is_compacted() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_compacted_command_done\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"done\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let conversation_messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Read gemini-smoke.txt from the workspace, keep the existing file unchanged, then run this verification command:\nnode -e \"const fs=require('fs'); process.stdout.write('PRODEX_GEMINI_LIVE_OK_456');\"\nAnswer with only the command output."
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Chunk ID: node\nOutput:\nPRODEX_GEMINI_LIVE_OK_456\n"
        }),
    ];
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        156,
        conversation_messages,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"text\":\"PRODEX_GEMINI_LIVE_OK_456\""));
    assert!(!output.contains("\"delta\":\"done\""));
}
