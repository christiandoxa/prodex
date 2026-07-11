use super::gemini_rewrite_test_support::{
    conversation_store, gemini_test_function_call, gemini_test_function_response,
};
use super::{runtime_deepseek_store_conversation, runtime_gemini_generate_request_body};

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
