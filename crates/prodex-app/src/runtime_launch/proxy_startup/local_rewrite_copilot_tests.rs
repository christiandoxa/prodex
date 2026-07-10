#![cfg(test)]

use super::super::copilot_instructions::{
    RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_HEADER, runtime_copilot_workspace_custom_instructions,
};
use super::super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekTranslatedRequest,
};
use super::*;
use std::io::{Cursor, Read};

fn copilot_profile(profile_name: &str) -> RuntimeCopilotProfileAuth {
    RuntimeCopilotProfileAuth {
        profile_name: profile_name.to_string(),
        api_key: format!("token-{profile_name}"),
        api_url: format!("https://api.{profile_name}.githubcopilot.test"),
        model_catalog: Vec::new(),
    }
}

fn copilot_pool(profile_names: &[&str]) -> RuntimeCopilotOAuthPool {
    RuntimeCopilotOAuthPool {
        state: Arc::new(Mutex::new(RuntimeCopilotOAuthPoolState {
            profiles: profile_names
                .iter()
                .map(|profile_name| copilot_profile(profile_name))
                .collect(),
            next_index: 0,
            response_profile_bindings: BTreeMap::new(),
        })),
    }
}

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    RuntimeDeepSeekConversationStore::default()
}

fn temp_copilot_instruction_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "prodex-copilot-instructions-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn copilot_workspace_custom_instructions_reads_github_files_only() {
    let root = temp_copilot_instruction_root("github-only");
    std::fs::create_dir_all(root.join(".github/instructions/nested")).unwrap();
    std::fs::write(
        root.join(".github/copilot-instructions.md"),
        "Prefer concise answers.",
    )
    .unwrap();
    std::fs::write(
        root.join(".github/instructions/nested/review.instructions.md"),
        "Review risky diffs first.",
    )
    .unwrap();
    std::fs::write(root.join("AGENTS.md"), "@/home/doxa/.prodex/private/RTK.md").unwrap();

    let instructions = runtime_copilot_workspace_custom_instructions(&root)
        .unwrap()
        .unwrap();

    assert!(instructions.contains("## .github/copilot-instructions.md"));
    assert!(instructions.contains("Prefer concise answers."));
    assert!(instructions.contains("## .github/instructions/nested/review.instructions.md"));
    assert!(instructions.contains("Review risky diffs first."));
    assert!(!instructions.contains("AGENTS.md"));
    assert!(!instructions.contains(".prodex/private"));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn copilot_workspace_custom_instructions_do_not_follow_symlinks() {
    let root = temp_copilot_instruction_root("symlink");
    let outside = root.with_file_name(format!(
        "{}-outside",
        root.file_name().unwrap().to_string_lossy()
    ));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(root.join(".github")).unwrap();
    std::fs::create_dir_all(outside.join("instructions")).unwrap();
    std::fs::write(
        outside.join("copilot-instructions.md"),
        "outside global secret",
    )
    .unwrap();
    std::fs::write(
        outside.join("instructions").join("leak.instructions.md"),
        "outside scoped secret",
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.join("copilot-instructions.md"),
        root.join(".github").join("copilot-instructions.md"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.join("instructions"),
        root.join(".github").join("instructions"),
    )
    .unwrap();

    let instructions = runtime_copilot_workspace_custom_instructions(&root).unwrap();

    assert_eq!(instructions, None);
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside);
}

#[test]
fn copilot_custom_instructions_merge_into_chat_body() {
    let mut translated = RuntimeDeepSeekTranslatedRequest {
        body: serde_json::to_vec(&serde_json::json!({
            "model": "gpt-5.1-codex",
            "stream": true,
            "messages": [
                {"role": "system", "content": "Existing system."},
                {"role": "user", "content": "Hi"}
            ]
        }))
        .unwrap(),
        messages: vec![
            serde_json::json!({"role": "system", "content": "Existing system."}),
            serde_json::json!({"role": "user", "content": "Hi"}),
        ],
        response_metadata: None,
    };

    runtime_copilot_apply_custom_instructions(&mut translated, "Prefer Rust tests.").unwrap();

    let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("Existing system."));
    assert!(system.contains(RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_HEADER));
    assert!(system.contains("Prefer Rust tests."));
    assert_eq!(
        translated.messages,
        body["messages"].as_array().unwrap().clone()
    );
}

#[test]
fn copilot_responses_bridge_maps_mcp_optional_tools_to_chat_functions() {
    let conversations = conversation_store();
    let request = serde_json::json!({
        "model": "codex",
        "stream": true,
        "input": "compress and inspect the workspace",
        "tools": [
            {
                "type": "mcp_tool",
                "name": "mcp__prodex_sqz__sqz_read_file",
                "description": "Read a file through SQZ.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }
            },
            {
                "type": "mcp_toolset",
                "mcp_server_name": "prodex-sqz",
                "default_config": {"enabled": false},
                "configs": {
                    "compress": {"enabled": true},
                    "sqz_read_file": {"enabled": false}
                }
            }
        ],
        "tool_choice": {
            "type": "mcp_tool",
            "name": "mcp__prodex_sqz__sqz_read_file"
        }
    });

    let translated = runtime_copilot_responses_chat_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversations,
    )
    .expect("Copilot Responses request should translate to chat");
    let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let tools = body["tools"].as_array().unwrap();

    assert_eq!(body["model"], prodex_cli::SUPER_COPILOT_DEFAULT_MODEL);
    assert_eq!(tools.len(), 2);
    assert_eq!(
        tools[0]["function"]["name"],
        "mcp__prodex_sqz__sqz_read_file"
    );
    assert_eq!(tools[0]["function"]["parameters"]["required"][0], "path");
    assert_eq!(tools[1]["function"]["name"], "mcp__prodex_sqz__compress");
    assert_eq!(
        body["tool_choice"]["function"]["name"],
        "mcp__prodex_sqz__sqz_read_file"
    );
}

#[test]
fn copilot_bridge_stream_records_response_id_and_restores_mcp_namespace() {
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured_for_recorder = Arc::clone(&captured);
    let recorder: RuntimeCopilotBindingRecorder = Arc::new(move |response_id| {
        captured_for_recorder.lock().unwrap().push(response_id);
    });
    let chat_stream = concat!(
        "data: {\"id\":\"chatcmpl_copilot_1\",\"model\":\"gpt-5.1-codex\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_sqz_1\",\"type\":\"function\",\"function\":{\"name\":\"mcp__prodex_sqz__compress\",\"arguments\":\"{}\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_copilot_1\",\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let chat_reader = super::super::deepseek_rewrite::RuntimeDeepSeekChatSseReader::new(
        Cursor::new(chat_stream),
        11,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut reader = RuntimeCopilotResponsesSseBindingReader::new(chat_reader, Some(recorder));
    let mut output = String::new();

    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"namespace\":\"mcp__prodex_sqz\""));
    assert!(output.contains("\"name\":\"compress\""));
    assert_eq!(captured.lock().unwrap().as_slice(), ["chatcmpl_copilot_1"]);
}

#[test]
fn copilot_oauth_pool_rotates_fresh_requests() {
    let pool = copilot_pool(&["alpha", "beta"]);
    let body = serde_json::to_vec(&serde_json::json!({"input": "hi"})).unwrap();

    let first = pool.select_attempts(&body, &[]).unwrap();
    let second = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(first[0].profile_name, "alpha");
    assert_eq!(
        first[0].api_url.as_deref(),
        Some("https://api.alpha.githubcopilot.test")
    );
    assert_eq!(first[1].profile_name, "beta");
    assert!(!first[0].hard_affinity);
    assert_eq!(second[0].profile_name, "beta");
    assert_eq!(second[1].profile_name, "alpha");
}

#[test]
fn copilot_oauth_pool_preserves_previous_response_affinity() {
    let pool = copilot_pool(&["alpha", "beta"]);
    pool.state
        .lock()
        .unwrap()
        .remember_response_binding("beta", "resp_1");
    let body = serde_json::to_vec(&serde_json::json!({"previous_response_id": "resp_1"})).unwrap();

    let attempts = pool.select_attempts(&body, &[]).unwrap();

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].profile_name, "beta");
    assert_eq!(
        attempts[0].api_url.as_deref(),
        Some("https://api.beta.githubcopilot.test")
    );
    assert!(attempts[0].hard_affinity);
}

#[test]
fn copilot_sse_binding_reader_preserves_bytes_and_records_response_id() {
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured_for_recorder = Arc::clone(&captured);
    let recorder: RuntimeCopilotBindingRecorder = Arc::new(move |response_id| {
        captured_for_recorder.lock().unwrap().push(response_id);
    });
    let stream = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"response_id\":\"resp_1\",\"delta\":\"hi\"}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader =
        RuntimeCopilotResponsesSseBindingReader::new(Cursor::new(stream), Some(recorder));
    let mut output = String::new();

    reader.read_to_string(&mut output).unwrap();

    assert_eq!(output, stream);
    assert_eq!(captured.lock().unwrap().as_slice(), ["resp_1"]);
}

#[test]
fn copilot_binding_recorder_reads_buffered_responses_body() {
    let captured = Arc::new(Mutex::new(None::<String>));
    let captured_for_recorder = Arc::clone(&captured);
    let recorder: RuntimeCopilotBindingRecorder = Arc::new(move |response_id| {
        *captured_for_recorder.lock().unwrap() = Some(response_id);
    });
    let body = serde_json::to_vec(&serde_json::json!({"id": "resp_1"})).unwrap();

    runtime_copilot_remember_bindings_from_responses_body(Some(&recorder), &body);

    assert_eq!(captured.lock().unwrap().as_deref(), Some("resp_1"));
}
