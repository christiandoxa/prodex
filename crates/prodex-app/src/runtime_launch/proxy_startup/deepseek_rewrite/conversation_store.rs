#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const RUNTIME_PROVIDER_CONVERSATION_SCOPE_PREFIX: &str = "\0prodex:";

#[derive(Clone, Debug, Default)]
pub(in crate::runtime_launch::proxy_startup) struct RuntimeDeepSeekConversationStore {
    conversations: Arc<Mutex<BTreeMap<String, Vec<serde_json::Value>>>>,
    scope_prefix: Option<String>,
}

impl RuntimeDeepSeekConversationStore {
    pub(in crate::runtime_launch::proxy_startup) fn scoped(&self, namespace: &str) -> Self {
        Self {
            conversations: Arc::clone(&self.conversations),
            scope_prefix: Some(format!(
                "{RUNTIME_PROVIDER_CONVERSATION_SCOPE_PREFIX}{}:{namespace}:",
                namespace.len()
            )),
        }
    }

    fn storage_key(&self, response_id: &str) -> String {
        self.scope_prefix.as_deref().map_or_else(
            || response_id.to_string(),
            |prefix| format!("{prefix}{response_id}"),
        )
    }

    fn key_is_in_scope(&self, key: &str) -> bool {
        self.scope_prefix.as_deref().map_or_else(
            || !key.starts_with(RUNTIME_PROVIDER_CONVERSATION_SCOPE_PREFIX),
            |prefix| key.starts_with(prefix),
        )
    }

    pub(in crate::runtime_launch::proxy_startup) fn history(
        &self,
        response_id: &str,
    ) -> Option<Vec<serde_json::Value>> {
        self.conversations
            .lock()
            .ok()?
            .get(&self.storage_key(response_id))
            .cloned()
    }

    pub(in crate::runtime_launch::proxy_startup) fn contains(&self, response_id: &str) -> bool {
        self.conversations
            .lock()
            .ok()
            .is_some_and(|conversations| conversations.contains_key(&self.storage_key(response_id)))
    }

    pub(in crate::runtime_launch::proxy_startup) fn find_history(
        &self,
        predicate: impl Fn(&[serde_json::Value]) -> bool,
    ) -> Option<Vec<serde_json::Value>> {
        let conversations = self.conversations.lock().ok()?;
        conversations
            .iter()
            .rev()
            .filter(|(key, _)| self.key_is_in_scope(key))
            .find_map(|(_, history)| predicate(history).then(|| history.clone()))
    }

    pub(in crate::runtime_launch::proxy_startup) fn insert_bounded(
        &self,
        response_id: &str,
        messages: Vec<serde_json::Value>,
        limit: usize,
    ) {
        let storage_key = self.storage_key(response_id);
        if let Ok(mut conversations) = self.conversations.lock() {
            conversations.insert(storage_key.clone(), messages);
            while conversations.len() > limit {
                let Some(stale_id) = conversations
                    .keys()
                    .find(|candidate| candidate.as_str() != storage_key)
                    .cloned()
                else {
                    break;
                };
                conversations.remove(&stale_id);
            }
        }
    }

    pub(in crate::runtime_launch::proxy_startup) fn lock(
        &self,
    ) -> std::sync::LockResult<std::sync::MutexGuard<'_, BTreeMap<String, Vec<serde_json::Value>>>>
    {
        self.conversations.lock()
    }
}
