use super::*;
use serde_json::json;

#[test]
fn gemini_chat_tool_call_item_keeps_signature_and_wrapped_arguments() {
    assert_eq!(
        gemini_chat_assistant_tool_call_item_with_call_id(
            &json!({"thoughtSignature": "part"}),
            &json!({
                "id": "input",
                "name": "shell",
                "args": {"cmd": "cargo check"},
                "thoughtSignature": "function",
            }),
            Some("override"),
        ),
        json!({
            "id": "input",
            "type": "function",
            "function": {"name": "shell", "arguments": "{\"cmd\":\"rtk cargo check\"}"},
            "gemini_thought_signature": "part",
        })
    );
    assert_eq!(
        gemini_chat_assistant_tool_call_item_with_call_id(&json!([]), &json!([]), None),
        json!({
            "id": "call_1",
            "type": "function",
            "function": {"name": "tool_call", "arguments": "{}"},
        })
    );
}

#[test]
fn gemini_provider_core_shapes_completed_stream_tool_call_items() {
    assert_eq!(
        gemini_provider_core_stream_completed_tool_call_arguments("apply_patch", "*** Begin Patch"),
        "*** Begin Patch"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &gemini_provider_core_stream_completed_tool_call_arguments(
                "shell",
                r#"{"cmd":"cd /repo && cargo check -q"}"#,
            )
        )
        .unwrap()["cmd"],
        "cd /repo && rtk cargo check -q"
    );
    assert_eq!(
        gemini_provider_core_stream_tool_call_arguments_value("{\"cmd\":\"pwd\"}")["cmd"],
        "pwd"
    );
    assert_eq!(
        gemini_provider_core_stream_tool_call_arguments_value("not-json"),
        json!("not-json")
    );

    let added = gemini_provider_core_stream_tool_call_added_item(
        "call_added",
        "plain_tool",
        Some("sig_added"),
    )
    .unwrap();
    assert_eq!(added["type"], "function_call");
    assert_eq!(added["call_id"], "call_added");
    assert_eq!(added["name"], "plain_tool");
    assert_eq!(added["gemini_thought_signature"], "sig_added");
    assert_eq!(
        gemini_provider_core_stream_tool_call_added_item("call_search", "tool_search", None),
        None
    );

    let blocked = gemini_provider_core_stream_completed_tool_call_item(
        "call_blocked",
        "shell",
        "blocked by policy",
        None,
        true,
    );
    assert_eq!(blocked["type"], "message");
    assert_eq!(blocked["content"][0]["text"], "blocked by policy");

    let item = gemini_provider_core_stream_completed_tool_call_item(
        "call_1",
        "plain_tool",
        "{\"path\":\"README.md\"}",
        Some("sig"),
        false,
    );
    assert_eq!(item["type"], "function_call");
    assert_eq!(item["call_id"], "call_1");
    assert_eq!(item["name"], "plain_tool");
    assert_eq!(item["arguments"], "{\"path\":\"README.md\"}");
    assert_eq!(item["gemini_thought_signature"], "sig");

    let raw = gemini_provider_core_stream_completed_tool_call_item(
        "call_raw",
        "raw_tool",
        "not-json",
        Some("sig_raw"),
        false,
    );
    assert_eq!(raw["call_id"], "call_raw");
    assert_eq!(raw["arguments"], "not-json");
    assert_eq!(raw["gemini_thought_signature"], "sig_raw");

    let search = gemini_provider_core_stream_completed_tool_call_item(
        "call_search",
        "tool_search",
        "{\"query\":\"sqz tools\"}",
        None,
        false,
    );
    assert_eq!(search["type"], "tool_search_call");
    assert_eq!(search["call_id"], "call_search");
    assert_eq!(search["arguments"]["query"], "sqz tools");

    assert_eq!(
        gemini_provider_core_stream_completed_tool_call_item(
            "call_patch",
            "apply_patch",
            r#"{"input":"*** Begin Patch\n*** End Patch"}"#,
            None,
            false,
        ),
        json!({
            "type": "custom_tool_call",
            "call_id": "call_patch",
            "name": "apply_patch",
            "input": "*** Begin Patch\n*** End Patch",
        })
    );

    assert_eq!(
        gemini_provider_core_stream_completed_tool_call_item(
            "call_namespaced",
            "mcp__prodex_sqz__compress",
            "{}",
            None,
            false,
        ),
        json!({
            "type": "function_call",
            "call_id": "call_namespaced",
            "name": "compress",
            "arguments": "{}",
            "namespace": "mcp__prodex_sqz",
        })
    );
}

#[test]
fn gemini_provider_core_shapes_namespaced_and_special_tool_items() {
    let namespaced = crate::gemini_provider_core_response_tool_call_item(
        &json!({"thoughtSignature": "part-🌱"}),
        &json!({
            "id": "call_from_input",
            "name": "mcp__prodex_sqz__compress",
            "args": {"text": "雪🙂\n"},
            "thoughtSignature": "call-signature",
        }),
        Some("call_override"),
        |_, _| None,
    );
    assert_eq!(
        namespaced,
        json!({
            "type": "function_call",
            "call_id": "call_from_input",
            "name": "compress",
            "arguments": "{\"text\":\"雪🙂\\n\"}",
            "namespace": "mcp__prodex_sqz",
            "gemini_thought_signature": "part-🌱",
        })
    );

    let malformed = crate::gemini_provider_core_response_tool_call_item(
        &json!({"thoughtSignature": false}),
        &json!({"id": 23, "name": false, "args": "not-json", "thoughtSignature": []}),
        None,
        |_, _| None,
    );
    assert_eq!(
        malformed,
        json!({
            "type": "function_call",
            "call_id": "call_1",
            "name": "tool_call",
            "arguments": "\"not-json\"",
        })
    );
    assert_eq!(
        crate::gemini_provider_core_response_tool_call_item(
            &json!([]),
            &json!([]),
            None,
            |_, _| None,
        ),
        json!({
            "type": "function_call",
            "call_id": "call_1",
            "name": "tool_call",
            "arguments": "{}",
        })
    );

    let rtk = crate::gemini_provider_core_response_tool_call_item(
        &json!({}),
        &json!({"name": "shell", "args": {"cmd": "cd /repo && cargo check -q"}}),
        None,
        |_, _| None,
    );
    assert_eq!(
        rtk,
        json!({
            "type": "function_call",
            "call_id": "call_1",
            "name": "shell",
            "arguments": "{\"cmd\":\"cd /repo && rtk cargo check -q\"}",
        })
    );

    assert_eq!(
        crate::gemini_provider_core_response_tool_call_item(
            &json!({}),
            &json!({"name": "tool_search", "args": {"query": "prodex"}}),
            None,
            |_, _| None,
        ),
        json!({
            "type": "tool_search_call",
            "call_id": "call_1",
            "execution": "client",
            "arguments": {"query": "prodex"},
        })
    );
    assert_eq!(
        crate::gemini_provider_core_response_tool_call_item(
            &json!({}),
            &json!({
                "name": "apply_patch",
                "args": {"file_path": "README.md", "old_string": "old", "new_string": "new"},
            }),
            None,
            |_, _| None,
        ),
        json!({
            "type": "custom_tool_call",
            "call_id": "call_1",
            "name": "apply_patch",
            "input": "*** Begin Patch\n*** Update File: README.md\n@@\n-old\n+new\n*** End Patch",
        })
    );

    assert_eq!(
        crate::gemini_provider_core_response_tool_call_raw_item(
            &json!({"thoughtSignature": "raw-雪"}),
            "服务器--搜索",
            "not-json",
            None,
        ),
        json!({
            "type": "function_call",
            "call_id": "call_1",
            "name": "搜索",
            "arguments": "not-json",
            "namespace": "服务器",
            "gemini_thought_signature": "raw-雪",
        })
    );

    assert_eq!(
        crate::gemini_provider_core_response_tool_call_added_item(
            &json!({"thoughtSignature": "added"}),
            &json!({"name": "mcp__server__lookup"}),
            None,
        ),
        Some(json!({
            "type": "function_call",
            "call_id": "call_1",
            "name": "lookup",
            "namespace": "mcp__server",
            "gemini_thought_signature": "added",
        }))
    );
    assert_eq!(
        crate::gemini_provider_core_response_tool_call_added_item(
            &json!({}),
            &json!({"name": "tool_search"}),
            None,
        ),
        None
    );
    assert_eq!(
        crate::gemini_provider_core_response_tool_call_raw_item(
            &json!({}),
            "server-- ",
            "{}",
            None,
        ),
        json!({
            "type": "function_call",
            "call_id": "call_1",
            "name": "server-- ",
            "arguments": "{}",
        })
    );
}
