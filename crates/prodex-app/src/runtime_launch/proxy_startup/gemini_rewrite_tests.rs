#![cfg(test)]

use super::gemini_rewrite_test_support::conversation_store;
use super::{
    runtime_gemini_generate_request_body, runtime_gemini_request_body_without_tool,
    runtime_gemini_responses_value_from_generate_value,
};

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
