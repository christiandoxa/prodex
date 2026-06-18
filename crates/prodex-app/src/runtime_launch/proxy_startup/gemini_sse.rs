use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
use super::gemini_rewrite::runtime_gemini_normalized_response_value;
use super::provider_sse_reader::{RuntimeProviderSseJsonReader, RuntimeProviderSseJsonState};
use std::io::{self, Read};
use std::sync::Arc;

#[path = "gemini_sse_guardrails.rs"]
mod gemini_sse_guardrails;
#[path = "gemini_sse_state.rs"]
mod gemini_sse_state;

pub(super) use gemini_sse_guardrails::runtime_gemini_forced_command_output;
use gemini_sse_state::RuntimeGeminiSseState;

pub(super) type RuntimeGeminiBindingRecorder = Arc<dyn Fn(String, Vec<String>) + Send + Sync>;

pub(super) struct RuntimeGeminiGenerateSseReader<R: Read> {
    inner: RuntimeProviderSseJsonReader<R, RuntimeGeminiSseState>,
}

impl<R: Read> RuntimeGeminiGenerateSseReader<R> {
    pub(super) fn new(
        reader: R,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
        binding_recorder: Option<RuntimeGeminiBindingRecorder>,
    ) -> Self {
        Self {
            inner: RuntimeProviderSseJsonReader::new(
                reader,
                RuntimeGeminiSseState::new(
                    request_id,
                    conversation_messages,
                    conversations,
                    binding_recorder,
                ),
            ),
        }
    }
}

impl RuntimeProviderSseJsonState for RuntimeGeminiSseState {
    fn eof(&self) -> bool {
        self.eof
    }

    fn set_eof(&mut self, eof: bool) {
        self.eof = eof;
    }

    fn observe_value(&mut self, value: &serde_json::Value) -> Vec<String> {
        let value = runtime_gemini_normalized_response_value(value);
        self.observe_generate_chunk(&value)
    }

    fn complete_event(&mut self) -> Option<String> {
        RuntimeGeminiSseState::complete_event(self)
    }

    fn failed_event(&mut self, code: &str, message: &str) -> Option<String> {
        RuntimeGeminiSseState::failed_event(self, code, message)
    }
}

impl<R: Read> Read for RuntimeGeminiGenerateSseReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

#[cfg(test)]
#[path = "gemini_sse_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gemini_sse_exact_output_tests.rs"]
mod exact_output_tests;

#[cfg(test)]
#[path = "gemini_sse_guardrail_tests.rs"]
mod guardrail_tests;

#[cfg(test)]
#[path = "gemini_sse_media_tests.rs"]
mod media_tests;

#[cfg(test)]
#[path = "gemini_sse_status_tests.rs"]
mod status_tests;
