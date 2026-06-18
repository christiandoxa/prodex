use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
use super::deepseek_sse::RuntimeDeepSeekSseState;
use super::provider_sse_reader::{RuntimeProviderSseJsonReader, RuntimeProviderSseJsonState};
use std::io::{self, Read};

pub(super) struct RuntimeDeepSeekChatSseReader<R: Read> {
    inner: RuntimeProviderSseJsonReader<R, RuntimeDeepSeekSseState>,
}

impl<R: Read> RuntimeDeepSeekChatSseReader<R> {
    pub(super) fn new(
        reader: R,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
    ) -> Self {
        Self {
            inner: RuntimeProviderSseJsonReader::new(
                reader,
                RuntimeDeepSeekSseState::new(request_id, conversation_messages, conversations),
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

impl<R: Read> Read for RuntimeDeepSeekChatSseReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}
