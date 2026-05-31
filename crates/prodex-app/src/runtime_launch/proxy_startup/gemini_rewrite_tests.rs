use super::{
    RuntimeDeepSeekConversationStore, runtime_gemini_generate_request_body,
    runtime_gemini_responses_value_from_generate_value,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
}

#[test]
fn gemini_request_translation_maps_tools_and_thinking() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "stream": true,
        "instructions": "Be concise.",
        "input": "List files",
        "tools": [{
            "type": "function",
            "name": "shell",
            "description": "Run shell",
            "parameters": {"type": "object"}
        }],
        "reasoning": {"effort": "high"},
        "max_output_tokens": 123
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated.model, "gemini-2.5-pro");
    assert!(translated.stream);
    assert_eq!(value["contents"][0]["parts"][0]["text"], "List files");
    assert_eq!(
        value["tools"][0]["functionDeclarations"][0]["name"],
        "shell"
    );
    assert_eq!(value["generationConfig"]["maxOutputTokens"], 123);
    assert_eq!(
        value["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        8192
    );
}

#[test]
fn gemini_request_translation_maps_gemma_4_thinking_level() {
    let body = serde_json::json!({
        "model": "gemma-4-31b-it",
        "input": "Check this patch",
        "reasoning": {"effort": "low"}
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated.model, "gemma-4-31b-it");
    assert_eq!(
        value["generationConfig"]["thinkingConfig"]["thinkingLevel"],
        "LOW"
    );
    assert!(
        value["generationConfig"]["thinkingConfig"]["thinkingBudget"].is_null(),
        "Gemma 4 should use Gemini CLI-style thinkingLevel, not thinkingBudget"
    );
}

#[test]
fn gemini_request_translation_maps_tool_outputs_as_user_function_responses() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [
            {
                "type": "function_call",
                "call_id": "call_shell_1",
                "name": "shell",
                "arguments": "{\"cmd\":\"git log -n 5\"}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_shell_1",
                "output": "commit abc123"
            }
        ]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(value["contents"][0]["role"], "model");
    assert_eq!(
        value["contents"][0]["parts"][0]["functionCall"]["id"],
        "call_shell_1"
    );
    assert_eq!(value["contents"][1]["role"], "user");
    assert_eq!(
        value["contents"][1]["parts"][0]["functionResponse"]["id"],
        "call_shell_1"
    );
    assert_eq!(
        value["contents"][1]["parts"][0]["functionResponse"]["response"]["output"],
        "commit abc123"
    );
}

#[test]
fn gemini_maps_mcp_optional_tools_to_function_declarations() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
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
                "type": "mcp_tool",
                "name": "mcp__prodex_token_savior__find_symbol",
                "description": "Find a symbol through token-savior.",
                "schema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"}
                    }
                }
            },
            {
                "type": "mcp_toolset",
                "mcp_server_name": "prodex-sqz",
                "description": "SQZ optimizer MCP server.",
                "default_config": {"enabled": false},
                "configs": {
                    "compress": {"enabled": true},
                    "sqz_read_file": {"enabled": true}
                }
            }
        ],
        "tool_choice": {
            "type": "mcp_tool",
            "name": "mcp__prodex_sqz__sqz_read_file"
        }
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let declarations = value["tools"][0]["functionDeclarations"]
        .as_array()
        .expect("function declarations should be present");
    let names = declarations
        .iter()
        .filter_map(|declaration| declaration["name"].as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "mcp__prodex_sqz__sqz_read_file",
            "mcp__prodex_token_savior__find_symbol",
            "mcp__prodex_sqz__compress",
        ]
    );
    assert_eq!(declarations[0]["parameters"]["required"][0], "path");
    assert_eq!(
        declarations[1]["parameters"]["properties"]["name"]["type"],
        "string"
    );
    assert_eq!(declarations[2]["parameters"]["additionalProperties"], true);
    assert_eq!(
        value["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"][0],
        "mcp__prodex_sqz__sqz_read_file"
    );
}

#[test]
fn gemini_response_preserves_optional_mcp_function_call_names_for_codex() {
    let response = serde_json::json!({
        "responseId": "resp_sqz_1",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "id": "call_sqz_1",
                        "name": "mcp__prodex_sqz__compress",
                        "args": {"text": "large content"}
                    }
                }]
            }
        }]
    });

    let responses = runtime_gemini_responses_value_from_generate_value(&response, 7);

    assert_eq!(responses["output"][0]["type"], "function_call");
    assert_eq!(responses["output"][0]["call_id"], "call_gemini_7_0");
    assert_eq!(responses["output"][0]["name"], "mcp__prodex_sqz__compress");
    assert_eq!(
        responses["output"][0]["arguments"],
        "{\"text\":\"large content\"}"
    );
}

#[test]
fn gemini_wraps_claw_compactor_shell_benchmarks_with_rtk() {
    let response = serde_json::json!({
        "responseId": "resp_claw_1",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "name": "shell",
                        "args": {"cmd": "claw-compactor benchmark /workspace --json"}
                    }
                }]
            }
        }]
    });

    let responses = runtime_gemini_responses_value_from_generate_value(&response, 7);
    let arguments: serde_json::Value =
        serde_json::from_str(responses["output"][0]["arguments"].as_str().unwrap()).unwrap();

    assert_eq!(
        arguments["cmd"],
        "rtk claw-compactor benchmark /workspace --json"
    );
}
