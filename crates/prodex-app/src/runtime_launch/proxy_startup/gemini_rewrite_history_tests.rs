use super::gemini_rewrite_test_support::{
    conversation_store, gemini_test_function_call, gemini_test_function_response,
};
use super::runtime_gemini_generate_request_body;

#[test]
fn gemini_request_translation_injects_missing_tool_response_sentinel() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "gemini_session": {
            "contents": [{
                "role": "model",
                "parts": [{
                    "functionCall": {
                        "id": "call_lost_1",
                        "name": "exec_command",
                        "args": {"cmd": "pwd"}
                    }
                }]
            }]
        },
        "input": "continue"
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
    let function_response = gemini_test_function_response(&value, "call_lost_1");

    assert_eq!(
        function_response["functionResponse"]["name"],
        "exec_command"
    );
    assert!(
        function_response["functionResponse"]["response"]["error"]
            .as_str()
            .unwrap()
            .contains("missing from the Codex conversation history")
    );
}

#[test]
fn gemini_request_translation_repairs_orphan_tool_response() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "gemini_session": {
            "contents": [{
                "role": "user",
                "parts": [{
                    "functionResponse": {
                        "id": "call_orphan_1",
                        "name": "tool_call",
                        "response": {"output": "orphan output"}
                    }
                }]
            }]
        },
        "input": "continue"
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
    let function_call = gemini_test_function_call(&value, "call_orphan_1");
    let function_response = gemini_test_function_response(&value, "call_orphan_1");

    assert_eq!(function_call["functionCall"]["name"], "tool_call");
    assert_eq!(
        function_call["thoughtSignature"],
        "skip_thought_signature_validator"
    );
    assert_eq!(
        function_response["functionResponse"]["response"]["output"],
        "orphan output"
    );
}

#[test]
fn gemini_request_translation_reorders_parallel_tool_responses_to_call_order() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "gemini_session": {
            "contents": [
                {
                    "role": "user",
                    "parts": [{"text": "run two commands"}]
                },
                {
                    "role": "model",
                    "parts": [
                        {
                            "functionCall": {
                                "id": "call_first",
                                "name": "shell",
                                "args": {"cmd": "first"}
                            }
                        },
                        {
                            "functionCall": {
                                "id": "call_second",
                                "name": "shell",
                                "args": {"cmd": "second"}
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "parts": [
                        {
                            "functionResponse": {
                                "id": "call_second",
                                "name": "shell",
                                "response": {"output": "second"}
                            }
                        },
                        {
                            "functionResponse": {
                                "id": "call_first",
                                "name": "shell",
                                "response": {"output": "first"}
                            }
                        }
                    ]
                }
            ]
        },
        "input": "continue"
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
    let user_response_turn = value["contents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|content| {
            content["role"] == "user"
                && content["parts"]
                    .as_array()
                    .unwrap_or(&Vec::new())
                    .iter()
                    .any(|part| part["functionResponse"]["id"] == "call_first")
        })
        .expect("tool response turn should exist");
    let parts = user_response_turn["parts"].as_array().unwrap();

    assert_eq!(parts[0]["functionResponse"]["id"], "call_first");
    assert_eq!(parts[1]["functionResponse"]["id"], "call_second");
}
