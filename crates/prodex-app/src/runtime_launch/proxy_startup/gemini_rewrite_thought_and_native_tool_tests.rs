#![cfg(test)]

use super::gemini_rewrite_test_support::conversation_store;
use super::{
    runtime_deepseek_store_conversation, runtime_gemini_blocked_tool_call_message,
    runtime_gemini_generate_request_body, runtime_gemini_harden_tool_call_thought_signatures,
    runtime_gemini_responses_value_from_generate_value,
};

use prodex_provider_core::{
    gemini_provider_core_chat_assistant_messages, gemini_provider_core_request_body_without_tool,
};

fn gemini_chat_assistant_messages_from_generate_value(
    value: &serde_json::Value,
    request_id: u64,
) -> Vec<serde_json::Value> {
    gemini_provider_core_chat_assistant_messages(value, request_id, |name, args| {
        runtime_gemini_blocked_tool_call_message(name, args)
    })
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
        gemini_chat_assistant_messages_from_generate_value(&response, 12),
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

    let stripped =
        gemini_provider_core_request_body_without_tool(&translated.body, "codeExecution")
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
    let stripped = gemini_provider_core_request_body_without_tool(&translated.body, "computerUse")
        .expect("computerUse should be removable for unsupported-model fallback");
    let stripped: serde_json::Value = serde_json::from_slice(&stripped).unwrap();
    assert!(stripped.get("tools").is_none());
}
