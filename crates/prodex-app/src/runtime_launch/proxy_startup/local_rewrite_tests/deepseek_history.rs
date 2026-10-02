use super::super::deepseek_rewrite::runtime_deepseek_chat_request_body;
use super::deepseek::deepseek_conversation_store;

#[test]
fn deepseek_request_translation_replays_history_only_for_matching_tool_call_key() {
    let conversations = deepseek_conversation_store();
    conversations.insert(
        "resp_tool",
        vec![serde_json::json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "call_found",
                "type": "function",
                "function": {"name": "lookup", "arguments": "{}"}
            }]
        })],
    );
    let translate = |call_id: &str| {
        let body = serde_json::json!({
            "model": "deepseek-v4-pro",
            "input": [{
                "type": "function_call_output",
                "tool_call_id": call_id,
                "output": "done"
            }]
        });
        let translated =
            runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
                .expect("tool output request should translate");
        serde_json::from_slice::<serde_json::Value>(&translated.body).unwrap()
    };

    let replayed = translate("call_found");
    assert_eq!(replayed["messages"].as_array().unwrap().len(), 2);
    assert_eq!(replayed["messages"][0]["role"], "assistant");
    assert_eq!(replayed["messages"][0]["tool_calls"][0]["id"], "call_found");
    assert_eq!(replayed["messages"][1]["role"], "tool");
    assert_eq!(replayed["messages"][1]["tool_call_id"], "call_found");

    let missing = translate("call_missing");
    assert_eq!(missing["messages"].as_array().unwrap().len(), 1);
    assert_eq!(missing["messages"][0]["role"], "user");
    assert!(missing["messages"][0].get("tool_calls").is_none());
}
