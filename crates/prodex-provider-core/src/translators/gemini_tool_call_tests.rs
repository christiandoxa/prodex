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
fn gemini_response_signature_selection_preserves_invalid_and_present_fields() {
    let invalid_part = gemini_chat_assistant_tool_call_item_with_call_id(
        &json!({"thoughtSignature": false}),
        &json!({"name": "lookup", "thoughtSignature": "function"}),
        None,
    );
    assert!(invalid_part.get("gemini_thought_signature").is_none());

    let null_part = gemini_chat_assistant_tool_call_item_with_call_id(
        &json!({"thoughtSignature": null}),
        &json!({"name": "lookup", "thoughtSignature": "function"}),
        None,
    );
    assert!(null_part.get("gemini_thought_signature").is_none());

    let whitespace_part = gemini_chat_assistant_tool_call_item_with_call_id(
        &json!({"thoughtSignature": "\u{2003}\u{3000}"}),
        &json!({"name": "lookup", "thoughtSignature": "function"}),
        None,
    );
    assert!(whitespace_part.get("gemini_thought_signature").is_none());

    let unicode_part = gemini_chat_assistant_tool_call_item_with_call_id(
        &json!({"thoughtSignature": "\u{2003}雪\u{3000}sig"}),
        &json!({"name": "lookup", "thoughtSignature": "function"}),
        None,
    );
    assert_eq!(
        unicode_part["gemini_thought_signature"],
        "\u{2003}雪\u{3000}sig"
    );
}

#[test]
fn gemini_apply_patch_unified_diff_is_mojo_owned() {
    let input = concat!(
        "diff --git a/README.md b/docs/README.md\r\n",
        "--- a/README.md\r\n",
        "+++ b/docs/README.md\r\n",
        "@@ -1 +1 @@ title\r\n",
        "-old\r\n",
        "+new\r\n",
    );
    assert_eq!(
        gemini_custom_apply_patch_input(&json!({"diff": input})),
        concat!(
            "*** Begin Patch\n",
            "*** Update File: README.md\n",
            "*** Move to: docs/README.md\n",
            "@@ title\n",
            "-old\n",
            "+new\n",
            "*** End Patch",
        )
    );

    let add = concat!(
        "--- /dev/null\n",
        "+++ b/new.txt\n",
        "@@ -0,0 +1,2 @@\n",
        "+hello\n",
        "+world\n",
    );
    assert_eq!(
        gemini_custom_apply_patch_input(&json!({"patch": add})),
        concat!(
            "*** Begin Patch\n",
            "*** Add File: new.txt\n",
            "+hello\n",
            "+world\n",
            "*** End Patch",
        )
    );

    let delete = concat!("--- a/gone.txt\n", "+++ /dev/null\n",);
    assert_eq!(
        gemini_custom_apply_patch_input(&json!({"text": delete})),
        concat!(
            "*** Begin Patch\n",
            "*** Delete File: gone.txt\n",
            "*** End Patch",
        )
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

    let blocked_valid_json = gemini_provider_core_stream_completed_tool_call_item(
        "call_blocked_json",
        "tool_search",
        "{ \"query\" : \"x\" }",
        None,
        true,
    );
    assert_eq!(
        blocked_valid_json["content"][0]["text"],
        "{ \"query\" : \"x\" }"
    );

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

    assert_eq!(
        gemini_provider_core_stream_completed_tool_call_item(
            "call_shell",
            "shell",
            r#"{"cmd":"ls"}"#,
            None,
            false,
        )["arguments"],
        r#"{"cmd":"rtk ls"}"#,
    );

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

    let malformed_search = gemini_provider_core_stream_completed_tool_call_item(
        "call_search_raw",
        "tool_search",
        "not-json",
        None,
        false,
    );
    assert_eq!(malformed_search["type"], "function_call");
    assert_eq!(malformed_search["arguments"], "not-json");

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
