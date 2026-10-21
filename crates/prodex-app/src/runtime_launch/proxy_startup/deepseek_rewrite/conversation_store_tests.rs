use super::{RuntimeDeepSeekConversationStore, RuntimeProviderConversation};
use prodex_provider_core::provider_core_conversation_estimated_bytes;
use std::cell::RefCell;

#[test]
fn conversation_store_evicts_oldest_entry_per_scope() {
    let conversations = RuntimeDeepSeekConversationStore::default();
    let scoped = conversations.scoped("tenant-a");
    let unrelated = conversations.scoped("tenant-b");
    unrelated.insert("first", vec![serde_json::json!("other")]);
    for index in 0..=256 {
        scoped.insert(
            &format!("resp-{index}"),
            vec![serde_json::json!({"role": "user", "content": index})],
        );
    }
    assert_eq!(scoped.history("resp-0"), None);
    assert_eq!(
        scoped.history("resp-256"),
        Some(vec![serde_json::json!({"role": "user", "content": 256})])
    );
    assert_eq!(
        unrelated.history("first"),
        Some(vec![serde_json::json!("other")])
    );
    assert_eq!(conversations.len(), 257);
}

#[test]
fn conversation_store_history_keeps_scope_isolation_predicate_order_and_ties() {
    let root = RuntimeDeepSeekConversationStore::default();
    let scope = root.scoped("a");
    let colliding = root.scoped("a:b");
    let unicode = root.scoped("é");
    scope.insert("z", vec![serde_json::json!("first")]);
    scope.insert("a", vec![serde_json::json!("latest")]);
    colliding.insert("a", vec![serde_json::json!("different")]);
    unicode.insert("a", vec![serde_json::json!("unicode")]);
    root.insert("root", vec![serde_json::json!("root")]);
    let visited = RefCell::new(Vec::new());
    assert_eq!(
        scope.find_history(|messages| {
            visited.borrow_mut().push(messages[0].clone());
            true
        }),
        Some(vec![serde_json::json!("latest")])
    );
    assert_eq!(
        *visited.borrow(),
        [serde_json::json!("latest"), serde_json::json!("first")]
    );
    assert_eq!(
        root.find_history(|_| true),
        Some(vec![serde_json::json!("root")])
    );
    assert_eq!(
        unicode.find_history(|_| true),
        Some(vec![serde_json::json!("unicode")])
    );
    {
        let mut state = root.state.lock().unwrap();
        state.next_sequence = u64::MAX;
        for row in state.conversations.values_mut() {
            row.sequence = u64::MAX;
        }
    }
    assert_eq!(
        scope.find_history(|_| true),
        Some(vec![serde_json::json!("first")])
    );
}

#[test]
fn conversation_store_replacement_updates_accounting_and_oversize_removes_old_value() {
    let root = RuntimeDeepSeekConversationStore::default();
    root.insert("a", vec![serde_json::json!("old")]);
    let replacement = vec![serde_json::json!({"nested": [true, null, "é"]})];
    root.insert("a", replacement.clone());
    {
        let state = root.state.lock().unwrap();
        assert_eq!(
            state.estimated_bytes,
            provider_core_conversation_estimated_bytes("a", &replacement).unwrap()
        );
        assert_eq!(state.next_sequence, 2);
    }
    root.insert(
        "a",
        vec![serde_json::Value::String("x".repeat(64 * 1024 * 1024))],
    );
    assert!(!root.contains("a"));
    let state = root.state.lock().unwrap();
    assert_eq!(state.estimated_bytes, 0);
    assert_eq!(state.next_sequence, 2);
}

#[test]
fn conversation_store_applies_global_count_and_byte_eviction_plans() {
    let root = RuntimeDeepSeekConversationStore::default();
    for index in 0..2048 {
        root.scoped(&format!("tenant-{index:04}"))
            .insert("response", vec![]);
    }
    root.insert("new", vec![]);
    assert_eq!(root.len(), 2048);
    assert!(!root.scoped("tenant-0000").contains("response"));
    assert!(root.contains("new"));
    {
        let mut state = root.state.lock().unwrap();
        state.conversations.clear();
        state.conversations.insert(
            "old".into(),
            RuntimeProviderConversation {
                messages: vec![],
                sequence: 1,
                estimated_bytes: 64 * 1024 * 1024,
            },
        );
        state.estimated_bytes = 64 * 1024 * 1024;
    }
    root.insert("new", vec![]);
    assert!(!root.contains("old"));
    assert_eq!(root.state.lock().unwrap().estimated_bytes, "new".len());
}

#[test]
fn conversation_store_accounts_nested_json_and_scoped_utf8_keys() {
    let root = RuntimeDeepSeekConversationStore::default();
    let scope = root.scoped("é");
    let messages = vec![serde_json::json!({"a": [null, true, 7, "é"], "empty": {}})];
    scope.insert("resp", messages.clone());
    let expected = "\0prodex:2:é:resp".len()
        + 7 * std::mem::size_of::<serde_json::Value>()
        + std::mem::size_of::<bool>()
        + std::mem::size_of::<serde_json::Number>()
        + "a".len()
        + "empty".len()
        + "é".len();
    assert_eq!(scope.history("resp"), Some(messages));
    let state = root.state.lock().unwrap();
    assert_eq!(state.estimated_bytes, expected);
    assert_eq!(
        state.conversations["\0prodex:2:é:resp"].estimated_bytes,
        expected
    );
}
