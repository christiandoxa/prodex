use super::RuntimeDeepSeekConversationStore;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub(super) fn conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
}

pub(super) fn gemini_test_function_call<'a>(
    value: &'a serde_json::Value,
    call_id: &str,
) -> &'a serde_json::Value {
    value["contents"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|content| {
            content["parts"]
                .as_array()
                .map(|parts| parts.iter())
                .into_iter()
                .flatten()
        })
        .find(|part| part["functionCall"]["id"] == call_id)
        .expect("functionCall should exist")
}

pub(super) fn gemini_test_function_response<'a>(
    value: &'a serde_json::Value,
    call_id: &str,
) -> &'a serde_json::Value {
    value["contents"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|content| {
            content["parts"]
                .as_array()
                .map(|parts| parts.iter())
                .into_iter()
                .flatten()
        })
        .find(|part| part["functionResponse"]["id"] == call_id)
        .expect("functionResponse should exist")
}
