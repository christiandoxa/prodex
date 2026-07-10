#![allow(dead_code)]

use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
use super::deepseek_sse::RuntimeDeepSeekSseState;
use super::provider_bridge::RuntimeProviderBridgeKind;
use super::provider_sse_reader::{
    RuntimeProviderSseJsonReader, RuntimeProviderSseJsonState, RuntimeProviderSseObserver,
};
use std::io::{self, Read};

pub(super) struct RuntimeChatCompatibleSseReader<R: Read> {
    inner: RuntimeProviderSseJsonReader<R, RuntimeDeepSeekSseState>,
}

impl<R: Read> RuntimeChatCompatibleSseReader<R> {
    #[cfg(test)]
    pub(super) fn new(
        reader: R,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        response_metadata: Option<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
    ) -> Self {
        Self::new_with_observer(
            reader,
            request_id,
            conversation_messages,
            response_metadata,
            conversations,
            None,
        )
    }

    pub(super) fn new_with_observer(
        reader: R,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        response_metadata: Option<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
        observer: Option<RuntimeProviderSseObserver>,
    ) -> Self {
        Self::new_with_provider_and_observer(
            reader,
            RuntimeProviderBridgeKind::DeepSeek,
            request_id,
            conversation_messages,
            response_metadata,
            conversations,
            observer,
        )
    }

    pub(super) fn new_with_provider_and_observer(
        reader: R,
        provider_kind: RuntimeProviderBridgeKind,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        response_metadata: Option<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
        observer: Option<RuntimeProviderSseObserver>,
    ) -> Self {
        Self {
            inner: RuntimeProviderSseJsonReader::new_with_observer(
                reader,
                RuntimeDeepSeekSseState::new_with_provider(
                    provider_kind,
                    request_id,
                    conversation_messages,
                    response_metadata,
                    conversations,
                ),
                observer,
            ),
        }
    }
}

impl RuntimeProviderSseJsonState for RuntimeDeepSeekSseState {
    fn eof(&self) -> bool {
        self.eof
    }

    fn set_eof(&mut self, eof: bool) {
        self.eof = eof;
    }

    fn observe_value(&mut self, value: &serde_json::Value) -> Vec<String> {
        self.observe_chat_chunk(value)
    }

    fn complete_event(&mut self) -> Option<String> {
        RuntimeDeepSeekSseState::complete_event(self)
    }

    fn failed_event(&mut self, code: &str, message: &str) -> Option<String> {
        RuntimeDeepSeekSseState::failed_event(self, code, message)
    }
}

impl<R: Read> Read for RuntimeChatCompatibleSseReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

pub(super) type RuntimeDeepSeekChatSseReader<R> = RuntimeChatCompatibleSseReader<R>;
