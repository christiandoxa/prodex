use super::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_store_conversation,
    runtime_gemini_chat_assistant_messages_from_generate_value,
    runtime_gemini_generate_request_body, runtime_gemini_harden_tool_call_thought_signatures,
    runtime_gemini_request_body_without_google_search,
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
    let system_text = value["systemInstruction"]["parts"][0]["text"]
        .as_str()
        .unwrap();
    assert!(system_text.contains("wait/read follow-up tool"));
    assert!(system_text.contains("gh run view"));
    assert!(system_text.contains("CI success"));
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
fn gemini_request_translation_maps_codex_web_search_to_google_search() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Search the web",
        "tools": [
            {"type": "web_search_preview"},
            {
                "type": "function",
                "name": "shell",
                "description": "Run shell",
                "parameters": {"type": "object"}
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

    assert_eq!(value["tools"][0]["googleSearch"], serde_json::json!({}));
    assert_eq!(
        value["tools"][1]["functionDeclarations"][0]["name"],
        "shell"
    );
}

#[test]
fn gemini_request_translation_strips_google_search_for_fallback() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Search the web and use tools",
        "tools": [
            {"type": "web_search_preview"},
            {
                "type": "function",
                "name": "shell",
                "description": "Run shell",
                "parameters": {"type": "object"}
            }
        ]
    });

    for code_assist in [false, true] {
        let translated = runtime_gemini_generate_request_body(
            &serde_json::to_vec(&body).unwrap(),
            &conversation_store(),
            code_assist,
            Some("project-id"),
        )
        .expect("request should translate");
        let stripped = runtime_gemini_request_body_without_google_search(&translated.body)
            .expect("googleSearch should be stripped");
        let value: serde_json::Value = serde_json::from_slice(&stripped).unwrap();
        let request = value.get("request").unwrap_or(&value);

        assert_eq!(request["tools"].as_array().unwrap().len(), 1);
        assert_eq!(
            request["tools"][0]["functionDeclarations"][0]["name"],
            "shell"
        );
    }
}

#[test]
fn gemini_request_translation_maps_namespace_tools_to_function_declarations() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Read README through SQZ",
        "tools": [{
            "type": "namespace",
            "name": "mcp__prodex_sqz",
            "description": "SQZ tools",
            "tools": [{
                "type": "function",
                "name": "sqz_read_file",
                "description": "Read a file through SQZ.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }
            }]
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(
        value["tools"][0]["functionDeclarations"][0]["name"],
        "mcp__prodex_sqz__sqz_read_file"
    );
    assert_eq!(
        value["tools"][0]["functionDeclarations"][0]["parameters"]["required"][0],
        "path"
    );
}

#[test]
fn gemini_request_translation_maps_tool_search_to_function_declaration() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Find the SQZ read tool",
        "tools": [{
            "type": "tool_search",
            "execution": "client",
            "description": "Search deferred tools.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"]
            }
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(
        value["tools"][0]["functionDeclarations"][0]["name"],
        "tool_search"
    );
    assert_eq!(
        value["tools"][0]["functionDeclarations"][0]["parameters"]["required"][0],
        "query"
    );
}

#[test]
fn gemini_response_translation_restores_namespace_tool_calls() {
    let response = serde_json::json!({
        "responseId": "resp_ns_1",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "id": "call_sqz_1",
                        "name": "mcp__prodex_sqz__sqz_read_file",
                        "args": {"path": "README.md"}
                    }
                }]
            }
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);

    assert_eq!(translated["output"][0]["type"], "function_call");
    assert_eq!(translated["output"][0]["namespace"], "mcp__prodex_sqz");
    assert_eq!(translated["output"][0]["name"], "sqz_read_file");
    assert_eq!(translated["output"][0]["call_id"], "call_sqz_1");
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
fn gemini_request_translation_groups_multiple_mcp_tool_results_from_history() {
    let conversations = conversation_store();
    let calls = [
        (
            "call_sqz_1",
            "mcp__prodex_sqz__compress",
            "{\"text\":\"large content\"}",
            "sqz result",
        ),
        (
            "call_ts_1",
            "mcp__prodex_token_savior__ts_search",
            "{\"query\":\"MCP tools\"}",
            "token-savior result",
        ),
    ];
    runtime_deepseek_store_conversation(
        &conversations,
        "resp_multi_mcp",
        vec![serde_json::json!({"role": "user", "content": "use sqz and token-savior"})],
        vec![serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": calls.iter().map(|(id, name, arguments, _)| serde_json::json!({
                "id": id,
                "type": "function",
                "function": {"name": name, "arguments": arguments}
            })).collect::<Vec<_>>()
        })],
    );
    let followup = serde_json::json!({
        "model": "gemini-2.5-flash",
        "previous_response_id": "resp_multi_mcp",
        "input": calls.iter().map(|(id, _, _, output)| serde_json::json!({
            "type": "mcp_tool_result",
            "call_id": id,
            "content": [{"type": "output_text", "text": output}]
        })).collect::<Vec<_>>()
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&followup).unwrap(),
        &conversations,
        false,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(value["contents"][0]["role"], "user");
    assert_eq!(value["contents"][1]["role"], "model");
    assert_eq!(value["contents"][1]["parts"].as_array().unwrap().len(), 2);
    assert_eq!(value["contents"][2]["role"], "user");
    assert_eq!(value["contents"][2]["parts"].as_array().unwrap().len(), 2);
    for (index, (_, name, _, _)) in calls.iter().enumerate() {
        assert_eq!(
            value["contents"][2]["parts"][index]["functionResponse"]["name"],
            *name
        );
    }
    assert!(
        value["contents"].as_array().unwrap().get(3).is_none(),
        "MCP results for one Gemini function-call turn must stay grouped"
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
    assert!(
        declarations[2]["parameters"]
            .get("additionalProperties")
            .is_none()
    );
    assert_eq!(
        value["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"][0],
        "mcp__prodex_sqz__sqz_read_file"
    );
}

#[test]
fn gemini_sanitizes_optional_tool_schemas_for_function_declarations() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "read and compress",
        "tools": [{
            "type": "mcp_tool",
            "name": "mcp__prodex_sqz__sqz_read_file",
            "description": "Read a file through SQZ.",
            "parametersJsonSchema": {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {
                        "anyOf": [
                            {"type": "string", "description": "File path"},
                            {"type": "null"}
                        ]
                    },
                    "detail": {
                        "type": ["string", "null"],
                        "enum": ["high", "original", 1],
                        "default": "high"
                    }
                },
                "required": ["path"]
            }
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let parameters = &value["tools"][0]["functionDeclarations"][0]["parameters"];

    assert_eq!(parameters["type"], "object");
    assert!(parameters.get("$schema").is_none());
    assert!(parameters.get("additionalProperties").is_none());
    assert!(parameters["properties"]["path"].get("anyOf").is_none());
    assert_eq!(parameters["properties"]["path"]["type"], "string");
    assert_eq!(parameters["properties"]["path"]["nullable"], true);
    assert_eq!(parameters["properties"]["detail"]["type"], "string");
    assert_eq!(parameters["properties"]["detail"]["nullable"], true);
    assert_eq!(
        parameters["properties"]["detail"]["enum"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn gemini_maps_required_and_none_tool_choice_modes() {
    for (choice, mode) in [("required", "ANY"), ("none", "NONE")] {
        let body = serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": "maybe use a tool",
            "tools": [{
                "type": "function",
                "name": "shell",
                "parameters": {"type": "object"}
            }],
            "tool_choice": choice
        });

        let translated = runtime_gemini_generate_request_body(
            &serde_json::to_vec(&body).unwrap(),
            &conversation_store(),
            false,
            None,
        )
        .expect("request should translate");
        let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

        assert_eq!(value["toolConfig"]["functionCallingConfig"]["mode"], mode);
        assert!(
            value["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"].is_null(),
            "{choice} should not restrict the allowed function names"
        );
    }
}

#[test]
fn gemini_preserves_thought_signature_for_tool_followup_history() {
    let conversations = conversation_store();
    let response = serde_json::json!({
        "responseId": "resp_sig_1",
        "modelVersion": "gemini-3.1-pro-preview-customtools",
        "candidates": [{
            "content": {
                "parts": [{
                    "thoughtSignature": "sig-fn-1",
                    "functionCall": {
                        "id": "call_google_1",
                        "name": "mcp__prodex_sqz__sqz_read_file",
                        "args": {"path": "README.md"}
                    }
                }]
            }
        }]
    });
    runtime_deepseek_store_conversation(
        &conversations,
        "resp_sig_1",
        vec![serde_json::json!({"role": "user", "content": "read README"})],
        runtime_gemini_chat_assistant_messages_from_generate_value(&response, 12),
    );
    let followup = serde_json::json!({
        "model": "gemini-3.1-pro-preview-customtools",
        "previous_response_id": "resp_sig_1",
        "input": [{
            "type": "function_call_output",
            "call_id": "call_google_1",
            "output": "Prodex README"
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&followup).unwrap(),
        &conversations,
        false,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(
        value["contents"][1]["parts"][0]["functionCall"]["id"],
        "call_google_1"
    );
    assert_eq!(
        value["contents"][1]["parts"][0]["thoughtSignature"],
        "sig-fn-1"
    );
    assert_eq!(
        value["contents"][2]["parts"][0]["functionResponse"]["id"],
        "call_google_1"
    );
}

#[test]
fn gemini_3_hardens_unsigned_first_tool_call_with_synthetic_signature() {
    let mut body = serde_json::to_vec(&serde_json::json!({
        "request": {
            "contents": [{
                "role": "model",
                "parts": [
                    {
                        "functionCall": {
                            "id": "call_1",
                            "name": "shell",
                            "args": {"cmd": "ls"}
                        }
                    },
                    {
                        "functionCall": {
                            "id": "call_2",
                            "name": "shell",
                            "args": {"cmd": "pwd"}
                        }
                    }
                ]
            }]
        }
    }))
    .unwrap();

    let injected =
        runtime_gemini_harden_tool_call_thought_signatures(&mut body, "gemini-3.1-pro-preview")
            .expect("request should harden");
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(injected, 1);
    assert_eq!(
        value["request"]["contents"][0]["parts"][0]["thoughtSignature"],
        "skip_thought_signature_validator"
    );
    assert!(
        value["request"]["contents"][0]["parts"][1]
            .get("thoughtSignature")
            .is_none()
    );
}

#[test]
fn gemini_25_does_not_harden_unsigned_tool_calls() {
    let mut body = serde_json::to_vec(&serde_json::json!({
        "contents": [{
            "role": "model",
            "parts": [{
                "functionCall": {
                    "id": "call_1",
                    "name": "shell",
                    "args": {"cmd": "ls"}
                }
            }]
        }]
    }))
    .unwrap();

    let injected = runtime_gemini_harden_tool_call_thought_signatures(&mut body, "gemini-2.5-pro")
        .expect("request should harden");
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(injected, 0);
    assert!(
        value["contents"][0]["parts"][0]
            .get("thoughtSignature")
            .is_none()
    );
}

#[test]
fn gemini_response_restores_optional_mcp_function_call_names_for_codex() {
    let response = serde_json::json!({
        "responseId": "resp_sqz_1",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "thoughtSignature": "sig-response-1",
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
    assert_eq!(responses["output"][0]["call_id"], "call_sqz_1");
    assert_eq!(responses["output"][0]["namespace"], "mcp__prodex_sqz");
    assert_eq!(responses["output"][0]["name"], "compress");
    assert_eq!(
        responses["output"][0]["arguments"],
        "{\"text\":\"large content\"}"
    );
    assert_eq!(
        responses["output"][0]["gemini_thought_signature"],
        "sig-response-1"
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
