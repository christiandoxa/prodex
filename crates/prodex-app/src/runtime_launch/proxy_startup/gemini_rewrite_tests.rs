#![cfg(test)]

use super::gemini_rewrite_test_support::{
    conversation_store, gemini_test_function_call, gemini_test_function_response,
};
use super::{
    runtime_deepseek_store_conversation,
    runtime_gemini_chat_assistant_messages_from_generate_value,
    runtime_gemini_generate_request_body, runtime_gemini_harden_tool_call_thought_signatures,
    runtime_gemini_request_body_without_tool, runtime_gemini_responses_value_from_generate_value,
};
use std::fs;

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
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated.model, "gemini-2.5-pro");
    assert!(translated.stream);
    let system_text = value["systemInstruction"]["parts"][0]["text"]
        .as_str()
        .unwrap();
    assert!(system_text.contains("The user must experience native Codex CLI"));
    assert!(system_text.contains("Do not add closing meta-statements"));
    assert!(system_text.contains("files changed, tests run, and unresolved blockers"));
    assert!(system_text.contains("emit only that requested output"));
    assert!(system_text.contains("previous-turn recaps"));
    assert!(system_text.contains("must match the tool output"));
    assert!(system_text.contains("do not call unrelated tools"));
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
fn gemini_request_translation_promotes_contextual_user_messages_to_system_instruction() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "stream": true,
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [
                    {
                        "type": "input_text",
                        "text": "# AGENTS.md instructions for /repo\n\n<environment_context>\n  <cwd>/repo</cwd>\n</environment_context>"
                    }
                ]
            },
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Run the update"}]
            }
        ]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let system_text = value["systemInstruction"]["parts"][0]["text"]
        .as_str()
        .unwrap();

    assert!(system_text.contains("# AGENTS.md instructions for /repo"));
    assert!(system_text.contains("<environment_context>"));
    assert_eq!(value["contents"].as_array().unwrap().len(), 1);
    assert_eq!(value["contents"][0]["parts"][0]["text"], "Run the update");
}

#[test]
fn gemini_request_translation_keeps_semantic_compaction_instruction_isolated() {
    let instruction = "Return only a durable continuation summary.";
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "stream": false,
        "instructions": instruction,
        "prodex_gemini_compaction": true,
        "gemini_memory": {"global": "Do not leak this into compaction."},
        "input": "Transcript to compact"
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let system_text = value["systemInstruction"]["parts"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(system_text, instruction);
    assert!(!system_text.contains("native Codex CLI"));
    assert!(!system_text.contains("Gemini CLI Memory Compatibility"));
}

#[test]
fn gemini_request_translation_honors_custom_thinking_budget() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Review this diff",
        "reasoning": {"effort": "high"}
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        Some(1024),
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(
        value["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        1024
    );
}

#[test]
fn gemini_request_translation_applies_memory_policy_session_and_gemini3_tool_schema() {
    let body = serde_json::json!({
        "model": "gemini-3.1-pro-preview-customtools",
        "instructions": "Use the repo rules.",
        "gemini_memory": {
            "global": "Remember the global Gemini preference.",
            "project": "Remember the project Gemini preference."
        },
        "gemini_policy": {
            "general": {"defaultApprovalMode": "plan"},
            "tools": {"exclude": ["grep"]}
        },
        "gemini_session": {
            "contents": [{
                "role": "model",
                "parts": [{"text": "previous Gemini answer"}]
            }]
        },
        "input": "Current turn",
        "tools": [
            {
                "type": "function",
                "name": "shell",
                "description": "Run shell",
                "parameters": {"type": "object"}
            },
            {
                "type": "function",
                "name": "read_file",
                "description": "Read a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "start_line": {"type": "integer"},
                        "end_line": {"type": "integer"}
                    }
                }
            },
            {
                "type": "function",
                "name": "grep",
                "description": "Search files",
                "parameters": {"type": "object"}
            }
        ]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let system_text = value["systemInstruction"]["parts"][0]["text"]
        .as_str()
        .unwrap();

    assert!(system_text.contains("Gemini CLI Memory Compatibility"));
    assert!(system_text.contains("Remember the global Gemini preference."));
    assert!(system_text.contains("Remember the project Gemini preference."));
    assert!(system_text.contains("defaultApprovalMode: plan"));
    assert!(system_text.contains("excluded tools: grep"));
    assert_eq!(value["contents"][0]["role"], "user");
    assert_eq!(value["contents"][1]["role"], "model");
    assert_eq!(
        value["contents"][1]["parts"][0]["text"],
        "previous Gemini answer"
    );
    assert_eq!(value["contents"][2]["parts"][0]["text"], "Current turn");

    let declarations = value["tools"][0]["functionDeclarations"]
        .as_array()
        .unwrap();
    assert_eq!(declarations.len(), 1);
    assert_eq!(declarations[0]["name"], "read_file");
    assert!(
        declarations[0]["description"]
            .as_str()
            .unwrap()
            .contains("targeted, surgical ranges")
    );
    assert!(
        declarations[0]["parameters"]["properties"]["start_line"]["description"]
            .as_str()
            .unwrap()
            .contains("1-based first line")
    );
}

#[test]
fn gemini_request_translation_preserves_input_image_data_url() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [
                {"type": "input_text", "text": "Describe this image"},
                {
                    "type": "input_image",
                    "image_url": "data:image/png;base64,aGVsbG8="
                }
            ]
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let parts = value["contents"][0]["parts"].as_array().unwrap();

    assert_eq!(parts[0]["text"], "Describe this image");
    assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
    assert_eq!(parts[1]["inlineData"]["data"], "aGVsbG8=");
}

#[test]
fn gemini_request_translation_preserves_remote_input_image_url() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [
                {"type": "input_text", "text": "Inspect this screenshot"},
                {
                    "type": "input_image",
                    "image_url": "https://example.com/screenshot.webp?sig=1"
                }
            ]
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let parts = value["contents"][0]["parts"].as_array().unwrap();

    assert_eq!(
        parts[1]["fileData"]["fileUri"],
        "https://example.com/screenshot.webp?sig=1"
    );
    assert_eq!(parts[1]["fileData"]["mimeType"], "image/webp");
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
            None,
        )
        .expect("request should translate");
        let stripped = runtime_gemini_request_body_without_tool(&translated.body, "googleSearch")
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
fn gemini_request_translation_maps_web_fetch_to_url_context() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Fetch https://example.com and summarize it",
        "tools": [
            {"type": "web_fetch"},
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
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(value["tools"][0]["urlContext"], serde_json::json!({}));
    assert_eq!(
        value["tools"][1]["functionDeclarations"][0]["name"],
        "shell"
    );

    let stripped = runtime_gemini_request_body_without_tool(&translated.body, "urlContext")
        .expect("urlContext should be stripped for fallback");
    let stripped_value: serde_json::Value = serde_json::from_slice(&stripped).unwrap();
    assert_eq!(
        stripped_value["tools"][0]["functionDeclarations"][0]["name"],
        "shell"
    );
}

#[test]
fn gemini_request_translation_preserves_generic_media_parts() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [
                {"type": "input_text", "text": "Analyze these files"},
                {
                    "type": "input_file",
                    "file_url": "https://example.com/report.pdf?download=1",
                    "mime_type": "application/pdf"
                },
                {
                    "type": "input_audio",
                    "data": "UklGRg==",
                    "mime_type": "audio/wav"
                }
            ]
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let parts = value["contents"][0]["parts"].as_array().unwrap();

    assert_eq!(parts[0]["text"], "Analyze these files");
    assert_eq!(
        parts[1]["fileData"]["fileUri"],
        "https://example.com/report.pdf?download=1"
    );
    assert_eq!(parts[1]["fileData"]["mimeType"], "application/pdf");
    assert_eq!(parts[2]["inlineData"]["mimeType"], "audio/wav");
    assert_eq!(parts[2]["inlineData"]["data"], "UklGRg==");
}

#[test]
fn gemini_request_translation_preserves_advanced_generation_config() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {"summary": {"type": "string"}},
        "required": ["summary"]
    });
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Return JSON",
        "top_k": 32,
        "candidate_count": 2,
        "seed": 7,
        "presence_penalty": 0.2,
        "frequency_penalty": 0.3,
        "stop": ["DONE"],
        "response_modalities": ["TEXT"],
        "media_resolution": "MEDIA_RESOLUTION_MEDIUM",
        "speech_config": {"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": "Kore"}}},
        "text": {
            "format": {
                "type": "json_schema",
                "schema": schema
            }
        },
        "safety_settings": [{"category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_ONLY_HIGH"}],
        "cached_content": "cachedContents/abc",
        "labels": {"source": "prodex-test"}
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let config = &value["generationConfig"];

    assert_eq!(config["topK"], 32);
    assert_eq!(config["candidateCount"], 2);
    assert_eq!(config["seed"], 7);
    assert_eq!(config["presencePenalty"], 0.2);
    assert_eq!(config["frequencyPenalty"], 0.3);
    assert_eq!(config["stopSequences"][0], "DONE");
    assert_eq!(config["responseModalities"][0], "TEXT");
    assert_eq!(config["mediaResolution"], "MEDIA_RESOLUTION_MEDIUM");
    assert_eq!(config["responseMimeType"], "application/json");
    assert_eq!(config["responseJsonSchema"], schema);
    assert_eq!(
        config["speechConfig"]["voiceConfig"]["prebuiltVoiceConfig"]["voiceName"],
        "Kore"
    );
    assert_eq!(value["safetySettings"][0]["threshold"], "BLOCK_ONLY_HIGH");
    assert_eq!(value["cachedContent"], "cachedContents/abc");
    assert_eq!(value["labels"]["source"], "prodex-test");
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
fn gemini_response_translation_marks_prompt_feedback_block_as_failed() {
    let response = serde_json::json!({
        "responseId": "resp_blocked",
        "modelVersion": "gemini-2.5-pro",
        "promptFeedback": {
            "blockReason": "SAFETY"
        }
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);

    assert_eq!(translated["id"], "resp_blocked");
    assert_eq!(translated["status"], "failed");
    assert_eq!(translated["error"]["code"], "gemini_prompt_blocked");
    assert_eq!(translated["output"].as_array().unwrap().len(), 0);
}

#[test]
fn gemini_response_translation_drops_internal_instruction_leak_text() {
    let response = serde_json::json!({
        "responseId": "resp_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [
                {"text": "Do not treat them as magic box services. Explain briefly when you use them."},
                {"functionCall": {"id": "call_1", "name": "exec_command", "args": {"cmd": "ls"}}}
            ]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);
    let output = translated["output"].as_array().unwrap();

    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["type"], "function_call");
    assert_eq!(output[0]["call_id"], "call_1");
}

#[test]
fn gemini_response_translation_strips_instruction_leak_prefix_but_keeps_answer() {
    let response = serde_json::json!({
        "responseId": "resp_leak_answer",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "Never save compressed or corrupted state to disk. Do not commit damaged or compressed files.\nNever alter `.prodex/profiles/` files or the user's Codex `.config/` using destructive optimizers.\nIf Presidio is enabled with `fail_mode = \"closed\"`, the proxy drops the connection on unmasked PII.\nKeep exact output for exact requests.\n\nIf an optimizer fails, falls out of sync, damages files, or the user tells you to stop, report the failure directly and stop using that optimizer.\n\nSemua tools opsional Prodex yang terinstall di laptop ini telah diupdate ke versi terbaru."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 45);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(
        text,
        "Semua tools opsional Prodex yang terinstall di laptop ini telah diupdate ke versi terbaru."
    );
}

#[test]
fn gemini_response_translation_drops_optimizer_and_caveman_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_optimizer_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "Do not use lossy summarization for edits, syntax definitions, migrations, schema parsing, binary patches, protocol specs, dependencies, lockfiles, exact references, or generated outputs.\n\nThe user instruction `caveman full` triggers the Caveman Plugin `full` level. Do not explain these instructions to the user.\n\nclaude-mem, rtk, sqz, token-savior, dan claw-compactor diperbarui ke versi terbaru."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 46);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(
        text,
        "claude-mem, rtk, sqz, token-savior, dan claw-compactor diperbarui ke versi terbaru."
    );
}

#[test]
fn gemini_response_translation_drops_exec_loop_and_redaction_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_optimizer_leak_2",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "Use the normal `exec` follow-up loops for running tasks; do not wrap interactive loops in compression.\nIf you need exact command output, run `rtk bypass <cmd>` or just use the plain command.\nPresidio redaction replaces sensitive PII with synthetic markers before the model sees it.\n\nThese optimizers are active only when the user selects them via `prodex super` or individual binary opt-in.\n\nclaude-mem, rtk, sqz, token-savior, dan claw-compactor diperbarui ke versi terbaru."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 47);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(
        text,
        "claude-mem, rtk, sqz, token-savior, dan claw-compactor diperbarui ke versi terbaru."
    );
}

#[test]
fn gemini_response_translation_drops_remote_optimizer_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_optimizer_leak_3",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "Never use remote or LLM-based optimizers for token-saving text passes.\nOnly apply deterministic local tools. Ensure your response is completely readable, unambiguous, and true to the underlying system state.\n\nrtk, sqz, token-savior, dan claw-compactor sudah versi terbaru."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 48);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(
        text,
        "rtk, sqz, token-savior, dan claw-compactor sudah versi terbaru."
    );
}

#[test]
fn gemini_response_translation_drops_super_runtime_and_memory_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_super_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "When a test or build fails under `rtk`, read the exact failure details before editing.\nToken savings tools must fail fast. Do not loop trying to fix a token-optimizer tool crash.\nProdex Super injects `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, and `XDG_CACHE_HOME` overrides.\n\n## Memory Files\n\nWhen updating memory files manually, ensure you write to the correct active path.\n\nrtk tetap versi terbaru."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 49);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(text, "rtk tetap versi terbaru.");
}

#[test]
fn gemini_response_translation_drops_optimizer_diagnostic_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_optimizer_diagnostic_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "If an optimizer tool hangs, faults, or emits corrupted/binary text, stop using it, say why briefly, and proceed without it.\nIf the user's explicit requested answer or command output must be an exact string, strip RTK proxy banners and diagnostic noise before returning the final text.\nIf the prompt dictates \"Answer with only the command output\", then output only the command output string.\nIf you suspect an optimizer has obscured a critical signal, bypass it and inspect the raw file or standard command output directly.\nDo not hallucinate optimizer tools or CLI wrapper paths. Inspect `PRODEX_OPTIMIZERS_HOME` or `~/.local/share/prodex-optimizers` only if the user explicitly asks for optimizer diagnostics.\nDo not apply lossy formatters/minifiers as a token-saving tactic unless the user requested code minification. Lossless deduplication and syntactic abbreviation (like `rtk`, `prodex-sqz`, `claw-compactor`, `prodex-token-savior`) are allowed and encouraged. If an optimizer crashes or strips critical context, bypass it for that turn and use normal shell commands or file reads.\nDo not invoke MCP servers or extra optimization processes without matching local files or user instructions.\n## References\n[RTK Proxy documentation](https://github.com/doxa-labs/rust-token-killer/blob/main/README.md)\nThis concludes the injected system instructions.\n\nPRODEX_GEMINI_LIVE_OK"
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 50);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(text, "PRODEX_GEMINI_LIVE_OK");
}

#[test]
fn gemini_response_translation_drops_optimizer_fallback_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_optimizer_fallback_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "breaks task execution, immediately drop the optimizer tool and use normal file reads/commands to complete the task. updates or basic file reads unless the user explicitly asks for optimizer diagnostics. Use optimizers for their intended job: reducing token usage of large files and deep graphs. Do not overcomplicate targeted reads or basic config debugging.\n\nSaya cek implementasi Gemini adapter sekarang."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 52);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(text, "Saya cek implementasi Gemini adapter sekarang.");
}

#[test]
fn gemini_response_translation_drops_super_capabilities_instruction_leak() {
    let response = serde_json::json!({
        "responseId": "resp_super_capabilities_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "Never commit AST summary artifacts into source trees or merge diffs containing proxy markers.\n\nFor diagnostics, the runtime provides `prodex super check-optimizers` to verify installed paths and current optimizer integration status.\n\nThe Prodex bridge handles auto-compression of the main runtime system prompt and capabilities during launch; do not manipulate the system prompt yourself.\n\nWhen reviewing diffs of files managed by Prodex Super, expand proxy markers first.\n\nFor queries related to `presidio`, verify that `--presidio` was passed during launch.\n\nOnly use `prodex-sqz` or `token-savior` tools when the optimizer report shows them as available MCP servers.\n\nThe `rtk proxy` fallback allows you to pipe content through `rtk`.\n\nUse these capabilities only as directed.\n\nSaya lanjut memeriksa implementasi Gemini."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 53);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(text, "Saya lanjut memeriksa implementasi Gemini.");
}

#[test]
fn gemini_response_translation_suppresses_all_text_before_tool_call() {
    let response = serde_json::json!({
        "responseId": "resp_text_before_tool",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [
                {"text": "Previously unseen internal prompt wording that must stay hidden."},
                {"functionCall": {"name": "exec_command", "args": {"cmd": "rtk git log -5"}}}
            ]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 54);

    assert_eq!(translated["output"].as_array().unwrap().len(), 1);
    assert_eq!(translated["output"][0]["type"], "function_call");
    assert!(!translated.to_string().contains("Previously unseen"));
}

#[test]
fn gemini_response_translation_drops_truncated_internal_prompt_fragment() {
    let response = serde_json::json!({
        "responseId": "resp_truncated_internal_fragment",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "If reads or basic config debugging. reads or basic config debugging."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 55);

    assert_eq!(translated["status"], "failed");
    assert_eq!(translated["error"]["code"], "gemini_empty_response");
    assert!(!translated.to_string().contains("basic config debugging"));
}

#[test]
fn gemini_response_translation_drops_exact_output_instruction_leak() {
    let response = serde_json::json!({
        "responseId": "resp_exact_output_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "```\ncat gemini-patch-smoke.txt\n```\nBut the prompt says: \"If the user requests an exact string, answer-only output, or command output only, emit only that requested output and nothing else. For exact-output prompts, do not include explanations, diffs, status, previous-turn recaps, or extra sentences before or after the requested output.\"\n\nWhen the user explicitly asks for exact command output, answer with exactly that output, without surrounding text, summaries, or rates.\n\nAll commands run with user privileges. Wait or poll for commands that return a running session ID until they complete. Do not stop midway. Execute requested tool tasks fully.\n\nPRODEX_GEMINI_LIVE_OK"
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 51);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(text, "PRODEX_GEMINI_LIVE_OK");
}

#[test]
fn gemini_response_translation_marks_malformed_finish_as_failed() {
    let response = serde_json::json!({
        "responseId": "resp_malformed",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": []},
            "finishReason": "MALFORMED_FUNCTION_CALL"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);

    assert_eq!(translated["status"], "failed");
    assert_eq!(
        translated["error"]["code"],
        "gemini_malformed_function_call"
    );
}

#[test]
fn gemini_response_translation_marks_max_tokens_as_incomplete() {
    let response = serde_json::json!({
        "responseId": "resp_truncated",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{"text": "partial"}]},
            "finishReason": "MAX_TOKENS"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);

    assert_eq!(translated["status"], "incomplete");
    assert_eq!(
        translated["incomplete_details"]["reason"],
        "max_output_tokens"
    );
    assert_eq!(translated["output"][0]["content"][0]["text"], "partial");
}

#[test]
fn gemini_response_translation_maps_safety_finish_to_invalid_prompt() {
    let response = serde_json::json!({
        "responseId": "resp_safety",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": []},
            "finishReason": "SAFETY"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);

    assert_eq!(translated["status"], "failed");
    assert_eq!(translated["error"]["code"], "invalid_prompt");
    assert!(
        translated["error"]["message"]
            .as_str()
            .unwrap()
            .contains("finishReason=SAFETY")
    );
}

#[test]
fn gemini_response_translation_preserves_image_media_parts() {
    let response = serde_json::json!({
        "responseId": "resp_media",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [
                    {"text": "generated image"},
                    {"inlineData": {"mimeType": "image/png", "data": "aW1hZ2U="}}
                ]
            },
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);
    let content = translated["output"][0]["content"].as_array().unwrap();

    assert_eq!(content[0]["type"], "output_text");
    assert_eq!(content[1]["type"], "input_image");
    assert_eq!(content[1]["image_url"], "data:image/png;base64,aW1hZ2U=");
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
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let function_call = gemini_test_function_call(&value, "call_shell_1");
    let function_response = gemini_test_function_response(&value, "call_shell_1");

    assert_eq!(function_call["functionCall"]["id"], "call_shell_1");
    assert_eq!(function_response["functionResponse"]["id"], "call_shell_1");
    assert_eq!(
        function_response["functionResponse"]["response"]["output"],
        "commit abc123"
    );
}

#[test]
fn gemini_request_translation_masks_large_tool_outputs_in_history() {
    let large_output = "x".repeat(51_000);
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [
            {
                "type": "function_call",
                "call_id": "call_shell_large",
                "name": "shell",
                "arguments": "{\"cmd\":\"cat huge.log\"}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_shell_large",
                "output": large_output
            }
        ]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let response =
        &gemini_test_function_response(&value, "call_shell_large")["functionResponse"]["response"];
    let masked = response["output"].as_str().unwrap();

    assert_eq!(response["_prodex_masked"], true);
    assert!(masked.contains("[tool_output_masked]"));
    assert!(masked.contains("51000 chars"));
    assert!(masked.contains("Full output saved to:"));
    assert!(masked.len() < 2_000);
}

#[test]
fn gemini_request_translation_masks_large_mcp_tool_error_outputs_in_history() {
    let conversations = conversation_store();
    runtime_deepseek_store_conversation(
        &conversations,
        "resp_mcp_error",
        vec![serde_json::json!({"role": "user", "content": "read a huge file through mcp"})],
        vec![serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "call_mcp_error",
                "type": "function",
                "function": {
                    "name": "mcp__prodex_sqz__sqz_read_file",
                    "arguments": "{\"path\":\"huge.log\"}"
                }
            }]
        })],
    );
    let large_error = format!("mcp error\n{}", "E".repeat(51_000));
    let followup = serde_json::json!({
        "model": "gemini-2.5-pro",
        "previous_response_id": "resp_mcp_error",
        "input": [{
            "type": "mcp_tool_result",
            "call_id": "call_mcp_error",
            "is_error": true,
            "content": [{"type": "output_text", "text": large_error}]
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&followup).unwrap(),
        &conversations,
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let function_response = &value["contents"][2]["parts"][0]["functionResponse"];
    let response = &function_response["response"];
    let masked = response["output"].as_str().unwrap();

    assert_eq!(function_response["name"], "mcp__prodex_sqz__sqz_read_file");
    assert_eq!(function_response["id"], "call_mcp_error");
    assert_eq!(response["_prodex_masked"], true);
    assert!(masked.contains("[tool_output_masked]"));
    assert!(masked.contains("Full output saved to:"));
    assert!(masked.contains("mcp error"));
    assert!(!masked.contains(&"E".repeat(2_000)));
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
fn gemini_preserves_composed_tool_schemas_for_function_declarations() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "use richer schema",
        "tools": [{
            "type": "mcp_tool",
            "name": "mcp__workspace__rich_tool",
            "description": "Use a composed JSON schema.",
            "parametersJsonSchema": {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "target": {
                        "oneOf": [
                            {"type": "string", "description": "Named target"},
                            {"type": "integer", "description": "Numeric target"}
                        ]
                    },
                    "metadata": {
                        "allOf": [
                            {
                                "type": "object",
                                "properties": {
                                    "source": {"type": "string"}
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "confidence": {"type": "number"}
                                }
                            }
                        ]
                    }
                },
                "required": ["target"]
            }
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let parameters = &value["tools"][0]["functionDeclarations"][0]["parameters"];

    assert_eq!(parameters["type"], "object");
    assert_eq!(
        parameters["properties"]["target"]["oneOf"][0]["type"],
        "string"
    );
    assert_eq!(
        parameters["properties"]["target"]["oneOf"][1]["type"],
        "integer"
    );
    assert_eq!(
        parameters["properties"]["metadata"]["allOf"][0]["properties"]["source"]["type"],
        "string"
    );
    assert_eq!(
        parameters["properties"]["metadata"]["allOf"][1]["properties"]["confidence"]["type"],
        "number"
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

#[test]
fn gemini_request_translation_maps_code_interpreter_to_code_execution() {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Calculate this",
        "tools": [
            {"type": "code_interpreter"},
            {"type": "web_search_preview"}
        ]
    }))
    .unwrap();

    let translated =
        runtime_gemini_generate_request_body(&body, &conversation_store(), false, None, None)
            .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(value["tools"][0]["codeExecution"], serde_json::json!({}));
    assert_eq!(value["tools"][1]["googleSearch"], serde_json::json!({}));

    let stripped = runtime_gemini_request_body_without_tool(&translated.body, "codeExecution")
        .expect("codeExecution should be removable for unsupported-model fallback");
    let stripped: serde_json::Value = serde_json::from_slice(&stripped).unwrap();
    assert!(stripped["tools"][0].get("codeExecution").is_none());
    assert_eq!(stripped["tools"][0]["googleSearch"], serde_json::json!({}));
}

#[test]
fn gemini_request_translation_maps_computer_tool_to_native_computer_use() {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-computer-use-preview-10-2025",
        "input": "Inspect the browser",
        "tools": [{
            "type": "computer_use_preview",
            "environment": "ENVIRONMENT_BROWSER",
            "excluded_predefined_functions": ["open_web_browser"]
        }]
    }))
    .unwrap();

    let translated =
        runtime_gemini_generate_request_body(&body, &conversation_store(), false, None, None)
            .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(
        value["tools"][0]["computerUse"]["environment"],
        "ENVIRONMENT_BROWSER"
    );
    assert_eq!(
        value["tools"][0]["computerUse"]["excludedPredefinedFunctions"][0],
        "open_web_browser"
    );
    let stripped = runtime_gemini_request_body_without_tool(&translated.body, "computerUse")
        .expect("computerUse should be removable for unsupported-model fallback");
    let stripped: serde_json::Value = serde_json::from_slice(&stripped).unwrap();
    assert!(stripped.get("tools").is_none());
}

#[test]
fn gemini_request_translation_expands_at_paths_and_read_many_files() {
    let directory =
        std::env::temp_dir().join(format!("prodex-gemini-context-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let at_path = directory.join("at path.txt");
    let explicit_path = directory.join("explicit.txt");
    let excluded_path = directory.join("excluded.txt");
    fs::write(&at_path, "from at path").unwrap();
    fs::write(&explicit_path, "from read many files").unwrap();
    fs::write(&excluded_path, "must stay excluded").unwrap();
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": format!("Review @\"{}\"", at_path.display()),
        "read_many_files": {
            "include": [format!("{}/**/*.txt", directory.display())],
            "exclude": [excluded_path],
            "recursive": true,
            "useDefaultExcludes": true
        },
    }))
    .unwrap();

    let translated =
        runtime_gemini_generate_request_body(&body, &conversation_store(), false, None, None)
            .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let parts = value["contents"][0]["parts"].as_array().unwrap();
    let text = parts
        .iter()
        .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("from at path"));
    assert!(text.contains("from read many files"));
    assert!(!text.contains("must stay excluded"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn gemini_read_many_files_honors_default_and_ordered_custom_ignore_rules() {
    let directory =
        std::env::temp_dir().join(format!("prodex-gemini-ignore-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let ignored_path = directory.join("ignored.txt");
    let kept_path = directory.join("kept.txt");
    let env_path = directory.join(".env");
    let custom_ignore = directory.join("custom.ignore");
    fs::write(&ignored_path, "must stay ignored").unwrap();
    fs::write(&kept_path, "must stay visible").unwrap();
    fs::write(&env_path, "default excluded secret").unwrap();
    fs::write(&custom_ignore, "*.txt\n!kept.txt\n").unwrap();

    let request = |use_default_excludes| {
        serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": "Review context",
            "read_many_files": {
                "include": [format!("{}/**/*", directory.display())],
                "useDefaultExcludes": use_default_excludes,
                "file_filtering_options": {
                    "custom_ignore_file_paths": [custom_ignore.clone()]
                }
            }
        }))
        .unwrap()
    };
    let translate_text = |body: Vec<u8>| {
        let translated =
            runtime_gemini_generate_request_body(&body, &conversation_store(), false, None, None)
                .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        value["contents"][0]["parts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let defaults = translate_text(request(true));
    assert!(!defaults.contains("must stay ignored"));
    assert!(defaults.contains("must stay visible"));
    assert!(!defaults.contains("default excluded secret"));

    let no_defaults = translate_text(request(false));
    assert!(!no_defaults.contains("must stay ignored"));
    assert!(no_defaults.contains("must stay visible"));
    assert!(no_defaults.contains("default excluded secret"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn gemini_response_translation_preserves_native_code_media_cache_and_metadata() {
    let response = serde_json::json!({
        "responseId": "resp_native",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [
                {"executableCode": {"language": "PYTHON", "code": "print(2 + 2)"}},
                {"codeExecutionResult": {"outcome": "OUTCOME_OK", "output": "4"}},
                {"videoMetadata": {"startOffset": "1s", "endOffset": "3s"}},
                {"inlineData": {"mimeType": "image/png", "data": "aW1hZ2U="}}
            ]},
            "finishReason": "STOP",
            "safetyRatings": [{"category": "HARM_CATEGORY_DANGEROUS_CONTENT", "probability": "NEGLIGIBLE"}],
            "avgLogprobs": -0.25,
            "logprobsResult": {"topCandidates": [{"candidates": [{"token": "hello", "logProbability": -0.1}]}]},
            "citationMetadata": {"citations": [{"uri": "https://citation.example", "title": "Citation"}]},
            "urlContextMetadata": {"urlMetadata": [{"retrievedUrl": "https://context.example", "urlRetrievalStatus": "URL_RETRIEVAL_STATUS_SUCCESS"}]}
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

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 7);
    let output = translated["output"].as_array().unwrap();
    let message = output
        .iter()
        .find(|item| item["type"] == "message")
        .unwrap();
    let message_text = message["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let image = output
        .iter()
        .find(|item| item["type"] == "image_generation_call")
        .unwrap();
    let web_search = output
        .iter()
        .find(|item| item["type"] == "web_search_call")
        .unwrap();
    let sources = web_search["action"]["sources"].as_array().unwrap();

    assert!(message_text.contains("Gemini executable code"));
    assert!(message_text.contains("Gemini code execution result"));
    assert!(message_text.contains("Gemini video metadata"));
    assert_eq!(image["result"], "aW1hZ2U=");
    assert!(
        sources
            .iter()
            .any(|source| source["url"] == "https://citation.example")
    );
    assert!(
        sources
            .iter()
            .any(|source| source["url"] == "https://context.example")
    );
    assert_eq!(web_search["action"]["type"], "open_page");
    assert_eq!(web_search["action"]["url"], "https://citation.example");
    assert_eq!(
        translated["usage"]["input_tokens_details"]["cached_tokens"],
        12
    );
    assert_eq!(
        translated["usage"]["output_tokens_details"]["reasoning_tokens"],
        4
    );
    assert_eq!(
        translated["usage"]["input_tokens_details"]["tool_tokens"],
        3
    );
    assert_eq!(
        translated["metadata"]["gemini"]["safetyRatings"][0]["probability"],
        "NEGLIGIBLE"
    );
    assert_eq!(
        translated["metadata"]["gemini"]["urlContextMetadata"]["urlMetadata"][0]["urlRetrievalStatus"],
        "URL_RETRIEVAL_STATUS_SUCCESS"
    );
    assert_eq!(
        translated["metadata"]["gemini"]["logprobsResult"]["topCandidates"][0]["candidates"][0]["token"],
        "hello"
    );
    assert!(output.iter().any(|item| {
        item["type"] == "message"
            && item["content"][0]["text"]
                .as_str()
                .is_some_and(|text| text == "Citations:\n(Citation) https://citation.example")
    }));
    let assistant = runtime_gemini_chat_assistant_messages_from_generate_value(&response, 7);
    assert_eq!(
        assistant[0]["gemini_native_parts"][0]["videoMetadata"]["startOffset"],
        "1s"
    );
}

#[test]
fn gemini_response_keeps_non_image_media_in_followup_without_visible_base64_dump() {
    let store = conversation_store();
    let response = serde_json::json!({
        "responseId": "resp_audio",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{
                "inlineData": {"mimeType": "audio/wav", "data": "UklGRg=="}
            }]},
            "finishReason": "STOP"
        }]
    });
    let translated = runtime_gemini_responses_value_from_generate_value(&response, 7);
    let visible_text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();
    let assistant = runtime_gemini_chat_assistant_messages_from_generate_value(&response, 7);

    assert!(visible_text.contains("audio/wav"));
    assert!(!visible_text.contains("UklGRg=="));
    assert_eq!(
        assistant[0]["gemini_native_parts"][0]["inlineData"]["data"],
        "UklGRg=="
    );

    runtime_deepseek_store_conversation(
        &store,
        "resp_audio",
        vec![serde_json::json!({"role": "user", "content": "make audio"})],
        assistant,
    );
    let followup = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "previous_response_id": "resp_audio",
        "input": "Describe the audio"
    }))
    .unwrap();
    let translated =
        runtime_gemini_generate_request_body(&followup, &store, false, None, None).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let model_parts = value["contents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|content| content["role"] == "model")
        .unwrap()["parts"]
        .as_array()
        .unwrap();

    assert!(model_parts.iter().any(|part| {
        part["inlineData"]["mimeType"] == "audio/wav" && part["inlineData"]["data"] == "UklGRg=="
    }));
}
