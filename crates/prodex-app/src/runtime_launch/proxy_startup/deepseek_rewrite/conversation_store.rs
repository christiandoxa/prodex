use prodex_mojo_core::provider_conversation_policy::{self, ConversationEntry};
use prodex_provider_core::provider_core_conversation_estimated_bytes;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

const RUNTIME_PROVIDER_CONVERSATION_SCOPE_PREFIX: &str = "\0prodex:";

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
        let entries = state.conversations.iter().collect::<Vec<_>>();
        let facts = entries
            .iter()
            .map(|(key, conversation)| ConversationEntry {
                key,
                sequence: conversation.sequence,
                estimated_bytes: conversation.estimated_bytes,
            })
            .collect::<Vec<_>>();
        let selected = provider_conversation_policy::latest_matching(
            &facts,
            self.scope_prefix.as_deref(),
            |index| predicate(&entries[index].1.messages),
        )
        .ok()??;
        entries
            .get(selected)
            .map(|(_, conversation)| conversation.messages.clone())
    }

    pub(in crate::runtime_launch::proxy_startup) fn insert(
        &self,
        response_id: &str,
        messages: Vec<serde_json::Value>,
    ) {
        let storage_key = self.storage_key(response_id);
        let Ok(estimated_bytes) =
            provider_core_conversation_estimated_bytes(&storage_key, &messages)
        else {
            return;
        };
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let facts = state
            .conversations
            .iter()
            .map(|(key, conversation)| ConversationEntry {
                key,
                sequence: conversation.sequence,
                estimated_bytes: conversation.estimated_bytes,
            })
            .collect::<Vec<_>>();
        let Ok(plan) = provider_conversation_policy::retention_plan(
            &facts,
            self.scope_prefix.as_deref(),
            &storage_key,
            estimated_bytes,
            state.next_sequence,
        ) else {
            return;
        };
        let mut keep = plan.keep.into_iter();
        state.conversations.retain(|_, _| keep.next() == Some(true));
        if plan.keep_incoming {
            state.conversations.insert(
                storage_key,
                RuntimeProviderConversation {
                    messages,
                    sequence: plan.next_sequence,
                    estimated_bytes,
                },
            );
        }
        state.next_sequence = plan.next_sequence;
        state.estimated_bytes = plan.estimated_bytes;
    }

    #[cfg(test)]
    pub(in crate::runtime_launch::proxy_startup) fn len(&self) -> usize {
        self.state
            .lock()
            .map(|state| state.conversations.len())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "conversation_store_tests.rs"]
mod tests;
