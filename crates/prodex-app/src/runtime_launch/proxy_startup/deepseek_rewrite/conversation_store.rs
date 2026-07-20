use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

const RUNTIME_PROVIDER_CONVERSATION_SCOPE_PREFIX: &str = "\0prodex:";
const RUNTIME_PROVIDER_CONVERSATIONS_PER_SCOPE: usize = 256;
const RUNTIME_PROVIDER_CONVERSATIONS_TOTAL: usize = 2_048;
const RUNTIME_PROVIDER_CONVERSATIONS_MAX_ESTIMATED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
struct RuntimeProviderConversation {
    messages: Vec<serde_json::Value>,
    sequence: u64,
    estimated_bytes: usize,
}

#[derive(Debug, Default)]
struct RuntimeProviderConversationState {
    conversations: BTreeMap<String, RuntimeProviderConversation>,
    next_sequence: u64,
    estimated_bytes: usize,
}

impl RuntimeProviderConversationState {
    fn remove(&mut self, key: &str) {
        if let Some(removed) = self.conversations.remove(key) {
            self.estimated_bytes = self.estimated_bytes.saturating_sub(removed.estimated_bytes);
        }
    }
}

#[derive(Clone, Default)]
pub(in crate::runtime_launch::proxy_startup) struct RuntimeDeepSeekConversationStore {
    state: Arc<Mutex<RuntimeProviderConversationState>>,
    scope_prefix: Option<String>,
}

impl fmt::Debug for RuntimeDeepSeekConversationStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeDeepSeekConversationStore")
            .field("scope", &self.scope_prefix.as_ref().map(|_| "<redacted>"))
            .finish_non_exhaustive()
    }
}

impl RuntimeDeepSeekConversationStore {
    pub(in crate::runtime_launch::proxy_startup) fn scoped(&self, namespace: &str) -> Self {
        Self {
            state: Arc::clone(&self.state),
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
        self.state
            .lock()
            .ok()?
            .conversations
            .get(&self.storage_key(response_id))
            .map(|conversation| conversation.messages.clone())
    }

    pub(in crate::runtime_launch::proxy_startup) fn contains(&self, response_id: &str) -> bool {
        self.state.lock().ok().is_some_and(|state| {
            state
                .conversations
                .contains_key(&self.storage_key(response_id))
        })
    }

    pub(in crate::runtime_launch::proxy_startup) fn find_history(
        &self,
        predicate: impl Fn(&[serde_json::Value]) -> bool,
    ) -> Option<Vec<serde_json::Value>> {
        let state = self.state.lock().ok()?;
        state
            .conversations
            .iter()
            .filter(|(key, conversation)| {
                self.key_is_in_scope(key) && predicate(&conversation.messages)
            })
            .max_by_key(|(_, conversation)| conversation.sequence)
            .map(|(_, conversation)| conversation.messages.clone())
    }

    pub(in crate::runtime_launch::proxy_startup) fn insert(
        &self,
        response_id: &str,
        messages: Vec<serde_json::Value>,
    ) {
        let storage_key = self.storage_key(response_id);
        let estimated_bytes =
            runtime_provider_conversation_estimated_bytes(&storage_key, &messages);
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.remove(&storage_key);
        if estimated_bytes > RUNTIME_PROVIDER_CONVERSATIONS_MAX_ESTIMATED_BYTES {
            return;
        }
        state.next_sequence = state.next_sequence.saturating_add(1);
        let sequence = state.next_sequence;
        state.estimated_bytes = state.estimated_bytes.saturating_add(estimated_bytes);
        state.conversations.insert(
            storage_key.clone(),
            RuntimeProviderConversation {
                messages,
                sequence,
                estimated_bytes,
            },
        );

        while state
            .conversations
            .keys()
            .filter(|key| self.key_is_in_scope(key))
            .count()
            > RUNTIME_PROVIDER_CONVERSATIONS_PER_SCOPE
        {
            let stale_key = state
                .conversations
                .iter()
                .filter(|(key, _)| self.key_is_in_scope(key))
                .min_by_key(|(_, conversation)| conversation.sequence)
                .map(|(key, _)| key.clone());
            let Some(stale_key) = stale_key else {
                break;
            };
            state.remove(&stale_key);
        }
        while state.conversations.len() > RUNTIME_PROVIDER_CONVERSATIONS_TOTAL
            || state.estimated_bytes > RUNTIME_PROVIDER_CONVERSATIONS_MAX_ESTIMATED_BYTES
        {
            let stale_key = state
                .conversations
                .iter()
                .min_by_key(|(_, conversation)| conversation.sequence)
                .map(|(key, _)| key.clone());
            let Some(stale_key) = stale_key else {
                break;
            };
            state.remove(&stale_key);
        }
    }

    #[cfg(test)]
    pub(in crate::runtime_launch::proxy_startup) fn len(&self) -> usize {
        self.state
            .lock()
            .map(|state| state.conversations.len())
            .unwrap_or_default()
    }
}

fn runtime_provider_conversation_estimated_bytes(
    storage_key: &str,
    messages: &[serde_json::Value],
) -> usize {
    messages.iter().fold(storage_key.len(), |size, message| {
        size.saturating_add(runtime_provider_json_estimated_bytes(message))
    })
}

fn runtime_provider_json_estimated_bytes(value: &serde_json::Value) -> usize {
    const NODE_BYTES: usize = std::mem::size_of::<serde_json::Value>();
    let payload = match value {
        serde_json::Value::Null => 0,
        serde_json::Value::Bool(_) => std::mem::size_of::<bool>(),
        serde_json::Value::Number(_) => std::mem::size_of::<serde_json::Number>(),
        serde_json::Value::String(value) => value.len(),
        serde_json::Value::Array(values) => values.iter().fold(0_usize, |size, value| {
            size.saturating_add(runtime_provider_json_estimated_bytes(value))
        }),
        serde_json::Value::Object(values) => values.iter().fold(0_usize, |size, (key, value)| {
            size.saturating_add(key.len())
                .saturating_add(runtime_provider_json_estimated_bytes(value))
        }),
    };
    NODE_BYTES.saturating_add(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_store_evicts_oldest_entry_per_scope() {
        let conversations = RuntimeDeepSeekConversationStore::default();
        let scoped = conversations.scoped("tenant-a");
        for index in 0..=RUNTIME_PROVIDER_CONVERSATIONS_PER_SCOPE {
            scoped.insert(
                &format!("resp-{index}"),
                vec![serde_json::json!({"role": "user", "content": index})],
            );
        }

        assert!(scoped.history("resp-0").is_none());
        assert!(
            scoped
                .history(&format!("resp-{RUNTIME_PROVIDER_CONVERSATIONS_PER_SCOPE}"))
                .is_some()
        );
        assert_eq!(
            conversations.len(),
            RUNTIME_PROVIDER_CONVERSATIONS_PER_SCOPE
        );
    }
}
