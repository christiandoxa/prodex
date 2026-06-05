use super::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_store_conversation,
    runtime_gemini_generate_request_body, runtime_gemini_responses_value_from_generate_value,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
}

#[test]
fn gemini_request_translation_maps_custom_apply_patch_to_function_declaration() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Edit a file",
        "tools": [{
            "type": "custom",
            "name": "apply_patch",
            "description": "Use the apply_patch tool to edit files.",
            "format": {
                "type": "grammar",
                "syntax": "lark",
                "definition": "start: begin_patch hunk+ end_patch"
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
        "apply_patch"
    );
    assert_eq!(
        value["tools"][0]["functionDeclarations"][0]["parameters"]["properties"]["input"]["type"],
        "string"
    );
}

#[test]
fn gemini_response_translation_maps_apply_patch_to_custom_tool_call() {
    let patch = "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch";
    let response = serde_json::json!({
        "responseId": "resp_patch_1",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "id": "call_patch_1",
                        "name": "apply_patch",
                        "args": {"input": patch}
                    }
                }]
            }
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 47);

    assert_eq!(translated["output"][0]["type"], "custom_tool_call");
    assert_eq!(translated["output"][0]["name"], "apply_patch");
    assert_eq!(translated["output"][0]["call_id"], "call_patch_1");
    assert_eq!(translated["output"][0]["input"], patch);
}

#[test]
fn gemini_response_translation_maps_tool_search_function_calls() {
    let response = serde_json::json!({
        "responseId": "resp_search_1",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "id": "call_search_1",
                        "name": "tool_search",
                        "args": {"query": "sqz read file", "limit": 2}
                    }
                }]
            }
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 45);

    assert_eq!(translated["output"][0]["type"], "tool_search_call");
    assert_eq!(translated["output"][0]["execution"], "client");
    assert_eq!(translated["output"][0]["call_id"], "call_search_1");
    assert_eq!(
        translated["output"][0]["arguments"]["query"],
        "sqz read file"
    );
}

#[test]
fn gemini_response_translation_preserves_grounding_metadata_as_web_search_call() {
    let response = serde_json::json!({
        "responseId": "resp_grounded_1",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{"text": "Gemini found one source."}]
            },
            "groundingMetadata": {
                "webSearchQueries": ["prodex gemini bridge"],
                "groundingChunks": [{
                    "web": {
                        "uri": "https://example.com/prodex-gemini",
                        "title": "Prodex Gemini"
                    }
                }]
            }
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 46);

    assert_eq!(translated["output"][0]["type"], "message");
    assert_eq!(translated["output"][1]["type"], "web_search_call");
    assert_eq!(translated["output"][1]["id"], "ws_resp_grounded_1");
    assert_eq!(translated["output"][1]["status"], "completed");
    assert_eq!(
        translated["output"][1]["action"]["queries"][0],
        "prodex gemini bridge"
    );
    assert_eq!(
        translated["output"][1]["action"]["sources"][0]["url"],
        "https://example.com/prodex-gemini"
    );
}

#[test]
fn gemini_request_translation_replays_custom_tool_output_as_function_response() {
    let conversations = conversation_store();
    let patch = "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch";
    runtime_deepseek_store_conversation(
        &conversations,
        "resp_patch_1",
        vec![serde_json::json!({"role": "user", "content": "edit"})],
        vec![serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "call_patch_1",
                "type": "function",
                "function": {
                    "name": "apply_patch",
                    "arguments": serde_json::to_string(&serde_json::json!({"input": patch})).unwrap()
                }
            }]
        })],
    );
    let followup = serde_json::json!({
        "model": "gemini-2.5-pro",
        "previous_response_id": "resp_patch_1",
        "input": [{
            "type": "custom_tool_call_output",
            "call_id": "call_patch_1",
            "output": "Success. Updated the following files:\nA note.txt\n"
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
        value["contents"][1]["parts"][0]["functionCall"]["name"],
        "apply_patch"
    );
    assert_eq!(
        value["contents"][2]["parts"][0]["functionResponse"]["name"],
        "apply_patch"
    );
    assert_eq!(
        value["contents"][2]["parts"][0]["functionResponse"]["response"]["output"],
        "Success. Updated the following files:\nA note.txt\n"
    );
}
