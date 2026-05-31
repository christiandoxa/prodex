use super::deepseek_rewrite::{
    RuntimeDeepSeekChatSseReader, RuntimeDeepSeekConversationStore,
    runtime_deepseek_chat_request_body,
};
use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};

fn deepseek_conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
}

#[test]
fn deepseek_request_translation_maps_responses_input_and_tools() {
    let conversations = deepseek_conversation_store();
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "stream": true,
        "instructions": "Be concise.",
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "List files"}]
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "README.md"
            }
        ],
        "tools": [
            {
                "type": "function",
                "name": "shell",
                "description": "Run shell",
                "parameters": {"type": "object"}
            },
            {"type": "web_search_preview"}
        ],
        "tool_choice": {
            "type": "function",
            "name": "shell"
        },
        "reasoning": {
            "effort": "xhigh"
        },
        "max_output_tokens": 123
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["model"], "deepseek-v4-pro");
    assert_eq!(translated["stream"], true);
    assert_eq!(translated["max_tokens"], 123);
    assert_eq!(translated["messages"][0]["role"], "system");
    assert_eq!(translated["messages"][1]["content"], "List files");
    assert_eq!(translated["messages"].as_array().unwrap().len(), 2);
    assert_eq!(translated["tools"].as_array().unwrap().len(), 1);
    assert_eq!(translated["tools"][0]["function"]["name"], "shell");
    assert!(translated.get("tool_choice").is_none());
    assert_eq!(translated["thinking"]["type"], "enabled");
    assert_eq!(translated["reasoning_effort"], "max");
}

#[test]
fn deepseek_request_translation_prepends_previous_response_history() {
    let conversations = deepseek_conversation_store();
    conversations.lock().unwrap().insert(
        "resp_prev".to_string(),
        vec![
            serde_json::json!({"role": "user", "content": "old prompt"}),
            serde_json::json!({"role": "assistant", "content": "old answer"}),
        ],
    );
    let body = serde_json::json!({
        "model": "deepseek-v4-pro",
        "previous_response_id": "resp_prev",
        "input": "new prompt"
    });

    let translated =
        runtime_deepseek_chat_request_body(&serde_json::to_vec(&body).unwrap(), &conversations)
            .expect("request should translate");
    let translated: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(translated["messages"][0]["content"], "old prompt");
    assert_eq!(translated["messages"][1]["content"], "old answer");
    assert_eq!(translated["messages"][2]["content"], "new prompt");
}

#[test]
fn deepseek_sse_reader_maps_text_and_tool_calls_to_responses_events() {
    let conversations = deepseek_conversation_store();
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_1\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"shell\",\"arguments\":\"{\\\"cmd\\\":\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"ls\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        conversations,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.created"));
    assert!(output.contains("\"type\":\"response.output_text.delta\""));
    assert!(output.contains("\"delta\":\"hi\""));
    assert!(output.contains("\"type\":\"response.output_item.added\""));
    assert!(output.contains("\"type\":\"response.function_call_arguments.delta\""));
    assert!(output.contains("\"type\":\"response.output_item.done\""));
    assert!(output.contains("\"arguments\":\"{\\\"cmd\\\":\\\"rtk ls\\\"}\""));
    assert!(output.contains("event: response.completed"));
}
